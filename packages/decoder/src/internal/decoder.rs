use std::{fmt::Debug, marker::PhantomData};

use symphonia::{
  core::{
    audio::{SampleBuffer, SignalSpec},
    codecs::{Decoder as Engine, DecoderOptions as EngineOptions, CODEC_TYPE_NULL},
    conv::ConvertibleSample,
    errors::Error as SymphoniaError,
    formats::{FormatReader, Track},
    probe::{ProbeResult, ProbedMetadata},
  },
  default::get_codecs,
};
use tracing::{info, trace};

use super::{
  error::{Error, Result},
  io::EndMarkerError,
};

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

    info!(available_tracks = ?reader.tracks(), "discovered tracks");

    let track = reader
      .tracks()
      .iter()
      .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
      .ok_or(SymphoniaError::Unsupported(
        "No tracks with supported codecs found",
      ))?;

    info!(track_id = track.id, "selected track");

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
      .field("len", &self.len())
      .finish_non_exhaustive()
  }
}

impl<S: ConvertibleSample> DecodedSample<S> {
  pub fn len(&self) -> usize {
    self.buffer.len()
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
