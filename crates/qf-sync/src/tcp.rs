//! Plain TCP transport for QFSNC1 framed protocol over [`SyncHub`].
//!
//! See [`crate::wire`] for frame layout. TLS wraps the same protocol in [`crate::tls`].

use crate::hub::{
    DrainNotifyRequest, Hello, HelloAck, NotifyBatch, PullRequest, PullResponse, PushAck,
    PushRequest, SubscribeRequest, Subscribed, SyncHub,
};
use crate::wire::{map_wire_err, read_frame, serve_one, write_frame, WireRequest, WireResponse};
use crate::SyncError;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

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

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
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
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| SyncError::Network(e.to_string()))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| SyncError::Network(e.to_string()))?;
    serve_one(&mut stream, hub)
}

/// Client that speaks the framed protocol over plain TCP.
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
            other => Err(SyncError::Network(format!(
                "unexpected response: {other:?}"
            ))),
        }
    }

    pub fn pull(&self, req: PullRequest) -> Result<PullResponse, SyncError> {
        match self.round_trip(WireRequest::Pull(req))? {
            WireResponse::Pull(p) => Ok(p),
            WireResponse::Error { kind, message } => Err(map_wire_err(kind, message)),
            other => Err(SyncError::Network(format!(
                "unexpected response: {other:?}"
            ))),
        }
    }

    pub fn push(&self, req: PushRequest) -> Result<PushAck, SyncError> {
        match self.round_trip(WireRequest::Push(req))? {
            WireResponse::PushAck(a) => Ok(a),
            WireResponse::Error { kind, message } => Err(map_wire_err(kind, message)),
            other => Err(SyncError::Network(format!(
                "unexpected response: {other:?}"
            ))),
        }
    }

    pub fn subscribe(&self, req: SubscribeRequest) -> Result<Subscribed, SyncError> {
        match self.round_trip(WireRequest::Subscribe(req))? {
            WireResponse::Subscribed(s) => Ok(s),
            WireResponse::Error { kind, message } => Err(map_wire_err(kind, message)),
            other => Err(SyncError::Network(format!(
                "unexpected response: {other:?}"
            ))),
        }
    }

    pub fn drain_notify(&self, req: DrainNotifyRequest) -> Result<NotifyBatch, SyncError> {
        match self.round_trip(WireRequest::DrainNotify(req))? {
            WireResponse::Notify(n) => Ok(n),
            WireResponse::Error { kind, message } => Err(map_wire_err(kind, message)),
            other => Err(SyncError::Network(format!(
                "unexpected response: {other:?}"
            ))),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::SyncHub;
    use qf_collab::TextCrdt;
    use qf_core::{ActorId, DocumentId, Hash256, Operation, SeqIdGen, SessionId};
    use std::io::{Read, Write};
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
    fn tcp_subscribe_notify_on_peer_push() {
        let server = TcpSyncServer::start(SyncHub::new()).unwrap();
        let addr = server.addr();
        let mut gen = SeqIdGen::default();
        let doc = DocumentId::new(&mut gen);

        let a = TcpSyncClient::connect(addr);
        let ack_a = a
            .hello(Hello {
                client_id: ActorId::new(&mut gen),
                session_id: SessionId::new(&mut gen),
                document_id: doc,
                have_seq: 0,
            })
            .unwrap();

        let b = TcpSyncClient::connect(addr);
        let ack_b = b
            .hello(Hello {
                client_id: ActorId::new(&mut gen),
                session_id: SessionId::new(&mut gen),
                document_id: doc,
                have_seq: 0,
            })
            .unwrap();
        b.subscribe(SubscribeRequest {
            token: ack_b.token.clone(),
        })
        .unwrap();

        let ops = vec![Operation::Noop {
            reason: qf_core::NoopReason::Other("notify".into()),
        }];
        a.push(PushRequest {
            token: ack_a.token,
            first_seq: 1,
            last_seq: 1,
            ops: ops.clone(),
            crdt: None,
            payload_hash: hash_ops(&ops),
        })
        .unwrap();

        let batch = b
            .drain_notify(DrainNotifyRequest { token: ack_b.token })
            .unwrap();
        assert_eq!(batch.events.len(), 1);
        assert_eq!(batch.events[0].server_head_seq, 1);
        assert_eq!(batch.events[0].document_id, doc);

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
            let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).unwrap();
            let _ = stream.write_all(b"BADMAGIC");
            let _ = stream.write_all(&4u32.to_le_bytes());
            let _ = stream.write_all(b"xxxx");
            let mut buf = [0u8; 64];
            let _ = stream.read(&mut buf);
        }
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
