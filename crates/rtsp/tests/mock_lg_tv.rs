//! Mock LG TV (Miracast Sink) for e2e testing
//! Simulates the actual behavior we observed from the LG C3 OLED TV

#![allow(dead_code)]

use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// LG TV WFD IEs as observed from actual device
/// wfd_dev_info=0x01131c440032
/// - Device Type: 01 (Primary Sink)
/// - Session Available: 1 (bit 2)
/// - RTSP Port: 0x131c = 4892 (non-standard!)
/// - Max Throughput: 0x4400
pub const LG_TV_WFD_IES: &[u8] = &[0x01, 0x13, 0x1c, 0x44, 0x00, 0x32];

/// LG TV MAC address
pub const LG_TV_MAC: &str = "22:28:BC:A8:6C:FE";

/// LG TV RTSP port (non-standard, observed from actual device)
pub const LG_TV_RTSP_PORT: u16 = 7236;

/// Standard Miracast RTSP port
pub const STANDARD_RTSP_PORT: u16 = 7236;

/// Expected WFD IEs from source (swaybeam)
/// Format: Subelement ID | Length | Device Info | RTSP Port | Throughput
pub const EXPECTED_SOURCE_WFD_IES: &[u8] = &[
    0x00, // Subelement ID: WFD Device Information
    0x00, 0x06, // Length: 6 bytes (big-endian)
    0x00, 0x90, // Device Info: match GNOME Network Displays source advertisement
    0x1C, 0x44, // RTSP Port: 7236 (big-endian)
    0x00, 0xC8, // Max Throughput: 200 Mbps
];

#[derive(Debug, Clone, PartialEq)]
pub enum MockTvState {
    Init,
    OptionsSent,
    GetParamSent,
    SetParamSent,
    SetupSent,
    PlaySent,
    Streaming,
    Teardown,
}

/// Mock LG TV that connects as RTSP client to source's RTSP server
pub struct MockLgTvClient {
    state: MockTvState,
    server_addr: String,
    stream: Option<TcpStream>,
    cseq: u32,
    sink_capabilities: HashMap<String, String>,
    session_id: Option<String>,
    rtp_port: u16,
}

impl MockLgTvClient {
    pub fn new(server_ip: &str, server_port: u16) -> Self {
        Self {
            state: MockTvState::Init,
            server_addr: format!("{}:{}", server_ip, server_port),
            stream: None,
            cseq: 0,
            sink_capabilities: HashMap::new(),
            session_id: None,
            rtp_port: 5004,
        }
    }

    pub async fn connect(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.stream = Some(TcpStream::connect(&self.server_addr).await?);
        self.state = MockTvState::Init;
        Ok(())
    }

    pub async fn run_negotiation(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_options().await?;
        self.get_source_capabilities().await?;
        self.send_sink_capabilities().await?;
        self.send_setup().await?;
        self.send_play().await?;
        self.state = MockTvState::Streaming;
        Ok(())
    }

    async fn send(&mut self, request: &str) -> Result<String, Box<dyn std::error::Error>> {
        let stream = self.stream.as_mut().ok_or("No connection")?;
        stream.write_all(request.as_bytes()).await?;

        let mut response = vec![0u8; 4096];
        let n = stream.read(&mut response).await?;
        Ok(String::from_utf8_lossy(&response[..n]).to_string())
    }

    pub async fn send_options(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        self.cseq += 1;
        let request = format!(
            "OPTIONS * RTSP/1.0\r\nCSeq: {}\r\nRequire: org.wfa.wfd1.0\r\n\r\n",
            self.cseq
        );
        let response = self.send(&request).await?;
        self.state = MockTvState::OptionsSent;
        Ok(response)
    }

    pub async fn get_source_capabilities(
        &mut self,
    ) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
        self.cseq += 1;
        let params = "wfd_video_formats\r\nwfd_audio_codecs\r\nwfd_client_rtp_ports\r\n";
        let request = format!(
            "GET_PARAMETER rtsp://localhost/stream RTSP/1.0\r\nCSeq: {}\r\nContent-Length: {}\r\n\r\n{}",
            self.cseq, params.len(), params
        );
        let response = self.send(&request).await?;

