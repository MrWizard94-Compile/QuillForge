//! Serve the QFSNC1 browser reference client next to a WebSocket hub.
//!
//! - [`run_browser_sync_demo`] — `http://` page + `ws://` hub  
//! - [`run_browser_wss_demo`] — `https://` page + `wss://` hub (same dev TLS identity)

use qf_sync::{SyncHub, TlsIdentity, WsSyncServer, WssSyncServer};
use rustls::ServerConfig;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const INDEX_HTML: &str = include_str!("../../../web/qfsnc1-client/index.html");
const QFSNC1_JS: &str = include_str!("../../../web/qfsnc1-client/qfsnc1.js");

fn demo_secs() -> u64 {
    std::env::var("QUILL_BROWSER_DEMO_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(120)
        .clamp(2, 600)
}

/// Plain HTTP page + plain WebSocket QFSNC1 hub.
pub fn run_browser_sync_demo() -> Result<(), String> {
    let ws = WsSyncServer::start(SyncHub::new()).map_err(|e| e.to_string())?;
    let ws_url = ws.url();
    let http = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    http.set_nonblocking(true).map_err(|e| e.to_string())?;
    let http_addr = http.local_addr().map_err(|e| e.to_string())?;
    let page_url = format!("http://{http_addr}/?ws={}", urlencoding_simple(&ws_url));

    println!("QFSNC1 WebSocket hub: {ws_url}");
    println!("Browser client:       {page_url}");
    println!("Open the URL above, click “Hello · Push · Pull”.");
    let secs = demo_secs();
    println!("Serving for ~{secs}s (set QUILL_BROWSER_DEMO_SECS to change)…");

    let stop = Arc::new(AtomicBool::new(false));
    let stop_t = stop.clone();
    let join = thread::spawn(move || plain_http_loop(http, stop_t));

    sleep_until(secs, &stop, http_addr);
    let _ = join.join();
    ws.stop();
    println!("browser-sync-demo complete");
    Ok(())
}

/// HTTPS page + WSS QFSNC1 hub sharing one self-signed [`TlsIdentity`].
///
/// Browsers will warn on the self-signed cert — accept the risk for local demos,
/// or install the printed PEM path as a trusted root for localhost.
pub fn run_browser_wss_demo() -> Result<(), String> {
    let identity = TlsIdentity::generate_dev().map_err(|e| e.to_string())?;
    let pem = identity.certificate_pem().map_err(|e| e.to_string())?;
    let pem_path = write_dev_cert_pem(&pem)?;

    let wss = WssSyncServer::start(SyncHub::new(), identity.clone()).map_err(|e| e.to_string())?;
    let wss_url = wss.url();

    let cfg = identity.server_config().map_err(|e| e.to_string())?;
    let https = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    https.set_nonblocking(true).map_err(|e| e.to_string())?;
    let https_addr = https.local_addr().map_err(|e| e.to_string())?;
    // Prefer localhost host so SAN matches the dev cert.
    let page_url = format!(
        "https://localhost:{}/?ws={}",
        https_addr.port(),
        urlencoding_simple(&wss_url.replace("127.0.0.1", "localhost"))
    );

    println!("QFSNC1 WSS hub:  {wss_url}");
    println!("Browser client:  {page_url}");
    println!("Dev certificate: {}", pem_path.display());
    println!("Note: accept the browser self-signed warning (local demo only).");
    println!("Open the HTTPS URL, click “Hello · Push · Pull”.");
    let secs = demo_secs();
    println!("Serving for ~{secs}s (set QUILL_BROWSER_DEMO_SECS to change)…");

    let stop = Arc::new(AtomicBool::new(false));
    let stop_t = stop.clone();
    let join = thread::spawn(move || https_loop(https, stop_t, cfg));

    sleep_until(secs, &stop, https_addr);
    let _ = join.join();
    wss.stop();
    println!("browser-wss-demo complete");
    Ok(())
}

fn write_dev_cert_pem(pem: &str) -> Result<PathBuf, String> {
    let dir = PathBuf::from("target/quill-browser-wss");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("dev-localhost.pem");
    std::fs::write(&path, pem).map_err(|e| e.to_string())?;
    Ok(path)
}

fn sleep_until(secs: u64, stop: &Arc<AtomicBool>, wake: SocketAddr) {
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    while std::time::Instant::now() < deadline {
        thread::sleep(Duration::from_millis(200));
    }
    stop.store(true, Ordering::SeqCst);
    let _ = TcpStream::connect_timeout(&wake, Duration::from_millis(200));
}

fn urlencoding_simple(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' | ':' | '/' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

fn plain_http_loop(listener: TcpListener, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                let _ = handle_http(&mut stream);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break,
        }
    }
}

fn https_loop(listener: TcpListener, stop: Arc<AtomicBool>, cfg: Arc<ServerConfig>) {
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                let cfg = cfg.clone();
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                // Handle inline (demo traffic is light); avoids join-handle tracking.
                let _ = handle_https_connection(stream, cfg);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break,
        }
    }
}

