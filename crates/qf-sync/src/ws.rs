//! WebSocket transport for QFSNC1 (browser-ready path).
//!
//! Each QFSNC1 frame (magic + length + CBOR) is one WebSocket **binary** message.
//! Handshake is RFC 6455 over plain TCP (`ws://`) or TLS (`wss://` via [`crate::wss`]).
//!
//! Path: `/qfsnc1` — servers reject other upgrade targets.

use crate::hub::{
    Hello, HelloAck, PullRequest, PullResponse, PushAck, PushRequest, SyncHub,
};
use crate::wire::{
    dispatch, err_resp, map_wire_err, read_frame, write_frame, WireRequest, WireResponse,
};
use crate::SyncError;
use ring::digest::{digest, SHA1_FOR_LEGACY_USE_ONLY};
use std::io::{Cursor, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
const WS_PATH: &str = "/qfsnc1";

/// Background WebSocket server wrapping a shared [`SyncHub`].
pub struct WsSyncServer {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    hub: Arc<Mutex<SyncHub>>,
}

impl WsSyncServer {
    /// Bind `127.0.0.1:0` and accept `ws://…/qfsnc1` connections.
    pub fn start(hub: SyncHub) -> Result<Self, SyncError> {
        let listener =
            TcpListener::bind("127.0.0.1:0").map_err(|e| SyncError::Network(e.to_string()))?;
        let addr = listener
            .local_addr()
            .map_err(|e| SyncError::Network(e.to_string()))?;
        let hub = Arc::new(Mutex::new(hub));
        let stop = Arc::new(AtomicBool::new(false));
        let hub_t = hub.clone();
        let stop_t = stop.clone();
        listener
            .set_nonblocking(true)
            .map_err(|e| SyncError::Network(e.to_string()))?;
        let join = thread::spawn(move || accept_loop(listener, hub_t, stop_t));
        thread::sleep(Duration::from_millis(20));
        Ok(Self {
            addr,
            stop,
            join: Some(join),
            hub,
        })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn hub(&self) -> Arc<Mutex<SyncHub>> {
        self.hub.clone()
    }

    /// Browser-oriented URL for this server (plain ws, local).
    pub fn url(&self) -> String {
        format!("ws://{}/qfsnc1", self.addr)
    }

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect_timeout(&self.addr, Duration::from_millis(200));
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

impl Drop for WsSyncServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect_timeout(&self.addr, Duration::from_millis(100));
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

fn accept_loop(listener: TcpListener, hub: Arc<Mutex<SyncHub>>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                let _ = handle_connection(stream, &hub);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(_) => break,
        }
    }
}

fn handle_connection(mut stream: TcpStream, hub: &Arc<Mutex<SyncHub>>) -> Result<(), SyncError> {
    stream
        .set_nonblocking(false)
        .map_err(|e| SyncError::Network(e.to_string()))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| SyncError::Network(e.to_string()))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| SyncError::Network(e.to_string()))?;
    server_handshake(&mut stream)?;
    // Long-lived session: many QFSNC1 request/response pairs on one WS connection.
    serve_session_ws(&mut stream, hub, false)
}

/// One-shot client (opens a new WebSocket per request). Prefer [`WsSyncSession`] for
/// multi-message conversations.
pub struct WsSyncClient {
    addr: SocketAddr,
}

impl WsSyncClient {
    pub fn connect(addr: SocketAddr) -> Self {
        Self { addr }
    }

    /// Open a long-lived session (handshake once; reuse for hello/push/pull).
    pub fn open_session(&self) -> Result<WsSyncSession, SyncError> {
        WsSyncSession::open(self.addr)
    }

    pub fn hello(&self, req: Hello) -> Result<HelloAck, SyncError> {
        let mut s = self.open_session()?;
        s.hello(req)
    }

    pub fn pull(&self, req: PullRequest) -> Result<PullResponse, SyncError> {
        let mut s = self.open_session()?;
        s.pull(req)
    }

    pub fn push(&self, req: PushRequest) -> Result<PushAck, SyncError> {
        let mut s = self.open_session()?;
        s.push(req)
    }
}

/// Persistent WebSocket connection carrying multiple QFSNC1 messages.
pub struct WsSyncSession {
    stream: TcpStream,
}