        let mut caps = HashMap::new();
        for line in response.lines() {
            if line.contains("wfd_") && line.contains(':') {
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                if parts.len() == 2 {
                    caps.insert(parts[0].trim().to_string(), parts[1].trim().to_string());
                }
            }
        }
        self.state = MockTvState::GetParamSent;
        Ok(caps)
    }

    pub async fn send_sink_capabilities(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        self.cseq += 1;

        let mut body = String::new();
        body.push_str("wfd_video_formats: 01 01 00 0000000000000007\r\n");
        body.push_str("wfd_audio_codecs: AAC 00000001 00\r\n");
        body.push_str("wfd_client_rtp_ports: RTP/AVP/UDP;unicast 5004 5005 mode=play\r\n");

        let request = format!(
            "SET_PARAMETER rtsp://localhost/stream RTSP/1.0\r\nCSeq: {}\r\nContent-Type: text/parameters\r\nContent-Length: {}\r\n\r\n{}",
            self.cseq, body.len(), body
        );
        let response = self.send(&request).await?;
        self.state = MockTvState::SetParamSent;
        Ok(response)
    }

    pub async fn send_setup(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        self.cseq += 1;
        let request = format!(
            "SETUP rtsp://localhost/stream RTSP/1.0\r\nCSeq: {}\r\nTransport: RTP/AVP/UDP;unicast;client_port={}-{}\r\n\r\n",
            self.cseq, self.rtp_port, self.rtp_port + 1
        );
        let response = self.send(&request).await?;

        if response.contains("Session:") {
            for line in response.lines() {
                if line.starts_with("Session:") {
                    self.session_id = Some(line.split(':').nth(1).unwrap_or("").trim().to_string());
                }
            }
        }
        self.state = MockTvState::SetupSent;
        Ok(response)
    }

    pub async fn send_play(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        self.cseq += 1;
        let session = self
            .session_id
            .clone()
            .unwrap_or_else(|| "default".to_string());
        let request = format!(
            "PLAY rtsp://localhost/stream RTSP/1.0\r\nCSeq: {}\r\nSession: {}\r\nRange: npt=0.000-\r\n\r\n",
            self.cseq, session
        );
        let response = self.send(&request).await?;
        self.state = MockTvState::PlaySent;
        Ok(response)
    }

    pub fn get_state(&self) -> MockTvState {
        self.state.clone()
    }
}

pub async fn run_reverse_lg_tv_server(
    server_ip: &str,
    server_port: u16,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let mut stream = TcpStream::connect(format!("{}:{}", server_ip, server_port)).await?;
    let mut methods = Vec::new();

    loop {
        let request = read_rtsp_message(&mut stream).await?;
        let method = request
            .split_whitespace()
            .next()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "Missing method"))?
            .to_string();
        methods.push(method.clone());

        let response = match method.as_str() {
            "OPTIONS" => "RTSP/1.0 200 OK\r\nCSeq: 1\r\nPublic: org.wfa.wfd1.0, OPTIONS, SETUP, PLAY, PAUSE, TEARDOWN, SET_PARAMETER, GET_PARAMETER\r\n\r\n".to_string(),
            "GET_PARAMETER" => {
                let peer_options = "OPTIONS * RTSP/1.0\r\nCSeq: 0\r\nUser-Agent: LGE\r\nRequire: org.wfa.wfd1.0\r\n\r\n";
                stream.write_all(peer_options.as_bytes()).await?;

                let peer_options_response = read_rtsp_message(&mut stream).await?;
                if !peer_options_response.starts_with("RTSP/1.0 200 OK") {
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "Expected 200 OK to peer OPTIONS, got: {}",
                            peer_options_response
                        ),
                    )));
                }

                let body = concat!(
                    "wfd_video_formats: 01 01 00 0000000000000007\r\n",
                    "wfd_audio_codecs: AAC 00000001 00\r\n",
                    "wfd_client_rtp_ports: RTP/AVP/UDP;unicast 5006 5007 mode=play\r\n",
                );
                format!(
                    "RTSP/1.0 200 OK\r\nCSeq: 2\r\nContent-Type: text/parameters\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                )
            }
            "SET_PARAMETER" => "RTSP/1.0 200 OK\r\nCSeq: 3\r\n\r\n".to_string(),
            "SETUP" => {
                let transport = request
                    .lines()
                    .find(|line| line.starts_with("Transport:"))
                    .unwrap_or("Transport: RTP/AVP/UDP;unicast;client_port=5004-5005");
                format!(
                    "RTSP/1.0 200 OK\r\nCSeq: 4\r\nSession: 12345678;timeout=30\r\n{};server_port=5006-5007\r\n\r\n",
                    transport
                )
            }
            "PLAY" => "RTSP/1.0 200 OK\r\nCSeq: 5\r\nSession: 12345678\r\nRTP-Info: url=rtsp://192.168.49.1/wfd1.0/streamid=0/trackID=1;seq=123456;rtptime=123456789\r\n\r\n".to_string(),
            other => {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Unexpected RTSP method {other}"),
                )));
            }
        };

        stream.write_all(response.as_bytes()).await?;

        if method == "PLAY" {
            break;
        }
    }

    Ok(methods)
}

