//! Mock Miracast Sink Server
//! Run this to simulate a Miracast display for testing
//! Usage: cargo run --example mock_sink_server

use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Debug, Clone, PartialEq)]
enum MockSinkState {
    Init,
    OptionsReceived,
    CapabilitiesExchanged,
    Streaming,
    Teardown,
}

struct MockMiracastSink {
    state: MockSinkState,
    client_addr: Option<SocketAddr>,
    negotiated_params: HashMap<String, String>,
}

impl MockMiracastSink {
    fn new() -> Self {
        Self {
            state: MockSinkState::Init,
            client_addr: None,
            negotiated_params: HashMap::new(),
        }
    }

    async fn handle_client(
        &mut self,
        mut stream: TcpStream,
        addr: SocketAddr,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.client_addr = Some(addr);
        println!("✓ Client connected from {}", addr);

        let mut buffer = vec![0u8; 4096];

        loop {
            let n = stream.read(&mut buffer).await?;
            if n == 0 {
                println!("✓ Client disconnected");
                break;
            }

            let request = String::from_utf8_lossy(&buffer[..n]);
            println!("\n📥 Received request:\n{}", request);

            let response = self.process_request(&request);
            println!("\n📤 Sending response:\n{}", response);

            stream.write_all(response.as_bytes()).await?;
        }

        Ok(())
    }

    fn process_request(&mut self, request: &str) -> String {
        let lines: Vec<&str> = request.lines().collect();
        if lines.is_empty() {
            return self.error_response(400, "Bad Request", "1");
        }

        let request_line: Vec<&str> = lines[0].split_whitespace().collect();
        if request_line.len() < 2 {
            return self.error_response(400, "Bad Request", "1");
        }

        let method = request_line[0];

        let cseq = lines
            .iter()
            .find(|l| l.starts_with("CSeq:"))
            .and_then(|l| l.split(':').nth(1))
            .map(|s| s.trim())
            .unwrap_or("1");

        match method {
            "OPTIONS" => {
                self.state = MockSinkState::OptionsReceived;
                format!(
                    "RTSP/1.0 200 OK\r\n\
                     CSeq: {}\r\n\
                     Public: org.wfa.wfd1.0, SET_PARAMETER, GET_PARAMETER\r\n\
                     \r\n",
                    cseq
                )
            }
            "SET_PARAMETER" => {
                self.state = MockSinkState::CapabilitiesExchanged;
                let body_start = request.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
                let body = &request[body_start..];

                for line in body.lines() {
                    if let Some((key, value)) = line.split_once(':') {
                        self.negotiated_params
                            .insert(key.trim().to_string(), value.trim().to_string());
                    }
                }

                println!("📋 Negotiated parameters: {:?}", self.negotiated_params);

                format!(
                    "RTSP/1.0 200 OK\r\n\
                     CSeq: {}\r\n\
                     \r\n",
                    cseq
                )
            }
            "GET_PARAMETER" => {
                let body_start = request.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
                let param = request[body_start..].trim();

                let value = match param {
                    "wfd_video_formats" => "01 01 00 0000000000000007", // H.264
                    "wfd_audio_codecs" => "1 00 02 10",                 // AAC
                    "wfd_client_rtp_ports" => "RTP/AVP/UDP;unicast 19000 0 mode=play",
                    _ => "unknown",
                };

                let body = format!("{}: {}", param, value);
                format!(
                    "RTSP/1.0 200 OK\r\n\
                     CSeq: {}\r\n\
                     Content-Type: text/parameters\r\n\
                     Content-Length: {}\r\n\
                     \r\n\
                     {}",
                    cseq,
                    body.len(),
                    body
                )
            }
            "PLAY" => {
                self.state = MockSinkState::Streaming;
                let session = lines
                    .iter()
                    .find(|l| l.starts_with("Session:"))
                    .and_then(|l| l.split(':').nth(1))
                    .map(|s| s.trim())
                    .unwrap_or("default_session");

                println!("🎬 Streaming started! Session: {}", session);

                format!(
                    "RTSP/1.0 200 OK\r\n\
                     CSeq: {}\r\n\
                     Session: {}\r\n\
                     RTP-Info: url=rtsp://localhost:7236/stream\r\n\
                     \r\n",
                    cseq, session
                )
            }
            "TEARDOWN" => {
                self.state = MockSinkState::Teardown;
                println!("🛑 Session teardown");

                format!(
                    "RTSP/1.0 200 OK\r\n\
                     CSeq: {}\r\n\
                     \r\n",
                    cseq
                )
            }
            _ => self.error_response(405, "Method Not Allowed", cseq),
        }
    }

    fn error_response(&self, code: u16, reason: &str, cseq: &str) -> String {
        format!(
            "RTSP/1.0 {} {}\r\n\
             CSeq: {}\r\n\
             \r\n",
            code, reason, cseq
        )
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔═══════════════════════════════════════════╗");
    println!("║   Mock Miracast Sink Server                ║");
    println!("║   For testing swaybeam implementation     ║");
    println!("╚═══════════════════════════════════════════╝\n");

    let addr = "127.0.0.1:7236";
    let listener = TcpListener::bind(addr).await?;

    println!("✓ Server listening on {}", addr);
    println!("  Waiting for connections...\n");
    println!("Usage:");
    println!("  1. Run this server: cargo run --example mock_sink_server");
    println!("  2. Test with swaybeam: cargo run --package swaybeam-rtsp --example basic_server");
    println!("  3. Or use: telnet localhost 7236\n");

    loop {
        let (stream, addr) = listener.accept().await?;
        let mut sink = MockMiracastSink::new();

        tokio::spawn(async move {
            if let Err(e) = sink.handle_client(stream, addr).await {
                eprintln!("❌ Error: {}", e);
            }
        });
    }
}
