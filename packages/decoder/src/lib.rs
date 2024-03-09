#![deny(clippy::all)]

use std::io::BufReader;

use crossbeam_channel::Sender;
use internal::{
  decoder::Decoder as PlatformDecoder,
  io::{CrossbeamChunk, CrossbeamReader},
  try_probe_format,
};
use napi::{
  self,
  bindgen_prelude::{BufferSlice, Int16Array},
};
use symphonia::core::{
  formats::FormatOptions,
  io::{MediaSourceStream, MediaSourceStreamOptions, ReadOnlySource},
  probe::Hint,
};
use tracing::{debug, info};
use tracing_log::LogTracer;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use crate::internal::error::Error;

mod internal;

#[macro_use]
extern crate napi_derive;

/// Enables logging within the native module.
///
/// Note: Uses [EnvFilter](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html)
/// under the hood - TL;DR this means you need `RUST_LOG=trace` (or similar) in the environment to see any output.
#[napi]
pub fn set_native_tracing(enabled: bool) -> napi::Result<()> {
  LogTracer::init()
    .map_err(|e| napi::Error::from_reason(format!("Failed to enable logging adapter: {e}")))?;

  if enabled {
    let subscriber = FmtSubscriber::builder()
      .with_env_filter(EnvFilter::from_default_env())
      .finish();

    tracing::subscriber::set_global_default(subscriber)
      .map_err(|e| napi::Error::from_reason(format!("Failed to install tracing provider: {e}")))?;
  } else {
    tracing::subscriber::set_global_default(tracing::subscriber::NoSubscriber::new()).map_err(
      |e| napi::Error::from_reason(format!("Failed to uninstall tracing provider: {e}")),
    )?;
  }

  info!(enabled, "tracing modified");

  Ok(())
}

#[allow(clippy::large_enum_variant)]
pub enum DecoderState {
  Pending(CrossbeamReader),
  Decoding(PlatformDecoder),
  Closed,
}

/// Configuration for a [`Decoder`].
#[napi(object)]
pub struct DecoderConfig {
  /// A high water mark, indicating the maximum amount of bytes that can be buffered
  /// within the decoder.
  pub high_water_mark: Option<u32>,

  /// Enables gapless decoding.
  pub enable_gapless: Option<bool>,

  /// Provides the decoder with a media format hint, based on it's mime type.
  pub mime_type: Option<String>,

  /// Provides the decoder with a media format hint, based on it's file extension.
  pub file_extension: Option<String>,
}

impl DecoderConfig {
  pub fn as_hint(&self) -> Hint {
    let mut hint = Hint::default();

    if let Some(mime_type) = &self.mime_type {
      hint.mime_type(mime_type);
    }

    if let Some(file_extension) = &self.file_extension {
      hint.with_extension(file_extension);
    }

    hint
  }

  pub fn as_format_options(&self) -> FormatOptions {
    FormatOptions {
      enable_gapless: self.enable_gapless.unwrap_or(false),
      ..Default::default()
    }
  }
}

/// A decoded audio sample.
#[napi(object)]
pub struct DecodedAudioSample {
  /// The audio data.
  ///
  /// Note: This contains interleaved PCM audio (Signed 16-bit PCM, Little-Endian).
  pub data: Int16Array,

  /// The audio sample rate.
  ///
  /// Note: For example `44100`.
  pub sample_rate: u32,

  /// The audio channel count.
  ///
  /// Note: For example `1` for mono, or `2` for stereo.
  pub channel_count: u32,
}

/// A decoder that processes audio samples from various supported containers
/// and codecs into interleaved PCM audio (Signed 16-bit PCM, Little-Endian) stored in a `Int16Array`.
#[napi]
pub struct Decoder {
  config: DecoderConfig,
  tx: Sender<CrossbeamChunk>,
  state: DecoderState,
}

#[napi]
impl Decoder {
  /// Creates a new decoder from the given `config`.
  #[allow(clippy::new_without_default)]
  #[napi(constructor)]
  #[tracing::instrument(skip_all)]
  pub fn new(config: DecoderConfig) -> Self {
    let (tx, reader) = CrossbeamReader::new(config.high_water_mark.map(|capacity| capacity as _));

    let state = DecoderState::Pending(reader);

    Self { config, tx, state }
  }

