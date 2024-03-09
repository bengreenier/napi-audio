use symphonia::{
  core::{
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::{Limit, MetadataOptions},
    probe::{Hint, ProbeResult},
  },
  default::get_probe,
};

use error::Result;

pub mod decoder;
pub mod error;
pub mod io;

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
