//! Localhost TCP framed transport for [`SyncHub`].
//!
//! Foundation network residual: real sockets over loopback with a fixed binary
//! frame + CBOR body. CBOR is required so CRDT maps with structured keys round-trip
//! (JSON object keys must be strings). TLS/WebSocket remain a thin adapter later.
//!
//! # Frame format
//! ```text
//! magic:  b"QFSNC1\0\0"   (8 bytes)
//! length: u32 LE          (payload bytes)
//! payload: CBOR WireRequest / WireResponse
//! ```
//!
//! One request/response per connection (short-lived) to keep the server simple
//! and deterministic under tests.

use crate::hub::{
    Hello, HelloAck, PullRequest, PullResponse, PushAck, PushRequest, SyncHub,
};
use crate::SyncError;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const MAGIC: &[u8; 8] = b"QFSNC1\0\0";
const MAX_FRAME: u32 = 16 * 1024 * 1024; // 16 MiB untrusted input budget

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WireRequest {
    Hello(Hello),
    Pull(PullRequest),
    Push(PushRequest),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WireResponse {
    HelloAck(HelloAck),
    Pull(PullResponse),
    PushAck(PushAck),
    Error { kind: String, message: String },
}

/// Background TCP server wrapping a shared [`SyncHub`].
pub struct TcpSyncServer {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    hub: Arc<Mutex<SyncHub>>,
}

impl TcpSyncServer {
    /// Bind `127.0.0.1:0` (ephemeral port) and spawn accept loop.
    pub fn start(hub: SyncHub) -> Result<Self, SyncError> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|e| SyncError::Network(e.to_string()))?;
        listener
            .set_nonblocking(false)
            .map_err(|e| SyncError::Network(e.to_string()))?;
        let addr = listener
            .local_addr()
            .map_err(|e| SyncError::Network(e.to_string()))?;
        let hub = Arc::new(Mutex::new(hub));
        let stop = Arc::new(AtomicBool::new(false));
        let hub_t = hub.clone();
        let stop_t = stop.clone();
        // Accept with timeout so stop can be observed.
        listener
            .set_nonblocking(true)
            .map_err(|e| SyncError::Network(e.to_string()))?;
        let join = thread::spawn(move || accept_loop(listener, hub_t, stop_t));
        // Brief settle so accept loop is polling before clients connect (Windows).
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

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake accept by connecting once.
        let _ = TcpStream::connect_timeout(&self.addr, Duration::from_millis(200));
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

impl Drop for TcpSyncServer {
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
                let hub = hub.clone();
                // Handle request on the accept thread (tests are low concurrency).
                // Production adapter may spawn; keep foundation deterministic.
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
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| SyncError::Network(e.to_string()))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| SyncError::Network(e.to_string()))?;
    let resp = match read_frame::<WireRequest>(&mut stream) {
        Ok(req) => dispatch(hub, req),
        Err(e) => err_resp(e),
    };
    write_frame(&mut stream, &resp)?;
    Ok(())
}

fn dispatch(hub: &Arc<Mutex<SyncHub>>, req: WireRequest) -> WireResponse {
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
    }
}

fn err_resp(e: SyncError) -> WireResponse {
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

/// Client that speaks the framed protocol over TCP.
pub struct TcpSyncClient {
    addr: SocketAddr,
}

impl TcpSyncClient {
    pub fn connect(addr: SocketAddr) -> Self {
        Self { addr }
    }

    pub fn hello(&self, req: Hello) -> Result<HelloAck, SyncError> {
        match self.round_trip(WireRequest::Hello(req))? {
            WireResponse::HelloAck(a) => Ok(a),
            WireResponse::Error { kind, message } => Err(map_wire_err(kind, message)),
            other => Err(SyncError::Network(format!("unexpected response: {other:?}"))),
        }
    }

    pub fn pull(&self, req: PullRequest) -> Result<PullResponse, SyncError> {
        match self.round_trip(WireRequest::Pull(req))? {
            WireResponse::Pull(p) => Ok(p),
            WireResponse::Error { kind, message } => Err(map_wire_err(kind, message)),
            other => Err(SyncError::Network(format!("unexpected response: {other:?}"))),
        }
    }

    pub fn push(&self, req: PushRequest) -> Result<PushAck, SyncError> {
        match self.round_trip(WireRequest::Push(req))? {
            WireResponse::PushAck(a) => Ok(a),
            WireResponse::Error { kind, message } => Err(map_wire_err(kind, message)),
            other => Err(SyncError::Network(format!("unexpected response: {other:?}"))),
        }
    }

    fn round_trip(&self, req: WireRequest) -> Result<WireResponse, SyncError> {
        let mut stream = TcpStream::connect_timeout(&self.addr, Duration::from_secs(2))
            .map_err(|e| SyncError::Network(e.to_string()))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| SyncError::Network(e.to_string()))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| SyncError::Network(e.to_string()))?;
        write_frame(&mut stream, &req)?;
        read_frame(&mut stream)
    }
}

