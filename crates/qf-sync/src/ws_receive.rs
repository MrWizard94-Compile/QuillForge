//! Stateful WS receive path, shared by TCP and rustls streams.
//!
//! A socket timeout is an idle poll, not a frame boundary. Use `Read::read`
//! and retain exact offsets: `read_exact` does not report partial progress on
//! error. Keep transport error kinds until the session decides whether to retry.

use crate::wire::read_frame;
use crate::SyncError;
use std::io::{self, Cursor, ErrorKind, Read};

const MAX_WS_PAYLOAD: u64 = 16 * 1024 * 1024;

#[derive(Debug)]
pub(crate) enum WsReadError {
    Io(io::Error),
    Protocol(SyncError),
    Closed,
}

impl WsReadError {
    pub(crate) fn is_idle(&self) -> bool {
        matches!(self, Self::Io(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut))
    }

    pub(crate) fn is_peer_gone(&self) -> bool {
        matches!(self, Self::Closed)
            || matches!(self, Self::Io(e) if matches!(e.kind(),
                ErrorKind::UnexpectedEof | ErrorKind::ConnectionReset |
                ErrorKind::ConnectionAborted | ErrorKind::BrokenPipe | ErrorKind::NotConnected))
    }
}

impl From<WsReadError> for SyncError {
    fn from(error: WsReadError) -> Self {
        match error {
            WsReadError::Io(e) => Self::Network(e.to_string()),
            WsReadError::Protocol(e) => e,
            WsReadError::Closed => Self::Network("websocket closed".into()),
        }
    }
}

/// One reader per connection, including across successive `wait_notify` calls.
/// At most one bounded frame is retained; never read into the next frame.
pub(crate) struct WsReader {
    header: [u8; 14],
    header_read: usize,
    header_len: usize,
    payload: Vec<u8>,
    payload_read: usize,
    payload_len: Option<usize>,
}

impl Default for WsReader {
    fn default() -> Self {
        Self {
            header: [0; 14],
            header_read: 0,
            header_len: 2,
            payload: Vec::new(),
            payload_read: 0,
            payload_len: None,
        }
    }
}

impl WsReader {
    pub(crate) fn read<R: Read, T: serde::de::DeserializeOwned>(
        &mut self,
        stream: &mut R,
    ) -> Result<T, WsReadError> {
        let payload = self.read_binary(stream)?;
        read_frame(&mut Cursor::new(payload)).map_err(WsReadError::Protocol)
    }

    fn read_binary<R: Read>(&mut self, stream: &mut R) -> Result<Vec<u8>, WsReadError> {
        loop {
            fill(
                stream,
                &mut self.header[..self.header_len],
                &mut self.header_read,
            )?;
            let masked = self.header[1] & 0x80 != 0;
            let length_tag = self.header[1] & 0x7f;
            let extended = match length_tag {
                126 => 2,
                127 => 8,
                _ => 0,
            };
            self.header_len = 2 + extended + if masked { 4 } else { 0 };
            fill(
                stream,
                &mut self.header[..self.header_len],
                &mut self.header_read,
            )?;

            if self.payload_len.is_none() {
                let len = match length_tag {
                    126 => u16::from_be_bytes([self.header[2], self.header[3]]) as u64,
                    127 => u64::from_be_bytes(self.header[2..10].try_into().expect("eight bytes")),
                    n => n as u64,
                };
                // Check the u64 before narrowing or allocating (also safe on 32-bit).
                if len > MAX_WS_PAYLOAD {
                    return Err(WsReadError::Protocol(SyncError::Rejected(
                        "websocket frame too large".into(),
                    )));
                }
                self.payload.resize(len as usize, 0);
                self.payload_len = Some(len as usize);
            }
            fill(stream, &mut self.payload, &mut self.payload_read)?;
            if masked {
                let key = &self.header[2 + extended..self.header_len];
                for (i, b) in self.payload.iter_mut().enumerate() {
                    *b ^= key[i % 4];
                }
            }
            let opcode = self.header[0] & 0x0f;
            let payload = std::mem::take(&mut self.payload);
            self.header_read = 0;
            self.header_len = 2;
            self.payload_read = 0;
            self.payload_len = None;
            match opcode {
                0x2 => return Ok(payload),
                0x8 => return Err(WsReadError::Closed),
                // Preserve the foundation transport's existing control handling.
                0x9 | 0xA => continue,
                0x1 => {
                    return Err(WsReadError::Protocol(SyncError::Rejected(
                        "text frames not used; send QFSNC1 as binary".into(),
                    )))
                }
                _ => {
                    return Err(WsReadError::Protocol(SyncError::Rejected(format!(
                        "unsupported websocket opcode {opcode}"
                    ))))
                }
            }
        }
    }
}

fn fill<R: Read>(stream: &mut R, buf: &mut [u8], read: &mut usize) -> Result<(), WsReadError> {
    while *read < buf.len() {
        match stream.read(&mut buf[*read..]) {
            Ok(0) => {
                return Err(WsReadError::Io(io::Error::new(
                    ErrorKind::UnexpectedEof,
                    "websocket peer closed during read",
                )))
            }
            Ok(n) => *read += n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(WsReadError::Io(e)),
        }
    }
    Ok(())
}
