//! QFSNC1 framed CBOR wire protocol (shared by plain TCP and TLS transports).
//!
//! ```text
//! magic:  b"QFSNC1\0\0"   (8 bytes)
//! length: u32 LE          (payload bytes, max 16 MiB)
//! payload: CBOR WireRequest / WireResponse
//! ```

use crate::hub::{
    DrainNotifyRequest, Hello, HelloAck, NotifyBatch, PullRequest, PullResponse, PushAck,
    PushRequest, SubscribeRequest, Subscribed, SyncHub,
};
use crate::SyncError;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

pub const MAGIC: &[u8; 8] = b"QFSNC1\0\0";
pub const MAX_FRAME: u32 = 16 * 1024 * 1024;
/// Protocol version string for documentation / negotiation hooks.
pub const PROTOCOL_ID: &str = "QFSNC1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WireRequest {
    Hello(Hello),
    Pull(PullRequest),
    Push(PushRequest),
    Subscribe(SubscribeRequest),
    DrainNotify(DrainNotifyRequest),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WireResponse {
    HelloAck(HelloAck),
    Pull(PullResponse),
    PushAck(PushAck),
    Subscribed(Subscribed),
    Notify(NotifyBatch),
    Error { kind: String, message: String },
}

pub fn write_frame<W: Write, T: Serialize>(stream: &mut W, value: &T) -> Result<(), SyncError> {
    let mut payload = Vec::new();
    ciborium::into_writer(value, &mut payload).map_err(|e| SyncError::Network(e.to_string()))?;
    if payload.len() > MAX_FRAME as usize {
        return Err(SyncError::Rejected("frame too large".into()));
    }
    let len = (payload.len() as u32).to_le_bytes();
    stream
        .write_all(MAGIC)
        .map_err(|e| SyncError::Network(e.to_string()))?;
    stream
        .write_all(&len)
        .map_err(|e| SyncError::Network(e.to_string()))?;
    stream
        .write_all(&payload)
        .map_err(|e| SyncError::Network(e.to_string()))?;
    stream
        .flush()
        .map_err(|e| SyncError::Network(e.to_string()))?;
    Ok(())
}

pub fn read_frame<R: Read, T: DeserializeOwned>(stream: &mut R) -> Result<T, SyncError> {
    let mut magic = [0u8; 8];
    stream
        .read_exact(&mut magic)
        .map_err(|e| SyncError::Network(e.to_string()))?;
    if &magic != MAGIC {
        return Err(SyncError::Rejected("bad frame magic".into()));
    }
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .map_err(|e| SyncError::Network(e.to_string()))?;
    let len = u32::from_le_bytes(len_buf);
    if len == 0 || len > MAX_FRAME {
        return Err(SyncError::Rejected(format!("invalid frame length {len}")));
    }
    let mut payload = vec![0u8; len as usize];
    stream
        .read_exact(&mut payload)
        .map_err(|e| SyncError::Network(e.to_string()))?;
    ciborium::from_reader(payload.as_slice()).map_err(|e| SyncError::Rejected(e.to_string()))
}

pub fn dispatch(hub: &Arc<Mutex<SyncHub>>, req: WireRequest) -> WireResponse {
    let mut hub = match hub.lock() {
        Ok(h) => h,
        Err(e) => {
            return WireResponse::Error {
                kind: "internal".into(),
                message: e.to_string(),
            }
        }
    };
    match req {
        WireRequest::Hello(h) => match hub.hello(h) {
            Ok(ack) => WireResponse::HelloAck(ack),
            Err(e) => err_resp(e),
        },
        WireRequest::Pull(p) => match hub.pull(p) {
            Ok(r) => WireResponse::Pull(r),
            Err(e) => err_resp(e),
        },
        WireRequest::Push(p) => match hub.push(p) {
            Ok(a) => WireResponse::PushAck(a),
            Err(e) => err_resp(e),
        },
        WireRequest::Subscribe(s) => match hub.subscribe(s) {
            Ok(a) => WireResponse::Subscribed(a),
            Err(e) => err_resp(e),
        },
        WireRequest::DrainNotify(d) => match hub.drain_notifies(d) {
            Ok(n) => WireResponse::Notify(n),
            Err(e) => err_resp(e),
        },
    }
}