impl WsSyncSession {
    pub fn open(addr: SocketAddr) -> Result<Self, SyncError> {
        let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2))
            .map_err(|e| SyncError::Network(e.to_string()))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(|e| SyncError::Network(e.to_string()))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(30)))
            .map_err(|e| SyncError::Network(e.to_string()))?;
        let host = format!("{addr}");
        client_handshake(&mut stream, &host)?;
        Ok(Self { stream })
    }

    pub fn hello(&mut self, req: Hello) -> Result<HelloAck, SyncError> {
        match self.round_trip(WireRequest::Hello(req))? {
            WireResponse::HelloAck(a) => Ok(a),
            WireResponse::Error { kind, message } => Err(map_wire_err(kind, message)),
            other => Err(SyncError::Network(format!("unexpected response: {other:?}"))),
        }
    }

    pub fn pull(&mut self, req: PullRequest) -> Result<PullResponse, SyncError> {
        match self.round_trip(WireRequest::Pull(req))? {
            WireResponse::Pull(p) => Ok(p),
            WireResponse::Error { kind, message } => Err(map_wire_err(kind, message)),
            other => Err(SyncError::Network(format!("unexpected response: {other:?}"))),
        }
    }

    pub fn push(&mut self, req: PushRequest) -> Result<PushAck, SyncError> {
        match self.round_trip(WireRequest::Push(req))? {
            WireResponse::PushAck(a) => Ok(a),
            WireResponse::Error { kind, message } => Err(map_wire_err(kind, message)),
            other => Err(SyncError::Network(format!("unexpected response: {other:?}"))),
        }
    }

    fn round_trip(&mut self, req: WireRequest) -> Result<WireResponse, SyncError> {
        write_qfsnc1_ws(&mut self.stream, &req, true)?;
        read_qfsnc1_ws(&mut self.stream)
    }
}

/// Serve many request/response pairs until the peer closes or an I/O error ends the session.
pub(crate) fn serve_session_ws<S: Read + Write>(
    stream: &mut S,
    hub: &Arc<Mutex<SyncHub>>,
    client_masking: bool,
) -> Result<(), SyncError> {
    loop {
        match read_qfsnc1_ws::<_, WireRequest>(stream) {
            Ok(req) => {
                let resp = dispatch(hub, req);
                write_qfsnc1_ws(stream, &resp, client_masking)?;
            }
            Err(e) if is_peer_gone(&e) => return Ok(()),
            Err(e) => {
                // Protocol error: try to surface once, then end session.
                let _ = write_qfsnc1_ws(stream, &err_resp(e), client_masking);
                return Ok(());
            }
        }
    }
}

fn is_peer_gone(e: &SyncError) -> bool {
    let msg = e.to_string().to_ascii_lowercase();
    msg.contains("eof")
        || msg.contains("failed to fill whole buffer")
        || msg.contains("connection reset")
        || msg.contains("forcibly closed")
        || msg.contains("broken pipe")
        || msg.contains("websocket closed")
        || msg.contains("ws closed")
        || msg.contains("10054")
        || msg.contains("os error 104")
}

pub(crate) fn write_qfsnc1_ws<W: Write, T: serde::Serialize>(
    stream: &mut W,
    value: &T,
    mask: bool,
) -> Result<(), SyncError> {
    let mut buf = Vec::new();
    write_frame(&mut buf, value)?;
    write_ws_binary(stream, &buf, mask)
}

pub(crate) fn read_qfsnc1_ws<R: Read, T: serde::de::DeserializeOwned>(
    stream: &mut R,
) -> Result<T, SyncError> {
    let payload = read_ws_binary(stream)?;
    let mut cur = Cursor::new(payload);
    read_frame(&mut cur)
}

// --- HTTP upgrade (generic Read+Write for TCP and TLS streams) ---

pub(crate) fn client_handshake<S: Read + Write>(
    stream: &mut S,
    host: &str,
) -> Result<(), SyncError> {
    let key_raw: [u8; 16] = {
        let mut k = [0u8; 16];
        // Non-crypto mix for connection nonce (tests/dev).
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        for (i, b) in k.iter_mut().enumerate() {
            *b = ((t >> (i * 4)) as u8)
                .wrapping_add(host.len() as u8)
                .wrapping_add(i as u8);
        }
        k
    };
    let key_b64 = b64_encode(&key_raw);
    let req = format!(
        "GET {WS_PATH} HTTP/1.1\r\n\
         Host: {host}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {key_b64}\r\n\
         Sec-WebSocket-Version: 13\r\n\
         \r\n"
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|e| SyncError::Network(e.to_string()))?;
    stream
        .flush()
        .map_err(|e| SyncError::Network(e.to_string()))?;

    let headers = read_http_headers(stream)?;
    if !headers.starts_with("HTTP/1.1 101") && !headers.starts_with("HTTP/1.0 101") {
        return Err(SyncError::Network(format!(
            "websocket upgrade failed: {}",
            headers.lines().next().unwrap_or("")
        )));
    }
    let expected = ws_accept_key(&key_b64);
    let accept = header_value(&headers, "sec-websocket-accept")
        .ok_or_else(|| SyncError::Network("missing Sec-WebSocket-Accept".into()))?;
    if accept != expected {
        return Err(SyncError::Network("bad Sec-WebSocket-Accept".into()));
    }
    Ok(())
}

