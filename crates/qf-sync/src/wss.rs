//! Secure WebSocket (`wss://`) transport for QFSNC1.
//!
//! Stack: TCP → rustls TLS 1.3 → RFC 6455 upgrade → binary frames carrying QFSNC1.
//! Path remains `/qfsnc1`. Uses the same [`SyncHub`] as plain TCP / TLS / `ws://`.

use crate::hub::{Hello, HelloAck, PullRequest, PullResponse, PushAck, PushRequest, SyncHub};
use crate::tls::{ensure_crypto_provider, TlsIdentity};
use crate::wire::{map_wire_err, WireRequest, WireResponse, PROTOCOL_ID};
use crate::ws::{
    client_handshake, read_qfsnc1_ws, serve_session_ws, server_handshake, write_qfsnc1_ws,
};
use crate::SyncError;
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ServerConfig};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// TLS-wrapped WebSocket server (`wss://host:port/qfsnc1`).
pub struct WssSyncServer {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    hub: Arc<Mutex<SyncHub>>,
    identity: TlsIdentity,
}

impl WssSyncServer {
    pub fn start(hub: SyncHub, identity: TlsIdentity) -> Result<Self, SyncError> {
        ensure_crypto_provider();
        let listener =
            TcpListener::bind("127.0.0.1:0").map_err(|e| SyncError::Network(e.to_string()))?;
        let addr = listener
            .local_addr()
            .map_err(|e| SyncError::Network(e.to_string()))?;
        let hub = Arc::new(Mutex::new(hub));
        let stop = Arc::new(AtomicBool::new(false));
        let hub_t = hub.clone();
        let stop_t = stop.clone();
        let cfg = identity.server_config()?;
        listener
            .set_nonblocking(true)
            .map_err(|e| SyncError::Network(e.to_string()))?;
        let join = thread::spawn(move || accept_loop_wss(listener, hub_t, stop_t, cfg));
        thread::sleep(Duration::from_millis(40));
        Ok(Self {
            addr,
            stop,
            join: Some(join),
            hub,
            identity,
        })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn hub(&self) -> Arc<Mutex<SyncHub>> {
        self.hub.clone()
    }

    pub fn identity(&self) -> &TlsIdentity {
        &self.identity
    }

    pub fn protocol_id(&self) -> &'static str {
        PROTOCOL_ID
    }

    /// Browser-oriented URL (TLS + WebSocket).
    pub fn url(&self) -> String {
        format!("wss://{}/qfsnc1", self.addr)
    }

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect_timeout(&self.addr, Duration::from_millis(200));
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

impl Drop for WssSyncServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect_timeout(&self.addr, Duration::from_millis(100));
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

fn accept_loop_wss(
    listener: TcpListener,
    hub: Arc<Mutex<SyncHub>>,
    stop: Arc<AtomicBool>,
    cfg: Arc<ServerConfig>,
) {
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                let hub_c = hub.clone();
                let cfg_c = cfg.clone();
                thread::spawn(move || {
                    let _ = handle_wss_connection(stream, &hub_c, &cfg_c);
                });
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(_) => break,
        }
    }
}

fn handle_wss_connection(
    stream: TcpStream,
    hub: &Arc<Mutex<SyncHub>>,
    cfg: &Arc<ServerConfig>,
) -> Result<(), SyncError> {
    stream
        .set_nonblocking(false)
        .map_err(|e| SyncError::Network(e.to_string()))?;
    // Short read timeout so live notify flush works between requests (same as plain WS).
    stream
        .set_read_timeout(Some(Duration::from_millis(50)))
        .map_err(|e| SyncError::Network(e.to_string()))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| SyncError::Network(e.to_string()))?;
    let conn =
        rustls::ServerConnection::new(cfg.clone()).map_err(|e| SyncError::Auth(e.to_string()))?;
    let mut tls = rustls::StreamOwned::new(conn, stream);
    server_handshake(&mut tls)?;
    serve_session_ws(&mut tls, hub, false)
}

/// One-shot client for QFSNC1 over `wss://`. Prefer [`WssSyncSession`] for multi-message use.
pub struct WssSyncClient {
    addr: SocketAddr,
    config: Arc<ClientConfig>,
    server_name: ServerName<'static>,
}

impl WssSyncClient {
    /// Connect trusting `identity` roots (dev self-signed chain).
    pub fn connect(addr: SocketAddr, identity: &TlsIdentity) -> Result<Self, SyncError> {
        ensure_crypto_provider();
        let config = identity.client_config()?;
        let server_name = ServerName::try_from("localhost")
            .map_err(|e| SyncError::Auth(e.to_string()))?
            .to_owned();
        Ok(Self {
            addr,
            config,
            server_name,
        })
    }