pub fn err_resp(e: SyncError) -> WireResponse {
    let (kind, message) = match e {
        SyncError::Rejected(s) => ("rejected", s),
        SyncError::Network(s) => ("network", s),
        SyncError::Auth(s) => ("auth", s),
        SyncError::StreamMismatch(s) => ("stream_mismatch", s),
    };
    WireResponse::Error {
        kind: kind.into(),
        message,
    }
}

pub fn map_wire_err(kind: String, message: String) -> SyncError {
    match kind.as_str() {
        "auth" => SyncError::Auth(message),
        "rejected" => SyncError::Rejected(message),
        "stream_mismatch" => SyncError::StreamMismatch(message),
        _ => SyncError::Network(format!("{kind}: {message}")),
    }
}

/// Serve one request/response on an already-connected byte stream.
pub fn serve_one<S: Read + Write>(
    stream: &mut S,
    hub: &Arc<Mutex<SyncHub>>,
) -> Result<(), SyncError> {
    let resp = match read_frame::<_, WireRequest>(stream) {
        Ok(req) => dispatch(hub, req),
        Err(e) => err_resp(e),
    };
    write_frame(stream, &resp)
}

#[cfg(test)]
mod cbor_shape_tests {
    use super::*;
    use crate::hub::Hello;
    use qf_core::{ActorId, DocumentId, SessionId, SeqIdGen};

    #[test]
    fn hello_cbor_is_tagged_map() {
        let mut gen = SeqIdGen::default();
        // Fixed-looking ids from seq gen for deterministic inspection.
        let req = WireRequest::Hello(Hello {
            client_id: ActorId::new(&mut gen),
            session_id: SessionId::new(&mut gen),
            document_id: DocumentId::new(&mut gen),
            have_seq: 0,
        });
        let mut payload = Vec::new();
        ciborium::into_writer(&req, &mut payload).unwrap();
        // Externally tagged enum: map with one key "hello" (or type field with rename)
        // serde tag = "type", rename_all = snake_case → {"type":"hello", ...fields}
        assert!(!payload.is_empty());
        // First byte should be a map major type (0xA0-0xBF) or indefinite map 0xBF
        let major = payload[0] >> 5;
        assert_eq!(major, 5, "expected CBOR map, first byte {:02x}", payload[0]);
        // Keep a golden prefix check for browser encoder compatibility.
        // Must start with map containing text key "type".
        let as_str = String::from_utf8_lossy(&payload);
        assert!(
            as_str.contains("type") && as_str.contains("hello"),
            "payload missing type/hello: {as_str:?}"
        );
    }

    #[test]
    fn hello_fixed_ids_matches_browser_golden() {
        use uuid::Uuid;
        let req = WireRequest::Hello(Hello {
            client_id: ActorId(Uuid::from_u128(1)),
            session_id: SessionId(Uuid::from_u128(2)),
            document_id: DocumentId(Uuid::from_u128(3)),
            have_seq: 0,
        });
        let mut p = Vec::new();
        ciborium::into_writer(&req, &mut p).unwrap();
        let mut hex = String::with_capacity(p.len() * 2);
        for b in &p {
            use std::fmt::Write as _;
            let _ = write!(hex, "{b:02x}");
        }
        // Kept in sync with web/qfsnc1-client/qfsnc1.js selfTest().
        let expected = "a564747970656568656c6c6f69636c69656e745f696450000000000000000000000000000000016a73657373696f6e5f696450000000000000000000000000000000026b646f63756d656e745f6964500000000000000000000000000000000368686176655f73657100";
        assert_eq!(hex, expected);
    }
}
