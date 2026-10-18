//! Deterministic receive regressions: no scheduler sleeps or OS error strings.
use super::*;
use qf_core::{ActorId, DocumentId, SeqIdGen, SessionId};
use std::collections::VecDeque;
use std::io::{self, Cursor, ErrorKind};

enum Step {
    Bytes(Cursor<Vec<u8>>),
    Error(ErrorKind),
}

#[derive(Default)]
struct ScriptedStream {
    input: VecDeque<Step>,
    output: Vec<u8>,
}

impl ScriptedStream {
    fn push_bytes(&mut self, bytes: &[u8]) {
        self.input
            .push_back(Step::Bytes(Cursor::new(bytes.to_vec())));
    }

    fn error(&mut self, kind: ErrorKind) {
        self.input.push_back(Step::Error(kind));
    }
}

impl Read for ScriptedStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            match self.input.front_mut() {
                Some(Step::Bytes(bytes)) => {
                    let n = bytes.read(buf)?;
                    if n != 0 {
                        return Ok(n);
                    }
                    self.input.pop_front();
                }
                Some(Step::Error(kind)) => {
                    let kind = *kind;
                    self.input.pop_front();
                    // WouldBlock deliberately has no English retry keywords;
                    // TimedOut also exercises the old recognized retry branch.
                    let message = if kind == ErrorKind::TimedOut {
                        "timed out"
                    } else {
                        "scripted I/O status"
                    };
                    return Err(io::Error::new(kind, message));
                }
                None => return Ok(0),
            }
        }
    }
}

impl Write for ScriptedStream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.output.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn hello_frame() -> Vec<u8> {
    let mut ids = SeqIdGen::default();
    let req = WireRequest::Hello(Hello {
        client_id: ActorId::new(&mut ids),
        session_id: SessionId::new(&mut ids),
        document_id: DocumentId::new(&mut ids),
        have_seq: 0,
    });
    let mut frame = Vec::new();
    write_qfsnc1_ws(&mut frame, &req, true).unwrap();
    frame
}

fn assert_two_hello_acks(stream: &ScriptedStream) {
    let mut output = Cursor::new(&stream.output);
    let mut reader = WsReader::default();
    for _ in 0..2 {
        let response: WireResponse = reader.read(&mut output).unwrap();
        assert!(
            matches!(response, WireResponse::HelloAck(_)),
            "{response:?}"
        );
    }
    assert_eq!(output.position() as usize, stream.output.len());
}

#[test]
fn server_idle_kinds_keep_healthy_connection_open() {
    for kind in [ErrorKind::WouldBlock, ErrorKind::TimedOut] {
        let frame = hello_frame();
        let mut stream = ScriptedStream::default();
        stream.error(kind);
        stream.push_bytes(&frame);
        stream.error(kind);
        stream.push_bytes(&frame);
        serve_session_ws(&mut stream, &Arc::new(Mutex::new(SyncHub::new())), false).unwrap();
        assert_two_hello_acks(&stream);
    }
}

#[test]
fn server_partial_frame_survives_idle_at_every_byte_boundary() {
    let frame = hello_frame();
    for kind in [ErrorKind::TimedOut, ErrorKind::WouldBlock] {
        for split in 1..frame.len() {
            let mut stream = ScriptedStream::default();
            stream.push_bytes(&frame[..split]);
            stream.error(kind);
            stream.push_bytes(&frame[split..]);
            stream.push_bytes(&frame);
            serve_session_ws(&mut stream, &Arc::new(Mutex::new(SyncHub::new())), false).unwrap();
            assert_two_hello_acks(&stream);
        }
    }
}

