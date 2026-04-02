//! Mock Miracast Sink Server
//! Simulates a real Miracast display for testing purposes
//! Implements WFD 2.0 protocol specification

use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Debug, Clone, PartialEq)]
pub enum MockSinkState {
    Init,
    OptionsReceived,
    CapabilitiesExchanged,
    Ready,
    Streaming,
    Teardown,
}

pub struct MockMiracastSink {
    pub state: MockSinkState,
    pub server_name: String,
    pub supported_codecs: Vec<String>,
    pub current_session: Option<String>,
    pub negotiated_params: HashMap<String, String>,
}

impl MockMiracastSink {
    pub fn new(name: &str) -> Self {
        Self {
            state: MockSinkState::Init,
            server_name: name.to_string(),
            supported_codecs: vec!["H.264".to_string(), "H.265".to_string()],
            current_session: None,
            negotiated_params: HashMap::new(),
        }
    }

    pub fn handle_options(&mut self, cseq: &str) -> String {
        self.state = MockSinkState::OptionsReceived;

        format!(
            "RTSP/1.0 200 OK\r\n\
             CSeq: {}\r\n\
             Public: org.wfa.wfd1.0, SET_PARAMETER, GET_PARAMETER\r\n\
             \r\n",
            cseq
        )
    }

    pub fn handle_set_parameter(&mut self, cseq: &str, body: &str) -> String {
        self.state = MockSinkState::CapabilitiesExchanged;

        for line in body.lines() {
            if let Some((key, value)) = line.split_once(':') {
                self.negotiated_params
                    .insert(key.trim().to_string(), value.trim().to_string());
            }
        }

        format!(
            "RTSP/1.0 200 OK\r\n\
             CSeq: {}\r\n\
             \r\n",
            cseq
        )
    }

    pub fn handle_get_parameter(&mut self, cseq: &str, param: &str) -> String {
        let value = match param {
            "wfd_video_formats" => "01 01 00 0000000000000007", // H.264 support
            "wfd_audio_codecs" => "1 00 02 10",                 // AAC support
            "wfd_client_rtp_ports" => "RTP/AVP/UDP;unicast 19000 0 mode=play",
            _ => "unknown",
        };

        format!(
            "RTSP/1.0 200 OK\r\n\
             CSeq: {}\r\n\
             Content-Type: text/parameters\r\n\
             Content-Length: {}\r\n\
             \r\n\
             {}: {}",
            cseq,
            format!("{}: {}", param, value).len(),
            param,
            value
        )
    }

    pub fn handle_play(&mut self, cseq: &str, session: &str) -> String {
        self.state = MockSinkState::Streaming;
        self.current_session = Some(session.to_string());

        format!(
            "RTSP/1.0 200 OK\r\n\
             CSeq: {}\r\n\
             Session: {}\r\n\
             RTP-Info: url=rtsp://localhost/stream\r\n\
             \r\n",
            cseq, session
        )
    }

    pub fn handle_teardown(&mut self, cseq: &str) -> String {
        self.state = MockSinkState::Teardown;
        self.current_session = None;

        format!(
            "RTSP/1.0 200 OK\r\n\
             CSeq: {}\r\n\
             \r\n",
            cseq
        )
    }

    pub async fn handle_client(
        &mut self,
        mut stream: TcpStream,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut buffer = vec![0u8; 4096];

        loop {
            let n = stream.read(&mut buffer).await?;
            if n == 0 {
                break;
            }

            let request = String::from_utf8_lossy(&buffer[..n]);
            println!("[MockSink] Received:\n{}", request);

            let response = self.process_request(&request);
            stream.write_all(response.as_bytes()).await?;
            println!("[MockSink] Sent response");
        }

        Ok(())
    }

    fn process_request(&mut self, request: &str) -> String {
        let lines: Vec<&str> = request.lines().collect();
        if lines.is_empty() {
            return "RTSP/1.0 400 Bad Request\r\n\r\n".to_string();
        }

        let request_line: Vec<&str> = lines[0].split_whitespace().collect();
        if request_line.len() < 2 {
            return "RTSP/1.0 400 Bad Request\r\n\r\n".to_string();
        }

        let method = request_line[0];

        // Extract CSeq
        let mut cseq = "1";
        for line in &lines {
            if line.starts_with("CSeq:") {
                cseq = line.split(':').nth(1).unwrap_or("1").trim();
            }
        }

        match method {
            "OPTIONS" => self.handle_options(cseq),
            "SET_PARAMETER" => {
                let body_start = request.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
                let body = &request[body_start..];
                self.handle_set_parameter(cseq, body)
            }
            "GET_PARAMETER" => {
                let body_start = request.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
                let param = request[body_start..].trim();
                self.handle_get_parameter(cseq, param)
            }
            "PLAY" => {
                let session = lines
                    .iter()
                    .find(|l| l.starts_with("Session:"))
                    .and_then(|l| l.split(':').nth(1))
                    .map(|s| s.trim())
                    .unwrap_or("test_session");
                self.handle_play(cseq, session)
            }
            "TEARDOWN" => self.handle_teardown(cseq),
            _ => format!("RTSP/1.0 405 Method Not Allowed\r\nCSeq: {}\r\n\r\n", cseq),
        }
    }
}

pub async fn run_mock_sink_server(addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(addr).await?;
    println!("[MockSink] Server listening on {}", addr);

    loop {
        let (stream, client_addr) = listener.accept().await?;
        println!("[MockSink] Client connected from {}", client_addr);

        let mut sink = MockMiracastSink::new("Mock Miracast Display");
        tokio::spawn(async move {
            if let Err(e) = sink.handle_client(stream).await {
                eprintln!("[MockSink] Error handling client: {}", e);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_sink_creation() {
        let sink = MockMiracastSink::new("Test Display");
        assert_eq!(sink.state, MockSinkState::Init);
        assert_eq!(sink.server_name, "Test Display");
    }

    #[test]
    fn test_options_handling() {
        let mut sink = MockMiracastSink::new("Test");
        let response = sink.handle_options("1");
        assert!(response.contains("200 OK"));
        assert!(response.contains("org.wfa.wfd1.0"));
        assert_eq!(sink.state, MockSinkState::OptionsReceived);
    }

    #[test]
    fn test_set_parameter_handling() {
        let mut sink = MockMiracastSink::new("Test");
        let body = "wfd_video_formats: 1 0 00 04";
        let response = sink.handle_set_parameter("2", body);
        assert!(response.contains("200 OK"));
        assert_eq!(sink.state, MockSinkState::CapabilitiesExchanged);
    }

    #[test]
    fn test_play_handling() {
        let mut sink = MockMiracastSink::new("Test");
        let response = sink.handle_play("3", "test_session");
        assert!(response.contains("200 OK"));
        assert!(response.contains("Session: test_session"));
        assert_eq!(sink.state, MockSinkState::Streaming);
    }

    #[test]
    fn test_teardown_handling() {
        let mut sink = MockMiracastSink::new("Test");
        let response = sink.handle_teardown("4");
        assert!(response.contains("200 OK"));
        assert_eq!(sink.state, MockSinkState::Teardown);
    }
}