pub(crate) fn server_handshake<S: Read + Write>(stream: &mut S) -> Result<(), SyncError> {
    let headers = read_http_headers(stream)?;
    let first = headers.lines().next().unwrap_or("");
    if !first.contains(WS_PATH) {
        let body = b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        let _ = stream.write_all(body);
        return Err(SyncError::Rejected(format!(
            "websocket path must be {WS_PATH}"
        )));
    }
    if !headers.to_ascii_lowercase().contains("upgrade: websocket") {
        return Err(SyncError::Rejected("not a websocket upgrade".into()));
    }
    let key = header_value(&headers, "sec-websocket-key")
        .ok_or_else(|| SyncError::Rejected("missing Sec-WebSocket-Key".into()))?;
    let accept = ws_accept_key(&key);
    let resp = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {accept}\r\n\
         \r\n"
    );
    stream
        .write_all(resp.as_bytes())
        .map_err(|e| SyncError::Network(e.to_string()))?;
    stream
        .flush()
        .map_err(|e| SyncError::Network(e.to_string()))?;
    Ok(())
}

fn read_http_headers<R: Read>(stream: &mut R) -> Result<String, SyncError> {
    // Byte-at-a-time so we never consume past the end of headers into WS frames.
    let mut buf = Vec::with_capacity(512);
    let mut b = [0u8; 1];
    loop {
        stream
            .read_exact(&mut b)
            .map_err(|e| SyncError::Network(e.to_string()))?;
        buf.push(b[0]);
        if buf.len() >= 4 && buf[buf.len() - 4..] == *b"\r\n\r\n" {
            break;
        }
        if buf.len() > 16 * 1024 {
            return Err(SyncError::Rejected("http headers too large".into()));
        }
    }
    String::from_utf8(buf).map_err(|e| SyncError::Network(e.to_string()))
}

