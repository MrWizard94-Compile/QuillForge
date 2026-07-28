//! TLS 1.3 transport adapter for QFSNC1 (rustls).
//!
//! Same framed CBOR protocol as plain TCP ([`crate::wire`]), encrypted with TLS.
//! Development material uses self-signed certificates ([`TlsIdentity::generate_dev`]).
//! Production deployments should supply CA-signed DER via [`TlsIdentity::from_der`].

use crate::hub::{
    Hello, HelloAck, PullRequest, PullResponse, PushAck, PushRequest, SyncHub,
};
use crate::wire::{
    map_wire_err, read_frame, serve_one, write_frame, WireRequest, WireResponse, PROTOCOL_ID,
};
use crate::SyncError;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName};
use rustls::{ClientConfig, RootCertStore, ServerConfig};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::thread::{self, JoinHandle};
use std::time::Duration;

static INSTALL_PROVIDER: Once = Once::new();

fn ensure_crypto_provider() {
    INSTALL_PROVIDER.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// Certificate + private key material for server (and matching client trust).
pub struct TlsIdentity {
    certs: Vec<CertificateDer<'static>>,
    /// PKCS#8 DER bytes (cloneable storage).
    key_pkcs8: Vec<u8>,
}

impl Clone for TlsIdentity {
    fn clone(&self) -> Self {
        Self {
            certs: self.certs.clone(),
            key_pkcs8: self.key_pkcs8.clone(),
        }
    }
}

impl TlsIdentity {
    /// Build identity from DER certificate chain + PKCS#8 private key bytes.
    pub fn from_pkcs8(
        certs: Vec<CertificateDer<'static>>,
        key_pkcs8: Vec<u8>,
    ) -> Result<Self, SyncError> {
        if certs.is_empty() {
            return Err(SyncError::Auth("empty certificate chain".into()));
        }
        if key_pkcs8.is_empty() {
            return Err(SyncError::Auth("empty private key".into()));
        }
        Ok(Self { certs, key_pkcs8 })
    }

    /// Self-signed identity for localhost demos and tests (SAN: localhost, 127.0.0.1).
    pub fn generate_dev() -> Result<Self, SyncError> {
        ensure_crypto_provider();
        let params = rcgen::CertificateParams::new(vec![
            "localhost".into(),
            "127.0.0.1".into(),
        ])
        .map_err(|e| SyncError::Network(e.to_string()))?;
        let key_pair =
            rcgen::KeyPair::generate().map_err(|e| SyncError::Network(e.to_string()))?;
        let cert = params
            .self_signed(&key_pair)
            .map_err(|e| SyncError::Network(e.to_string()))?;
        let cert_der = CertificateDer::from(cert.der().to_vec());
        Self::from_pkcs8(vec![cert_der], key_pair.serialize_der())
    }

    fn private_key(&self) -> PrivateKeyDer<'static> {
        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key_pkcs8.clone()))
    }

    fn server_config(&self) -> Result<Arc<ServerConfig>, SyncError> {
        ensure_crypto_provider();
        let cfg = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(self.certs.clone(), self.private_key())
            .map_err(|e| SyncError::Auth(e.to_string()))?;
        Ok(Arc::new(cfg))
    }

    fn client_config(&self) -> Result<Arc<ClientConfig>, SyncError> {
        ensure_crypto_provider();
        let mut roots = RootCertStore::empty();
        for cert in &self.certs {
            roots
                .add(cert.clone())
                .map_err(|e| SyncError::Auth(e.to_string()))?;
        }
        let cfg = ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        Ok(Arc::new(cfg))
    }
}

/// TLS server speaking QFSNC1 over rustls.
pub struct TlsSyncServer {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    hub: Arc<Mutex<SyncHub>>,
    identity: TlsIdentity,
}

impl TlsSyncServer {
    pub fn start(hub: SyncHub, identity: TlsIdentity) -> Result<Self, SyncError> {
        ensure_crypto_provider();
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|e| SyncError::Network(e.to_string()))?;
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
        let join = thread::spawn(move || accept_loop_tls(listener, hub_t, stop_t, cfg));
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

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect_timeout(&self.addr, Duration::from_millis(200));
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

impl Drop for TlsSyncServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect_timeout(&self.addr, Duration::from_millis(100));
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

fn accept_loop_tls(
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
                let _ = handle_tls_connection(stream, &hub, &cfg);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(_) => break,
        }
    }
}

fn handle_tls_connection(
    stream: TcpStream,
    hub: &Arc<Mutex<SyncHub>>,
    cfg: &Arc<ServerConfig>,
) -> Result<(), SyncError> {
    // Listener is non-blocking; force accepted sockets blocking for TLS I/O.
    stream
        .set_nonblocking(false)
        .map_err(|e| SyncError::Network(e.to_string()))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| SyncError::Network(e.to_string()))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| SyncError::Network(e.to_string()))?;
    let conn = rustls::ServerConnection::new(cfg.clone())
        .map_err(|e| SyncError::Auth(e.to_string()))?;
    let mut tls = rustls::StreamOwned::new(conn, stream);
    serve_one(&mut tls, hub)
}

/// TLS client for QFSNC1.
pub struct TlsSyncClient {
    addr: SocketAddr,
    config: Arc<ClientConfig>,
    server_name: ServerName<'static>,
}

impl TlsSyncClient {
    /// Connect to `addr` trusting `identity` roots (dev self-signed chain).
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
        let stream = TcpStream::connect_timeout(&self.addr, Duration::from_secs(3))
            .map_err(|e| SyncError::Network(e.to_string()))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| SyncError::Network(e.to_string()))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| SyncError::Network(e.to_string()))?;
        let conn = rustls::ClientConnection::new(self.config.clone(), self.server_name.clone())
            .map_err(|e| SyncError::Auth(e.to_string()))?;
        let mut tls = rustls::StreamOwned::new(conn, stream);
        write_frame(&mut tls, &req)?;
        read_frame(&mut tls)
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
    fn tls_hello_push_pull_over_rustls() {
        let identity = TlsIdentity::generate_dev().unwrap();
        let server = TlsSyncServer::start(SyncHub::new(), identity.clone()).unwrap();
        let client = TlsSyncClient::connect(server.addr(), &identity).unwrap();
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
                        reason: qf_core::NoopReason::Other("tls".into()),
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
                    let pull = client
                        .pull(PullRequest {
                            token: ack.token,
                            have_seq: 0,
                        })
                        .unwrap();
                    assert_eq!(pull.server_head_seq, 1);
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
        panic!("tls_hello_push_pull failed: {last_err:?}");
    }

    #[test]
    fn tls_still_serves_after_plaintext_peer() {
        let identity = TlsIdentity::generate_dev().unwrap();
        let server = TlsSyncServer::start(SyncHub::new(), identity.clone()).unwrap();
        let addr = server.addr();
        {
            let mut stream =
                TcpStream::connect_timeout(&addr, Duration::from_secs(2)).unwrap();
            let _ = stream.write_all(crate::wire::MAGIC);
            let _ = stream.write_all(&4u32.to_le_bytes());
            let _ = stream.write_all(b"xxxx");
        }
        thread::sleep(Duration::from_millis(30));
        let client = TlsSyncClient::connect(addr, &identity).unwrap();
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
        assert!(ok, "TLS server failed after plaintext peer");
        server.stop();
    }
}