#[test]
fn extended_lengths_masking_and_coalesced_frames_survive_idle() {
    for size in [0, 120, 1024, 65_536] {
        let message = "x".repeat(size);
        let response = WireResponse::Error {
            kind: "test".into(),
            message: message.clone(),
        };
        for mask in [false, true] {
            let mut frame = Vec::new();
            write_qfsnc1_ws(&mut frame, &response, mask).unwrap();
            for kind in [ErrorKind::WouldBlock, ErrorKind::TimedOut] {
                // Include every possible header byte and early/late payload bytes.
                for split in (1..=15).chain([frame.len() / 2, frame.len() - 1]) {
                    let mut stream = ScriptedStream::default();
                    stream.push_bytes(&frame[..split]);
                    stream.error(kind);
                    stream.push_bytes(&frame[split..]);
                    stream.push_bytes(&frame);
                    let mut reader = WsReader::default();
                    let error = reader.read::<_, WireResponse>(&mut stream).unwrap_err();
                    assert!(error.is_idle(), "{error:?}");
                    for _ in 0..2 {
                        match reader.read::<_, WireResponse>(&mut stream).unwrap() {
                            WireResponse::Error {
                                kind,
                                message: actual,
                            } => {
                                assert_eq!(kind, "test");
                                assert_eq!(actual, message);
                            }
                            other => panic!("unexpected response: {other:?}"),
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn interrupted_reads_retry_without_losing_progress() {
    let frame = hello_frame();
    let mut stream = ScriptedStream::default();
    for byte in frame {
        stream.error(ErrorKind::Interrupted);
        stream.push_bytes(&[byte]);
    }
    let response: WireRequest = WsReader::default().read(&mut stream).unwrap();
    assert!(matches!(response, WireRequest::Hello(_)));
}

#[test]
fn eof_close_fatal_errors_and_malformed_lengths_are_not_idle() {
    // Error text must never turn an unrelated I/O/protocol failure into retry.
    let fatal = crate::ws_receive::WsReadError::Io(io::Error::new(
        ErrorKind::InvalidData,
        "timeout would block 10060",
    ));
    assert!(!fatal.is_idle());
    let protocol = crate::ws_receive::WsReadError::Protocol(SyncError::Network(
        "timeout would block 10060".into(),
    ));
    assert!(!protocol.is_idle());
    let frame = hello_frame();
    for length in [0, 1, frame.len() - 1] {
        let mut reader = WsReader::default();
        let error = reader
            .read::<_, WireRequest>(&mut Cursor::new(&frame[..length]))
            .unwrap_err();
        assert!(error.is_peer_gone());
        assert!(!error.is_idle());
    }
    for kind in [
        ErrorKind::ConnectionReset,
        ErrorKind::BrokenPipe,
        ErrorKind::InvalidData,
    ] {
        let mut stream = ScriptedStream::default();
        stream.error(kind);
        let error = WsReader::default()
            .read::<_, WireRequest>(&mut stream)
            .unwrap_err();
        assert!(!error.is_idle());
        assert!(matches!(error, crate::ws_receive::WsReadError::Io(e) if e.kind() == kind));
    }
    for header in [
        vec![0x82, 127, 0, 0, 0, 0, 1, 0, 0, 1], // 16 MiB + 1, no allocation
        vec![0x82, 127, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
        vec![0x81, 0], // text still rejected
        vec![0x83, 0], // unsupported opcode still rejected
        vec![0x82, 0], // binary but not QFSNC1
    ] {
        let error = WsReader::default()
            .read::<_, WireRequest>(&mut Cursor::new(header))
            .unwrap_err();
        assert!(!error.is_idle());
        assert!(matches!(error, crate::ws_receive::WsReadError::Protocol(_)));
    }
    let error = WsReader::default()
        .read::<_, WireRequest>(&mut Cursor::new([0x88, 0]))
        .unwrap_err();
    assert!(error.is_peer_gone());
    assert!(!error.is_idle());
}

#[test]
fn control_frames_do_not_consume_following_binary_frame() {
    let mut stream = ScriptedStream::default();
    stream.push_bytes(&[0x89, 1, 42]); // existing ping handling
    stream.push_bytes(&[0x8a, 0]);
    stream.error(ErrorKind::WouldBlock);
    stream.push_bytes(&hello_frame());
    let mut reader = WsReader::default();
    assert!(reader
        .read::<_, WireRequest>(&mut stream)
        .unwrap_err()
        .is_idle());
    assert!(matches!(
        reader.read::<_, WireRequest>(&mut stream).unwrap(),
        WireRequest::Hello(_)
    ));
}

#[test]
fn client_wait_notify_resumes_partial_frame_and_restores_socket_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (ready_tx, ready_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    let mut ids = SeqIdGen::default();
    let document_id = DocumentId::new(&mut ids);
    let event = HeadNotify {
        document_id,
        server_head_seq: 7,
        from_actor: ActorId::new(&mut ids),
    };
    let writer = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        server_handshake(&mut stream).unwrap();
        let mut frame = Vec::new();
        write_qfsnc1_ws(
            &mut frame,
            &WireResponse::Notify(NotifyBatch {
                events: vec![event],
            }),
            false,
        )
        .unwrap();
        stream.write_all(&frame[..1]).unwrap();
        stream.flush().unwrap();
        ready_tx.send(()).unwrap();
        resume_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        stream.write_all(&frame[1..]).unwrap();
        stream.flush().unwrap();
    });
    let mut client = WsSyncSession::open(addr).unwrap();
    let previous = client.stream.read_timeout().unwrap();
    ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(client
        .wait_notify(Duration::from_millis(30))
        .unwrap()
        .events
        .is_empty());
    assert_eq!(client.stream.read_timeout().unwrap(), previous);
    resume_tx.send(()).unwrap();
    let batch = client.wait_notify(Duration::from_secs(2)).unwrap();
    assert_eq!(batch.events.len(), 1);
    assert_eq!(batch.events[0].document_id, document_id);
    assert_eq!(batch.events[0].server_head_seq, 7);
    assert_eq!(client.stream.read_timeout().unwrap(), previous);
    // Failure paths restore the same timeout too.
    assert!(client.wait_notify(Duration::from_secs(1)).is_err());
    assert_eq!(client.stream.read_timeout().unwrap(), previous);
    writer.join().unwrap();
}

#[test]
fn ws_upgrade_can_span_more_than_one_live_poll() {
    let server = WsSyncServer::start(SyncHub::new()).unwrap();
    let mut stream = TcpStream::connect(server.addr()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream.write_all(b"GET /qfsnc1 HTTP/1.1\r\n").unwrap();
    thread::sleep(LIVE_POLL * 3);
    stream.write_all(b"Host: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n").unwrap();
    let headers = read_http_headers(&mut stream).unwrap();
    assert!(headers.starts_with("HTTP/1.1 101"), "{headers}");
    stream.write_all(&hello_frame()).unwrap();
    assert!(matches!(
        WsReader::default()
            .read::<_, WireResponse>(&mut stream)
            .unwrap(),
        WireResponse::HelloAck(_)
    ));
    drop(stream);
    server.stop();
}