fn handle_https_connection(stream: TcpStream, cfg: Arc<ServerConfig>) -> Result<(), String> {
    let conn = rustls::ServerConnection::new(cfg).map_err(|e| e.to_string())?;
    let mut tls = rustls::StreamOwned::new(conn, stream);
    handle_http(&mut tls)
}

fn handle_http<S: Read + Write>(stream: &mut S) -> Result<(), String> {
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
    if n == 0 {
        return Ok(());
    }
    let req = String::from_utf8_lossy(&buf[..n]);
    let first = req.lines().next().unwrap_or("");
    let path = first.split_whitespace().nth(1).unwrap_or("/");

    let (status, ctype, body): (&str, &str, Vec<u8>) = if path.starts_with("/qfsnc1.js") {
        (
            "200 OK",
            "application/javascript; charset=utf-8",
            QFSNC1_JS.as_bytes().to_vec(),
        )
    } else if path == "/" || path.starts_with("/?") || path.starts_with("/index") {
        (
            "200 OK",
            "text/html; charset=utf-8",
            INDEX_HTML.as_bytes().to_vec(),
        )
    } else if path.starts_with("/health") {
        ("200 OK", "text/plain; charset=utf-8", b"ok".to_vec())
    } else {
        (
            "404 Not Found",
            "text/plain; charset=utf-8",
            b"not found".to_vec(),
        )
    };

    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
        body.len()
    );
    stream
        .write_all(header.as_bytes())
        .map_err(|e| e.to_string())?;
    stream.write_all(&body).map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustls::pki_types::ServerName;
    use std::io::Write;

    #[test]
    fn embedded_assets_present() {
        assert!(INDEX_HTML.contains("QFSNC1"));
        assert!(INDEX_HTML.contains("Quillforge"));
        assert!(!INDEX_HTML.to_ascii_lowercase().contains("microsoft"));
        assert!(QFSNC1_JS.contains("QFSNC1"));
        assert!(QFSNC1_JS.contains("selfTest"));
        assert!(QFSNC1_JS.contains("a564747970656568656c6c6f"));
    }

    #[test]
    fn urlencoding_keeps_ws_and_wss() {
        let u = urlencoding_simple("ws://127.0.0.1:9/qfsnc1");
        assert!(u.contains("ws://"));
        let s = urlencoding_simple("wss://localhost:9/qfsnc1");
        assert!(s.contains("wss://"));
        assert!(s.contains("/qfsnc1"));
    }

    #[test]
    fn https_serves_index_and_js() {
        let identity = TlsIdentity::generate_dev().unwrap();
        let cfg = identity.server_config().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_t = stop.clone();
        let join = thread::spawn(move || https_loop(listener, stop_t, cfg));

        // Client with rustls trusting the same identity.
        let client_cfg = identity.client_config().unwrap();
        let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let name = ServerName::try_from("localhost").unwrap().to_owned();
        let conn = rustls::ClientConnection::new(client_cfg, name).unwrap();
        let mut tls = rustls::StreamOwned::new(conn, stream);
        tls.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut resp = Vec::new();
        let mut buf = [0u8; 2048];
        loop {
            match tls.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => resp.extend_from_slice(&buf[..n]),
                Err(_) => break,
            }
            if resp.len() > 64 && resp.windows(4).any(|w| w == b"\r\n\r\n") && resp.len() > 200 {
                // enough of body for check
                if resp.len() > 800 {
                    break;
                }
            }
        }
        let text = String::from_utf8_lossy(&resp);
        assert!(text.contains("200 OK"), "got {text}");
        assert!(
            text.contains("QFSNC1") || text.contains("Quillforge"),
            "got {text}"
        );

        // Second request for JS asset.
        let stream2 = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).unwrap();
        stream2
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        stream2
            .set_write_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let name2 = ServerName::try_from("localhost").unwrap().to_owned();
        let conn2 =
            rustls::ClientConnection::new(identity.client_config().unwrap(), name2).unwrap();
        let mut tls2 = rustls::StreamOwned::new(conn2, stream2);
        tls2.write_all(b"GET /qfsnc1.js HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut resp2 = Vec::new();
        let mut buf2 = [0u8; 4096];
        loop {
            match tls2.read(&mut buf2) {
                Ok(0) => break,
                Ok(n) => resp2.extend_from_slice(&buf2[..n]),
                Err(_) => break,
            }
            if resp2.len() > 500 {
                break;
            }
        }
        let js = String::from_utf8_lossy(&resp2);
        assert!(js.contains("selfTest") || js.contains("QFSNC1"), "got {js}");

        stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect_timeout(&addr, Duration::from_millis(200));
        let _ = join.join();
    }

    #[test]
    fn certificate_pem_export_round_trips_header() {
        let identity = TlsIdentity::generate_dev().unwrap();
        let pem = identity.certificate_pem().unwrap();
        assert!(pem.contains("BEGIN CERTIFICATE"));
        assert!(pem.contains("END CERTIFICATE"));
    }
}