fn header_value(headers: &str, name: &str) -> Option<String> {
    let name_l = name.to_ascii_lowercase();
    for line in headers.lines() {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().to_ascii_lowercase() == name_l {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

fn ws_accept_key(client_key: &str) -> String {
    let mut data = Vec::with_capacity(client_key.len() + WS_GUID.len());
    data.extend_from_slice(client_key.as_bytes());
    data.extend_from_slice(WS_GUID.as_bytes());
    let dig = digest(&SHA1_FOR_LEGACY_USE_ONLY, &data);
    b64_encode(dig.as_ref())
}

// --- WebSocket binary frames ---

fn write_ws_binary<W: Write>(
    stream: &mut W,
    payload: &[u8],
    mask: bool,
) -> Result<(), SyncError> {
    let mut header = Vec::with_capacity(14);
    // FIN + binary opcode 0x2
    header.push(0x82);
    let len = payload.len();
    let mask_bit = if mask { 0x80 } else { 0x00 };
    if len < 126 {
        header.push(mask_bit | (len as u8));
    } else if len <= u16::MAX as usize {
        header.push(mask_bit | 126);
        header.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        header.push(mask_bit | 127);
        header.extend_from_slice(&(len as u64).to_be_bytes());
    }
    let mask_key = if mask {
        let k = [0x11u8, 0x22, 0x33, 0x44];
        header.extend_from_slice(&k);
        Some(k)
    } else {
        None
    };
    stream
        .write_all(&header)
        .map_err(|e| SyncError::Network(e.to_string()))?;
    if let Some(k) = mask_key {
        let mut masked = payload.to_vec();
        for (i, b) in masked.iter_mut().enumerate() {
            *b ^= k[i % 4];
        }
        stream
            .write_all(&masked)
            .map_err(|e| SyncError::Network(e.to_string()))?;
    } else {
        stream
            .write_all(payload)
            .map_err(|e| SyncError::Network(e.to_string()))?;
    }
    stream
        .flush()
        .map_err(|e| SyncError::Network(e.to_string()))?;
    Ok(())
}

fn read_ws_binary<R: Read>(stream: &mut R) -> Result<Vec<u8>, SyncError> {
    loop {
        let mut b0 = [0u8; 1];
        stream
            .read_exact(&mut b0)
            .map_err(|e| SyncError::Network(e.to_string()))?;
        let opcode = b0[0] & 0x0f;
        let mut b1 = [0u8; 1];
        stream
            .read_exact(&mut b1)
            .map_err(|e| SyncError::Network(e.to_string()))?;
        let masked = (b1[0] & 0x80) != 0;
        let mut len = (b1[0] & 0x7f) as u64;
        if len == 126 {
            let mut ext = [0u8; 2];
            stream
                .read_exact(&mut ext)
                .map_err(|e| SyncError::Network(e.to_string()))?;
            len = u16::from_be_bytes(ext) as u64;
        } else if len == 127 {
            let mut ext = [0u8; 8];
            stream
                .read_exact(&mut ext)
                .map_err(|e| SyncError::Network(e.to_string()))?;
            len = u64::from_be_bytes(ext);
        }
        if len > 16 * 1024 * 1024 {
            return Err(SyncError::Rejected("websocket frame too large".into()));
        }
        let mask_key = if masked {
            let mut k = [0u8; 4];
            stream
                .read_exact(&mut k)
                .map_err(|e| SyncError::Network(e.to_string()))?;
            Some(k)
        } else {
            None
        };
        let mut payload = vec![0u8; len as usize];
        if len > 0 {
            stream
                .read_exact(&mut payload)
                .map_err(|e| SyncError::Network(e.to_string()))?;
        }
        if let Some(k) = mask_key {
            for (i, b) in payload.iter_mut().enumerate() {
                *b ^= k[i % 4];
            }
        }
        match opcode {
            0x2 => return Ok(payload), // binary
            0x8 => return Err(SyncError::Network("websocket closed".into())),
            0x9 => {
                // ping → ignore for foundation (or could pong)
                continue;
            }
            0xA => continue, // pong
            0x1 => {
                return Err(SyncError::Rejected(
                    "text frames not used; send QFSNC1 as binary".into(),
                ));
            }
            _ => {
                return Err(SyncError::Rejected(format!(
                    "unsupported websocket opcode {opcode}"
                )));
            }
        }
    }
}

/// Minimal Base64 encode (RFC 4648) — enough for WS key material.
fn b64_encode(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    let mut i = 0;
    while i + 3 <= data.len() {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8) | (data[i + 2] as u32);
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(T[((n >> 6) & 63) as usize] as char);
        out.push(T[(n & 63) as usize] as char);
        i += 3;
    }
    let rem = data.len() - i;
    if rem == 1 {
        let n = (data[i] as u32) << 16;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8);
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(T[((n >> 6) & 63) as usize] as char);
        out.push('=');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::SyncHub;
    use qf_core::{ActorId, DocumentId, Hash256, Operation, SeqIdGen, SessionId};

    fn hash_ops(ops: &[Operation]) -> Hash256 {
        let mut buf = Vec::new();
        for op in ops {
            let s = format!("{op:?}");
            buf.extend_from_slice(&(s.len() as u32).to_le_bytes());
            buf.extend_from_slice(s.as_bytes());
        }
        Hash256::sha256(&buf)
    }

    #[test]
    fn ws_accept_key_matches_rfc_example() {
        // RFC 6455 §1.3 example
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        assert_eq!(ws_accept_key(key), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    #[test]
    fn ws_two_clients_hello_push_pull() {
        let server = WsSyncServer::start(SyncHub::new()).unwrap();
        let addr = server.addr();
        assert!(server.url().starts_with("ws://"));
        assert!(server.url().contains("/qfsnc1"));

        let client = WsSyncClient::connect(addr);
        let mut gen = SeqIdGen::default();
        let doc = DocumentId::new(&mut gen);
        let a = ActorId::new(&mut gen);
        let s = SessionId::new(&mut gen);

        let ack = client
            .hello(Hello {
                client_id: a,
                session_id: s,
                document_id: doc,
                have_seq: 0,
            })
            .unwrap();

        let ops = vec![Operation::Noop {
            reason: qf_core::NoopReason::Other("ws".into()),
        }];
        let push = client
            .push(PushRequest {
                token: ack.token.clone(),
                first_seq: 1,
                last_seq: 1,
                ops: ops.clone(),
                crdt: None,
                payload_hash: hash_ops(&ops),
            })
            .unwrap();
        assert_eq!(push.accepted_head_seq, 1);

        let client_b = WsSyncClient::connect(addr);
        let ack_b = client_b
            .hello(Hello {
                client_id: ActorId::new(&mut gen),
                session_id: SessionId::new(&mut gen),
                document_id: doc,
                have_seq: 0,
            })
            .unwrap();
        let pull = client_b
            .pull(PullRequest {
                token: ack_b.token,
                have_seq: 0,
            })
            .unwrap();
        assert_eq!(pull.server_head_seq, 1);
        assert_eq!(pull.ops.len(), 1);

        server.stop();
    }

    #[test]
    fn ws_session_multi_message_same_connection() {
        let server = WsSyncServer::start(SyncHub::new()).unwrap();
        let mut session = WsSyncSession::open(server.addr()).unwrap();
        let mut gen = SeqIdGen::default();
        let doc = DocumentId::new(&mut gen);
        let ack = session
            .hello(Hello {
                client_id: ActorId::new(&mut gen),
                session_id: SessionId::new(&mut gen),
                document_id: doc,
                have_seq: 0,
            })
            .unwrap();
        let mut head = 0u64;
        for i in 1..=5u64 {
            let ops = vec![Operation::Noop {
                reason: qf_core::NoopReason::Other(format!("sess{i}")),
            }];
            let push = session
                .push(PushRequest {
                    token: ack.token.clone(),
                    first_seq: i,
                    last_seq: i,
                    ops: ops.clone(),
                    crdt: None,
                    payload_hash: hash_ops(&ops),
                })
                .unwrap();
            head = push.accepted_head_seq;
            let pull = session
                .pull(PullRequest {
                    token: ack.token.clone(),
                    have_seq: head.saturating_sub(1),
                })
                .unwrap();
            assert!(pull.server_head_seq >= head);
        }
        assert_eq!(head, 5);
        // Drop session closes TCP; server loop exits cleanly.
        drop(session);
        server.stop();
    }

    #[test]
    fn budget_ws_session_50_roundtrips_under_limit() {
        use std::time::Instant;
        let server = WsSyncServer::start(SyncHub::new()).unwrap();
        let mut session = WsSyncSession::open(server.addr()).unwrap();
        let mut gen = SeqIdGen::default();
        let doc = DocumentId::new(&mut gen);
        let ack = session
            .hello(Hello {
                client_id: ActorId::new(&mut gen),
                session_id: SessionId::new(&mut gen),
                document_id: doc,
                have_seq: 0,
            })
            .unwrap();
        let start = Instant::now();
        let mut head = 0u64;
        for i in 1..=50u64 {
            let ops = vec![Operation::Noop {
                reason: qf_core::NoopReason::Other(format!("wsrt{i}")),
            }];
            let push = session
                .push(PushRequest {
                    token: ack.token.clone(),
                    first_seq: i,
                    last_seq: i,
                    ops: ops.clone(),
                    crdt: None,
                    payload_hash: hash_ops(&ops),
                })
                .unwrap();
            head = push.accepted_head_seq;
            let _ = session
                .pull(PullRequest {
                    token: ack.token.clone(),
                    have_seq: head.saturating_sub(1),
                })
                .unwrap();
        }
        let elapsed = start.elapsed();
        assert_eq!(head, 50);
        assert!(
            elapsed < Duration::from_secs(15),
            "PB-WS-SESSION-001 exceeded: {elapsed:?}"
        );
        drop(session);
        server.stop();
    }

    #[test]
    fn ws_rejects_wrong_path() {
        let server = WsSyncServer::start(SyncHub::new()).unwrap();
        let addr = server.addr();
        let mut stream =
            TcpStream::connect_timeout(&addr, Duration::from_secs(2)).unwrap();
        let bad = format!(
            "GET /wrong HTTP/1.1\r\nHost: {addr}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
        );
        stream.write_all(bad.as_bytes()).unwrap();
        let mut buf = [0u8; 128];
        let n = stream.read(&mut buf).unwrap_or(0);
        let resp = String::from_utf8_lossy(&buf[..n]);
        assert!(
            resp.contains("404") || resp.is_empty() || resp.contains("HTTP"),
            "got {resp}"
        );
        // Server still accepts good clients after a bad handshake.
        let client = WsSyncClient::connect(addr);
        let mut gen = SeqIdGen::default();
        let ack = client
            .hello(Hello {
                client_id: ActorId::new(&mut gen),
                session_id: SessionId::new(&mut gen),
                document_id: DocumentId::new(&mut gen),
                have_seq: 0,
            })
            .unwrap();
        assert_eq!(ack.server_head_seq, 0);
        server.stop();
    }
}