  /// Appends a chunk of data to the decoder, optionally resulting in a processed sample.
  #[napi]
  #[tracing::instrument(skip_all)]
  pub fn append(&mut self, buffer: BufferSlice) -> napi::Result<Option<DecodedAudioSample>> {
    if matches!(self.state, DecoderState::Closed) {
      return Err(napi::Error::from_reason("Decoder is closed"));
    }

    debug!("attempting to append data");

    self
      .tx
      .send(CrossbeamChunk::Data(buffer.to_vec()))
      .map_err(|e| napi::Error::from_reason(format!("Failed to append buffer: {e}")))?;

    if let DecoderState::Pending(buffer) = &self.state {
      debug!("attempting to probe data");

      let mss = buffer.clone();
      let mss = MediaSourceStream::new(
        Box::new(ReadOnlySource::new(BufReader::new(mss))),
        MediaSourceStreamOptions::default(),
      );

      if let Ok(probe_result) = try_probe_format(
        mss,
        &self.config.as_hint(),
        &self.config.as_format_options(),
      ) {
        debug!("attempting to allocate decoder");

        if let Ok(decoder) = PlatformDecoder::from_probe_result(probe_result) {
          self.state = DecoderState::Decoding(decoder);
        }
      }
    }

    self.flush()
  }

  /// Flushes data in the decoder, optionally resulting in a processed sample.
  #[napi]
  #[tracing::instrument(skip_all)]
  pub fn flush(&mut self) -> napi::Result<Option<DecodedAudioSample>> {
    if matches!(self.state, DecoderState::Closed) {
      return Err(napi::Error::from_reason("Decoder is closed"));
    }

    debug!("attempting to flush data");

    if let DecoderState::Decoding(ref mut decoder) = self.state {
      let mut decoded_spec = None;
      let mut decoded_sample = Vec::new();

      // TODO(bengreenier): this blocks when we run out of shit to read
      for maybe_sample_result in decoder.iter_mut::<i16>() {
        debug!("attempting to process next sample");

        match maybe_sample_result {
          Ok(sample) => {
            let mut data = sample.buffer.samples().to_owned();

            debug!("sample processed");

            match decoded_spec {
              Some(spec) => {
                if spec != sample.spec {
                  return Err(napi::Error::from_reason(format!(
                    "Signal format changed mid-decode: {:?} != {:?}",
                    spec, sample.spec
                  )));
                }
              }
              None => decoded_spec = Some(sample.spec),
            }

            decoded_sample.append(&mut data);
          }
          Err(ref err) => {
            if matches!(err, Error::InsufficientData) {
              break;
            }
          }
        }
      }

      debug!(sample_len = decoded_sample.len(), spec = ?decoded_spec, "generating node sample");

      if decoded_sample.is_empty() {
        return Ok(None);
      }

      // this mirrors decoded_sample, so if that's not empty we can safely unwrap here
      let spec = decoded_spec.unwrap();
      let channel_count = spec.channels.count() as u32;

      // a consumer might notice that playing back at this sample rate results in "sped-up" audio
      // this is likely because channel_count > 1, so this sample rate needs to be divided by channel_count
      // for accurate playback
      let sample_rate = spec.rate;

      return Ok(Some(DecodedAudioSample {
        data: Int16Array::new(decoded_sample),
        sample_rate,
        channel_count,
      }));
    }

    Ok(None)
  }

  /// Notifies the encoder that no more data will be sent.
  ///
  /// Note: This is used as a signal that the decoder should finish processing data.
  #[napi]
  #[tracing::instrument(skip_all)]
  pub fn finalize(&mut self) -> napi::Result<()> {
    debug!("attempting to finalize decoder");

    self
      .tx
      .send(CrossbeamChunk::End)
      .map_err(|e| napi::Error::from_reason(format!("Failed to finalize: {e}")))?;

    Ok(())
  }

  /// Closes the decoder, ensuring all internal resources are cleaned up.
  #[napi]
  #[tracing::instrument(skip_all)]
  pub fn close(&mut self) -> napi::Result<()> {
    debug!("attempting to close decoder");

    self
      .tx
      .send(CrossbeamChunk::End)
      .map_err(|e| napi::Error::from_reason(format!("Failed to close: {e}")))?;

    self.state = DecoderState::Closed;

    Ok(())
  }
}
