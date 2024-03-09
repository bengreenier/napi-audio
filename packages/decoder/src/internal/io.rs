use std::{
  fmt::{Debug, Display},
  io::{Error as IoError, ErrorKind as IoErrorKind, Read},
  ops::Deref,
};

use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use thiserror::Error;

#[derive(Debug, Error)]
pub struct EndMarkerError;

impl Display for EndMarkerError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("EndMarkerError").finish()
  }
}

pub struct CrossbeamData(pub Vec<u8>);

impl Deref for CrossbeamData {
  type Target = Vec<u8>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl Debug for CrossbeamData {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("CrossbeamData")
      .field("len", &self.len())
      .finish_non_exhaustive()
  }
}

#[derive(Debug, PartialEq, Eq)]
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
        return Err(IoError::new(
          IoErrorKind::UnexpectedEof,
          "No data available for reading at this time",
        ));
      }

      self.buffer = match self.rx.recv() {
        Ok(v) => match v {
          CrossbeamChunk::Data(v) => v,
          CrossbeamChunk::End => return Err(IoError::other(EndMarkerError)),
        },
        Err(err) => return Err(IoError::new(IoErrorKind::NotConnected, err)),
      };
      self.offset = 0;
    }
    let size = std::cmp::min(buf.len(), self.buffer.len() - self.offset);
    buf[..size].copy_from_slice(&self.buffer[self.offset..self.offset + size]);
    self.offset += size;
    Ok(size)
  }
}

impl Debug for CrossbeamReader {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("CrossbeamReader")
      .field("rx", &self.rx)
      .field("offset", &self.offset)
      .field("len", &self.len())
      .finish_non_exhaustive()
  }
}

impl CrossbeamReader {
  pub fn len(&self) -> usize {
    self.buffer.len()
  }

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
