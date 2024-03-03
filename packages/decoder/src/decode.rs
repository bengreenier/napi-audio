use std::{
  fmt::{Debug, Display},
  io::Read,
  marker::PhantomData,
};

use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use symphonia::{
  core::{
    audio::{SampleBuffer, SignalSpec},
    codecs::{Decoder as Engine, DecoderOptions as EngineOptions},
    conv::ConvertibleSample,
    errors::Error as SymphoniaError,
    formats::{FormatOptions, FormatReader, Track},
    io::MediaSourceStream,
    meta::{Limit, MetadataOptions},
    probe::{Hint, ProbeResult, ProbedMetadata},
  },
  default::{get_codecs, get_probe},
};
use thiserror::Error;
use tracing::trace;

#[derive(Debug, Error)]
pub enum Error {
  #[error(transparent)]
  Symphonia(#[from] SymphoniaError),
  #[error("The decoder was reset, please try again")]
  ResetRequired,
  #[error("The decoder is waiting for additional data, please try again")]
  InsufficientData,
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub struct EndMarkerError;

impl Display for EndMarkerError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("EndMarkerError").finish()
  }
}

pub enum CrossbeamChunk {
  Data(Vec<u8>),
  End,
}

#[derive(Clone)]
pub struct CrossbeamReader {
  rx: Receiver<CrossbeamChunk>,
  offset: usize,
  buffer: Vec<u8>,
}

impl Read for CrossbeamReader {
  fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
    while self.offset >= self.buffer.len() {
      if self.rx.is_empty() {
        return Err(std::io::Error::new(
          std::io::ErrorKind::UnexpectedEof,
          "No data available for reading",
        ));
      }

      self.buffer = match self.rx.recv() {
        Ok(v) => match v {
          CrossbeamChunk::Data(v) => v,
          CrossbeamChunk::End => return Err(std::io::Error::other(EndMarkerError)),
        },
        Err(err) => return Err(std::io::Error::new(std::io::ErrorKind::NotConnected, err)),
      };
      self.offset = 0;
    }
    let size = std::cmp::min(buf.len(), self.buffer.len() - self.offset);
    buf[..size].copy_from_slice(&self.buffer[self.offset..self.offset + size]);
    self.offset += size;
    Ok(size)
  }
}

impl CrossbeamReader {
  pub fn new(capacity: Option<usize>) -> (Sender<CrossbeamChunk>, Self) {
    let (tx, rx) = match capacity {
      Some(capacity) => bounded(capacity),
      None => unbounded(),
    };

    (
      tx,
      Self {
        rx,
        offset: 0,
        buffer: Vec::new(),
      },
    )
  }
}

pub fn try_probe_format(
  mss: MediaSourceStream,
  hint: &Hint,
  format_options: &FormatOptions,
) -> Result<ProbeResult> {
  let probe = get_probe();

  let result = probe.format(
    hint,
    mss,
    format_options,
    &MetadataOptions {
      limit_visual_bytes: Limit::Maximum(0),
      ..Default::default()
    },
  )?;

  Ok(result)
}

pub struct Decoder {
  metadata: ProbedMetadata,
  track: Track,
  reader: Box<dyn FormatReader>,
  engine: Box<dyn Engine>,
}

impl TryFrom<ProbeResult> for Decoder {
  type Error = Error;
  fn try_from(value: ProbeResult) -> Result<Self> {
    let codecs = get_codecs();
    let reader = value.format;
    let metadata = value.metadata;

    let track = reader
      .default_track()
      .ok_or(SymphoniaError::Unsupported("Missing default track"))?;

    let engine = codecs.make(&track.codec_params, &EngineOptions::default())?;

    Ok(Self {
      metadata,
      track: track.to_owned(),
      reader,
      engine,
    })
  }
}

impl Debug for Decoder {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Decoder")
      .field("track", &self.track)
      .finish_non_exhaustive()
  }
}

impl Decoder {
  pub fn from_probe_result(probe_result: ProbeResult) -> Result<Self> {
    probe_result.try_into()
  }

  pub fn metadata(&self) -> &ProbedMetadata {
    &self.metadata
  }

  pub fn iter_mut<S: ConvertibleSample>(&mut self) -> DecoderIter<'_, S> {
    DecoderIter {
      decoder: self,
      _ty: PhantomData,
    }
  }
}