    /// Open a long-lived WSS session (TLS + WS handshake once).
    pub fn open_session(&self) -> Result<WssSyncSession, SyncError> {
        WssSyncSession::open(self.addr, self.config.clone(), self.server_name.clone())
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

/// Persistent `wss://` connection for multiple QFSNC1 messages.
pub struct WssSyncSession {
    tls: rustls::StreamOwned<rustls::ClientConnection, TcpStream>,
}

impl WssSyncSession {
    pub fn open(
        addr: SocketAddr,
        config: Arc<ClientConfig>,
        server_name: ServerName<'static>,
    ) -> Result<Self, SyncError> {
        let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(3))
            .map_err(|e| SyncError::Network(e.to_string()))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(|e| SyncError::Network(e.to_string()))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(30)))
            .map_err(|e| SyncError::Network(e.to_string()))?;
        let conn = rustls::ClientConnection::new(config, server_name)
            .map_err(|e| SyncError::Auth(e.to_string()))?;
        let mut tls = rustls::StreamOwned::new(conn, stream);
        let host = format!("{addr}");
        client_handshake(&mut tls, &host)?;
        Ok(Self { tls })
    }

    pub fn hello(&mut self, req: Hello) -> Result<HelloAck, SyncError> {
        match self.round_trip(WireRequest::Hello(req))? {
            WireResponse::HelloAck(a) => Ok(a),
            WireResponse::Error { kind, message } => Err(map_wire_err(kind, message)),
            other => Err(SyncError::Network(format!(
                "unexpected response: {other:?}"
            ))),
        }
    }

    pub fn pull(&mut self, req: PullRequest) -> Result<PullResponse, SyncError> {
        match self.round_trip(WireRequest::Pull(req))? {
            WireResponse::Pull(p) => Ok(p),
            WireResponse::Error { kind, message } => Err(map_wire_err(kind, message)),
            other => Err(SyncError::Network(format!(
                "unexpected response: {other:?}"
            ))),
        }
    }

    pub fn push(&mut self, req: PushRequest) -> Result<PushAck, SyncError> {
        match self.round_trip(WireRequest::Push(req))? {
            WireResponse::PushAck(a) => Ok(a),
            WireResponse::Error { kind, message } => Err(map_wire_err(kind, message)),
            other => Err(SyncError::Network(format!(
                "unexpected response: {other:?}"
            ))),
        }
    }

    fn round_trip(&mut self, req: WireRequest) -> Result<WireResponse, SyncError> {
        write_qfsnc1_ws(&mut self.tls, &req, true)?;
        read_qfsnc1_ws(&mut self.tls)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::SyncHub;
    use qf_core::{ActorId, DocumentId, Hash256, Operation, SeqIdGen, SessionId};
    use std::io::Write;

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
    fn wss_hello_push_pull() {
        let identity = TlsIdentity::generate_dev().unwrap();
        let server = WssSyncServer::start(SyncHub::new(), identity.clone()).unwrap();
        assert!(server.url().starts_with("wss://"));
        assert!(server.url().ends_with("/qfsnc1"));
        let client = WssSyncClient::connect(server.addr(), &identity).unwrap();
        let mut gen = SeqIdGen::default();
        let doc = DocumentId::new(&mut gen);

        let mut last_err = None;
        for _ in 0..8 {
            match client.hello(Hello {
                client_id: ActorId::new(&mut gen),
                session_id: SessionId::new(&mut gen),
                document_id: doc,
                have_seq: 0,
            }) {
                Ok(ack) => {
                    let ops = vec![Operation::Noop {
                        reason: qf_core::NoopReason::Other("wss".into()),
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
                    let client_b = WssSyncClient::connect(server.addr(), &identity).unwrap();
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
                    assert_eq!(server.protocol_id(), "QFSNC1");
                    server.stop();
                    return;
                }
                Err(e) => {
                    last_err = Some(e);
                    thread::sleep(Duration::from_millis(50));
                }
            }
        }
        panic!("wss_hello_push_pull failed: {last_err:?}");
    }

    #[test]
    fn wss_session_multi_message_same_connection() {
        let identity = TlsIdentity::generate_dev().unwrap();
        let server = WssSyncServer::start(SyncHub::new(), identity.clone()).unwrap();
        let client = WssSyncClient::connect(server.addr(), &identity).unwrap();
        let mut session = client.open_session().unwrap();
        let mut gen = SeqIdGen::default();
        let doc = DocumentId::new(&mut gen);
        let mut last_err = None;
        for _ in 0..8 {
            match session.hello(Hello {
                client_id: ActorId::new(&mut gen),
                session_id: SessionId::new(&mut gen),
                document_id: doc,
                have_seq: 0,
            }) {
                Ok(ack) => {
                    for i in 1..=3u64 {
                        let ops = vec![Operation::Noop {
                            reason: qf_core::NoopReason::Other(format!("wsss{i}")),
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
                        assert_eq!(push.accepted_head_seq, i);
                    }
                    let pull = session
                        .pull(PullRequest {
                            token: ack.token,
                            have_seq: 0,
                        })
                        .unwrap();
                    assert_eq!(pull.server_head_seq, 3);
                    drop(session);
                    server.stop();
                    return;
                }
                Err(e) => {
                    last_err = Some(e);
                    session = client.open_session().unwrap();
                    thread::sleep(Duration::from_millis(40));
                }
            }
        }
        panic!("wss session failed: {last_err:?}");
    }

    #[test]
    fn wss_still_serves_after_plaintext_peer() {
        let identity = TlsIdentity::generate_dev().unwrap();
        let server = WssSyncServer::start(SyncHub::new(), identity.clone()).unwrap();
        let addr = server.addr();
        {
            let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).unwrap();
            let _ = stream.write_all(b"GET / HTTP/1.1\r\n\r\n");
        }
        thread::sleep(Duration::from_millis(40));
        let client = WssSyncClient::connect(addr, &identity).unwrap();
        let mut gen = SeqIdGen::default();
        let mut ok = false;
        for _ in 0..8 {
            if client
                .hello(Hello {
                    client_id: ActorId::new(&mut gen),
                    session_id: SessionId::new(&mut gen),
                    document_id: DocumentId::new(&mut gen),
                    have_seq: 0,
                })
                .is_ok()
            {
                ok = true;
                break;
            }
            thread::sleep(Duration::from_millis(40));
        }
        assert!(ok, "WSS server failed after plaintext peer");
        server.stop();
    }
}