pub async fn run_reverse_lg_tv_server_implicit_play(
    server_ip: &str,
    server_port: u16,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let mut stream = TcpStream::connect(format!("{}:{}", server_ip, server_port)).await?;
    let mut methods = Vec::new();

    loop {
        let request = read_rtsp_message(&mut stream).await?;
        let method = request
            .split_whitespace()
            .next()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "Missing method"))?
            .to_string();
        methods.push(method.clone());

        let response = match method.as_str() {
            "OPTIONS" => {
                "RTSP/1.0 200 OK\r\nCSeq: 1\r\nPublic: org.wfa.wfd1.0, GET_PARAMETER, SET_PARAMETER, PLAY\r\n\r\n".to_string()
            }
            "GET_PARAMETER" => {
                let body = concat!(
                    "wfd_video_formats: 01 01 00 0000000000000007\r\n",
                    "wfd_audio_codecs: AAC 00000001 00\r\n",
                    "wfd_client_rtp_ports: RTP/AVP/UDP;unicast 5006 0 mode=play\r\n",
                );
                format!(
                    "RTSP/1.0 200 OK\r\nCSeq: 2\r\nContent-Type: text/parameters\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                )
            }
            "SET_PARAMETER" => "RTSP/1.0 200 OK\r\nCSeq: 3\r\n\r\n".to_string(),
            "PLAY" => "RTSP/1.0 200 OK\r\nCSeq: 4\r\nRTP-Info: url=rtsp://192.168.49.1/wfd1.0/streamid=0/trackID=1;seq=123456;rtptime=123456789\r\n\r\n".to_string(),
            other => {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Unexpected RTSP method {other}"),
                )));
            }
        };

        stream.write_all(response.as_bytes()).await?;

        if method == "PLAY" {
            break;
        }
    }

    Ok(methods)
}

async fn read_rtsp_message(
    stream: &mut TcpStream,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut header_bytes = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let bytes_read = stream.read(&mut byte).await?;
        if bytes_read == 0 {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Connection closed before RTSP request completed",
            )));
        }

        header_bytes.push(byte[0]);
        if header_bytes.ends_with(b"\r\n\r\n") {
            break;
        }
    }

    let headers = String::from_utf8(header_bytes)?;
    let mut request = headers.clone();

    let content_length = headers
        .lines()
        .find_map(|header| {
            header
                .strip_prefix("Content-Length:")
                .and_then(|value| value.trim().parse::<usize>().ok())
        })
        .unwrap_or(0);

    if content_length > 0 {
        let mut body = vec![0u8; content_length];
        stream.read_exact(&mut body).await?;
        request.push_str(&String::from_utf8_lossy(&body));
    }

    Ok(request)
}

/// Validate WFD Device Information IE format
pub fn validate_wfd_device_info(wfd_ies: &[u8]) -> Result<WfdDeviceInfo, WfdError> {
    if wfd_ies.is_empty() {
        return Err(WfdError::EmptyIes);
    }

    if wfd_ies[0] != 0x00 {
        return Err(WfdError::InvalidSubelementId(wfd_ies[0]));
    }

    if wfd_ies.len() < 3 {
        return Err(WfdError::TooShort);
    }

    let length = ((wfd_ies[1] as u16) << 8) | (wfd_ies[2] as u16);
    if length != 6 {
        return Err(WfdError::InvalidLength(length));
    }

    if wfd_ies.len() < 9 {
        return Err(WfdError::TooShortForLength);
    }

    let device_info = ((wfd_ies[3] as u16) << 8) | (wfd_ies[4] as u16);
    let device_type = ((device_info >> 8) & 0x03) as u8;
    let session_available = (device_info & 0x0001) != 0;
    let wfd_enabled = (device_info & 0x0001) != 0;

    let rtsp_port = ((wfd_ies[5] as u16) << 8) | (wfd_ies[6] as u16);
    let max_throughput = ((wfd_ies[7] as u16) << 8) | (wfd_ies[8] as u16);
    let coupled_sink = 0;

    Ok(WfdDeviceInfo {
        device_type,
        session_available,
        wfd_enabled,
        rtsp_port,
        max_throughput,
        coupled_sink,
    })
}

#[derive(Debug, Clone)]
pub struct WfdDeviceInfo {
    pub device_type: u8,
    pub session_available: bool,
    pub wfd_enabled: bool,
    pub rtsp_port: u16,
    pub max_throughput: u16,
    pub coupled_sink: u8,
}

#[derive(Debug)]
pub enum WfdError {
    EmptyIes,
    InvalidSubelementId(u8),
    TooShort,
    InvalidLength(u16),
    TooShortForLength,
}

impl std::fmt::Display for WfdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WfdError::EmptyIes => write!(f, "WFD IEs are empty"),
            WfdError::InvalidSubelementId(id) => write!(f, "Invalid subelement ID: 0x{:02x}", id),
            WfdError::TooShort => write!(f, "WFD IEs too short"),
            WfdError::InvalidLength(len) => write!(f, "Invalid length: {}", len),
            WfdError::TooShortForLength => write!(f, "WFD IEs too short for declared length"),
        }
    }
}

impl std::error::Error for WfdError {}