pub struct DecodedSample<S: ConvertibleSample> {
  pub spec: SignalSpec,
  pub buffer: SampleBuffer<S>,
}

impl<S: ConvertibleSample> Debug for DecodedSample<S> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("DecodedSample")
      .field("spec", &self.spec)
      .field("buffer_len", &self.buffer.len())
      .finish_non_exhaustive()
  }
}

pub struct DecoderIter<'a, S: ConvertibleSample> {
  decoder: &'a mut Decoder,
  _ty: PhantomData<S>,
}

impl<'a, S: ConvertibleSample> Debug for DecoderIter<'a, S> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("DecoderIter")
      .field("decoder", &self.decoder)
      .finish_non_exhaustive()
  }
}

impl<'a, S> Iterator for DecoderIter<'a, S>
where
  S: ConvertibleSample,
{
  type Item = Result<DecodedSample<S>>;

  #[tracing::instrument]
  fn next(&mut self) -> Option<Self::Item> {
    let maybe_packet = self.decoder.reader.next_packet();

    let packet = match maybe_packet {
      Ok(packet) => packet,
      Err(err) => match &err {
        SymphoniaError::IoError(io) => match io.kind() {
          std::io::ErrorKind::Other => {
            // inspect the underlying boxed error
            if let Some(boxed_err) = io.get_ref() {
              // handle the end marker
              if boxed_err.is::<EndMarkerError>() {
                return None;
              }
            }

            return Some(Err(Error::Symphonia(err)));
          }
          // handle eofs as if there's just no available sample
          std::io::ErrorKind::UnexpectedEof => {
            return Some(Err(Error::InsufficientData));
          }
          _ => return Some(Err(Error::Symphonia(err))),
        },
        // handle resets
        SymphoniaError::ResetRequired => {
          self.decoder.engine.reset();

          return Some(Err(Error::ResetRequired));
        }
        _ => return Some(Err(Error::Symphonia(err))),
      },
    };

    if packet.track_id() != self.decoder.track.id {
      trace!("TrackIdMismatch: None");

      return None;
    }

    let maybe_audio = self.decoder.engine.decode(&packet);

    let audio = match maybe_audio {
      Ok(audio) => audio,
      Err(err) => return Some(Err(Error::Symphonia(err))),
    };

    let spec = *audio.spec();
    let duration = audio.capacity() as _;

    let mut buffer = SampleBuffer::<S>::new(duration, spec);
    buffer.copy_interleaved_ref(audio);

    Some(Ok(DecodedSample { buffer, spec }))
  }
}

#[cfg(test)]
mod test {
  use std::io::{BufReader, Write};

  use symphonia::core::io::{MediaSourceStreamOptions, ReadOnlySource};
  use tracing::{debug, error};
  use tracing_log::LogTracer;
  use tracing_test::traced_test;

  use super::*;

  #[test]
  #[traced_test]
  fn decode_mp3() {
    LogTracer::init().unwrap();

    let (sender, buffer) = CrossbeamReader::new(None);
    let buffer = BufReader::new(buffer);

    sender
      .send(CrossbeamChunk::Data(
        std::fs::read("./test-media.mp3").unwrap(),
      ))
      .unwrap();

    sender.send(CrossbeamChunk::End).unwrap();

    let mss = MediaSourceStream::new(
      Box::new(ReadOnlySource::new(buffer)),
      MediaSourceStreamOptions::default(),
    );

    let mut hint = Hint::new();
    hint.with_extension("mp3");
    hint.mime_type("audio/mpeg3");

    let probe_result = try_probe_format(mss, &hint, &FormatOptions::default()).unwrap();
    let mut decoder = Decoder::from_probe_result(probe_result).unwrap();

    let mut out = std::fs::File::create("./test-media-mp3.pcm").unwrap();

    for maybe_sample in decoder.iter_mut::<i16>() {
      match maybe_sample {
        Ok(sample) => {
          debug!(?sample, "got data");
          for value in sample.buffer.samples() {
            let bytes: [u8; 2] = value.to_le_bytes();
            out.write_all(&bytes).unwrap()
          }
        }
        Err(err) => error!(?err, "got error"),
      }
    }
  }
}
