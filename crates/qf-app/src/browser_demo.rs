//! Serve the QFSNC1 browser reference client next to a `WsSyncServer`.

use qf_sync::{SyncHub, WsSyncServer};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const INDEX_HTML: &str = include_str!("../../../web/qfsnc1-client/index.html");
const QFSNC1_JS: &str = include_str!("../../../web/qfsnc1-client/qfsnc1.js");

/// Run until Ctrl+C is impractical in demos — serve for a fixed window or until stop.
pub fn run_browser_sync_demo() -> Result<(), String> {
    let ws = WsSyncServer::start(SyncHub::new()).map_err(|e| e.to_string())?;
    let ws_addr = ws.addr();
    let ws_url = ws.url();

    let http = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    http.set_nonblocking(true).map_err(|e| e.to_string())?;
    let http_addr = http.local_addr().map_err(|e| e.to_string())?;
    let page_url = format!(
        "http://{http_addr}/?ws={}",
        urlencoding_simple(&ws_url)
    );

    println!("QFSNC1 WebSocket hub: {ws_url}");
    println!("Browser client:       {page_url}");
    println!("Open the URL above, click “Hello · Push · Pull”.");
    println!("Serving for ~120s (or until process exit)…");

    let stop = Arc::new(AtomicBool::new(false));
    let stop_t = stop.clone();
    let join = thread::spawn(move || http_loop(http, stop_t, ws_addr.port()));

    // Keep WS server alive while HTTP serves.
    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    while std::time::Instant::now() < deadline {
        thread::sleep(Duration::from_millis(200));
    }
    stop.store(true, Ordering::SeqCst);
    let _ = TcpStream::connect_timeout(&http_addr, Duration::from_millis(200));
    let _ = join.join();
    ws.stop();
    println!("browser-sync-demo complete");
    Ok(())
}

fn urlencoding_simple(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' | ':' | '/' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

fn http_loop(listener: TcpListener, stop: Arc<AtomicBool>, ws_port: u16) {
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                let _ = handle_http(&mut stream, ws_port);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break,
        }
    }
}

fn handle_http(stream: &mut TcpStream, _ws_port: u16) -> Result<(), String> {
    let mut buf = [0u8; 2048];
    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let first = req.lines().next().unwrap_or("");
    let path = first.split_whitespace().nth(1).unwrap_or("/");

    let (status, ctype, body): (&str, &str, Vec<u8>) = if path.starts_with("/qfsnc1.js") {
        ("200 OK", "application/javascript; charset=utf-8", QFSNC1_JS.as_bytes().to_vec())
    } else if path == "/" || path.starts_with("/?") || path.starts_with("/index") {
        ("200 OK", "text/html; charset=utf-8", INDEX_HTML.as_bytes().to_vec())
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

    #[test]
    fn embedded_assets_present() {
        assert!(INDEX_HTML.contains("QFSNC1"));
        assert!(INDEX_HTML.contains("Quillforge"));
        assert!(!INDEX_HTML.to_ascii_lowercase().contains("microsoft"));
        assert!(QFSNC1_JS.contains("QFSNC1"));
        assert!(QFSNC1_JS.contains("selfTest"));
        // Golden hello CBOR shared with Rust wire test.
        assert!(QFSNC1_JS.contains("a564747970656568656c6c6f"));
    }

    #[test]
    fn urlencoding_keeps_ws_url() {
        let u = urlencoding_simple("ws://127.0.0.1:9/qfsnc1");
        assert!(u.contains("ws://"));
        assert!(u.contains("/qfsnc1"));
    }
}