fn map_wire_err(kind: String, message: String) -> SyncError {
    match kind.as_str() {
        "auth" => SyncError::Auth(message),
        "rejected" => SyncError::Rejected(message),
        "stream_mismatch" => SyncError::StreamMismatch(message),
        _ => SyncError::Network(format!("{kind}: {message}")),
    }
}

fn write_frame<T: Serialize>(stream: &mut TcpStream, value: &T) -> Result<(), SyncError> {
    let mut payload = Vec::new();
    ciborium::into_writer(value, &mut payload)
        .map_err(|e| SyncError::Network(e.to_string()))?;
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

fn read_frame<T: for<'de> Deserialize<'de>>(stream: &mut TcpStream) -> Result<T, SyncError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::SyncHub;
    use qf_collab::TextCrdt;
    use qf_core::{ActorId, DocumentId, Hash256, Operation, SeqIdGen, SessionId};
    use std::time::Instant;

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
    fn tcp_two_clients_hello_push_pull() {
        let server = TcpSyncServer::start(SyncHub::new()).unwrap();
        let addr = server.addr();
        let client = TcpSyncClient::connect(addr);

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
            reason: qf_core::NoopReason::Other("tcp".into()),
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

        // Second client
        let client_b = TcpSyncClient::connect(addr);
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
    fn tcp_crdt_push_over_socket() {
        let server = TcpSyncServer::start(SyncHub::new()).unwrap();
        let client = TcpSyncClient::connect(server.addr());
        let mut gen = SeqIdGen::default();
        let doc = DocumentId::new(&mut gen);
        let mut last_err = None;
        for _ in 0..5 {
            match client.hello(Hello {
                client_id: ActorId::new(&mut gen),
                session_id: SessionId::new(&mut gen),
                document_id: doc,
                have_seq: 0,
            }) {
                Ok(ack) => {
                    let mut crdt = TextCrdt::new(7);
                    crdt.local_insert(0, "net");
                    client
                        .push(PushRequest {
                            token: ack.token,
                            first_seq: 1,
                            last_seq: 1,
                            ops: vec![],
                            crdt: Some(crdt),
                            payload_hash: Hash256::ZERO,
                        })
                        .unwrap();
                    assert_eq!(server.hub().lock().unwrap().head_seq(doc), 1);
                    server.stop();
                    return;
                }
                Err(e) => {
                    last_err = Some(e);
                    thread::sleep(Duration::from_millis(30));
                }
            }
        }
        panic!("tcp_crdt_push_over_socket failed after retries: {last_err:?}");
    }

    /// PB-SYNC-001 — 50 TCP Hello/Push/Pull round-trips under CI budget.
    #[test]
    fn budget_tcp_50_roundtrips_under_limit() {
        let server = TcpSyncServer::start(SyncHub::new()).unwrap();
        let client = TcpSyncClient::connect(server.addr());
        let mut gen = SeqIdGen::default();
        let doc = DocumentId::new(&mut gen);
        let start = Instant::now();
        let mut head = 0u64;
        for i in 1..=50u64 {
            let mut last_err = None;
            let mut ok = false;
            for _attempt in 0..8 {
                match client.hello(Hello {
                    client_id: ActorId::new(&mut gen),
                    session_id: SessionId::new(&mut gen),
                    document_id: doc,
                    have_seq: head,
                }) {
                    Ok(ack) => {
                        let ops = vec![Operation::Noop {
                            reason: qf_core::NoopReason::Other(format!("rt{i}")),
                        }];
                        let payload_hash = hash_ops(&ops);
                        match client.push(PushRequest {
                            token: ack.token.clone(),
                            first_seq: i,
                            last_seq: i,
                            ops,
                            crdt: None,
                            payload_hash,
                        }) {
                            Ok(push) => {
                                head = push.accepted_head_seq;
                                let pull = client
                                    .pull(PullRequest {
                                        token: ack.token,
                                        have_seq: head.saturating_sub(1),
                                    })
                                    .unwrap();
                                assert!(pull.server_head_seq >= head);
                                ok = true;
                                break;
                            }
                            Err(e) => last_err = Some(e),
                        }
                    }
                    Err(e) => last_err = Some(e),
                }
                thread::sleep(Duration::from_millis(20));
            }
            assert!(ok, "round-trip {i} failed: {last_err:?}");
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_secs(15),
            "PB-SYNC-001 exceeded: {elapsed:?}"
        );
        server.stop();
    }

    #[test]
    fn tcp_rejects_bad_magic_without_crashing_server() {
        let server = TcpSyncServer::start(SyncHub::new()).unwrap();
        let addr = server.addr();
        {
            let mut stream =
                TcpStream::connect_timeout(&addr, Duration::from_secs(2)).unwrap();
            // Best-effort bad frame; host may abort mid-write after server rejects.
            let _ = stream.write_all(b"BADMAGIC");
            let _ = stream.write_all(&4u32.to_le_bytes());
            let _ = stream.write_all(b"xxxx");
            let mut buf = [0u8; 64];
            let _ = stream.read(&mut buf);
        }
        // Server still serves well-formed traffic after a bad peer.
        let client = TcpSyncClient::connect(addr);
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
