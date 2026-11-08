//! `Transfer-Encoding: chunked` bodies, decoded as they arrive.

use alloc::vec::Vec;

use super::HttpError;

/// Incremental decoder for `Transfer-Encoding: chunked` bodies.
#[derive(Debug, Clone, Default)]
pub struct ChunkedDecoder {
    state: ChunkState,
    line: Vec<u8>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum ChunkState {
    #[default]
    Size,
    Data(u64),
    DataEnd,
    Trailer,
    Done,
}

/// Longest chunk-size or trailer line accepted.
const MAX_CHUNK_LINE: usize = 4096;

impl ChunkedDecoder {
    /// Feeds `input`, appending decoded body bytes to `out`. Returns the number
    /// of input bytes consumed (less than `input.len()` only once done).
    ///
    /// # Errors
    /// Malformed chunk framing.
    pub fn decode(&mut self, input: &[u8], out: &mut Vec<u8>) -> Result<usize, HttpError> {
        let mut i = 0;
        while i < input.len() && self.state != ChunkState::Done {
            match self.state {
                ChunkState::Size | ChunkState::Trailer => {
                    let b = input[i];
                    i += 1;
                    if b != b'\n' {
                        if self.line.len() >= MAX_CHUNK_LINE {
                            return Err(HttpError::Malformed("chunk line too long"));
                        }
                        self.line.push(b);
                        continue;
                    }
                    let line = core::mem::take(&mut self.line);
                    let line = line.strip_suffix(b"\r").unwrap_or(&line);
                    if self.state == ChunkState::Trailer {
                        if line.is_empty() {
                            self.state = ChunkState::Done;
                        }
                        continue;
                    }
                    let hex = line.split(|&c| c == b';').next().unwrap_or(&[]);
                    let hex = core::str::from_utf8(hex).map_err(|_| HttpError::Malformed("chunk size"))?.trim();
                    let size = u64::from_str_radix(hex, 16).map_err(|_| HttpError::Malformed("chunk size"))?;
                    self.state = if size == 0 { ChunkState::Trailer } else { ChunkState::Data(size) };
                }
                ChunkState::Data(left) => {
                    let n = usize::try_from(left).unwrap_or(usize::MAX).min(input.len() - i);
                    out.extend_from_slice(&input[i..i + n]);
                    i += n;
                    let left = left - n as u64;
                    self.state = if left == 0 { ChunkState::DataEnd } else { ChunkState::Data(left) };
                }
                ChunkState::DataEnd => {
                    let b = input[i];
                    i += 1;
                    match b {
                        b'\r' => {}
                        b'\n' => self.state = ChunkState::Size,
                        _ => return Err(HttpError::Malformed("missing CRLF after chunk")),
                    }
                }
                ChunkState::Done => {}
            }
        }
        Ok(i)
    }

    #[must_use]
    pub fn is_done(&self) -> bool {
        self.state == ChunkState::Done
    }
}
