use symphonia::core::errors::Error as SymphoniaError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
  #[error(transparent)]
  Symphonia(#[from] SymphoniaError),
  #[error("The decoder was reset, please try again")]
  ResetRequired,
  #[error("The decoder is waiting for additional data, please try again")]
  InsufficientData,
}

pub type Result<T> = std::result::Result<T, Error>;
