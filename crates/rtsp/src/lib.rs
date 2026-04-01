use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use parking_lot;

// WFD Capabilities representing Wi-Fi Display capabilities
#[derive(Debug, Clone)]
pub struct WfdCapabilities {
    pub client_rtp_ports: Option<String>,
    pub video_formats: Option<String>,
    pub audio_codecs: Option<String>,
    pub display_edid: Option<String>,
    pub coupled_sink: Option<String>,
    pub uibc_capability: Option<String>,
    pub standby_resume_capability: Option<String>,
    pub content_protection: Option<String>,
}

impl WfdCapabilities {
    pub fn new() -> Self {
        WfdCapabilities {
            client_rtp_ports: None,
            video_formats: None,
            audio_codecs: None,
            display_edid: None,
            coupled_sink: None,
            uibc_capability: None,
            standby_resume_capability: None,
            content_protection: None,
        }
    }
    
    pub fn set_parameter(&mut self, param_name: &str, value: &str) -> Result<(), RtspError> {
        match param_name {
            "wfd_client_rtp_ports" => self.client_rtp_ports = Some(value.to_string()),
            "wfd_video_formats" => self.video_formats = Some(value.to_string()),
            "wfd_audio_codecs" => self.audio_codecs = Some(value.to_string()),
            "wfd_display_edid" => self.display_edid = Some(value.to_string()),
            "wfd_coupled_sink" => self.coupled_sink = Some(value.to_string()),
            "wfd_uibc_capability" => self.uibc_capability = Some(value.to_string()),
            "wfd_standby_resume_capability" => self.standby_resume_capability = Some(value.to_string()),
            "wfd_content_protection" => self.content_protection = Some(value.to_string()),
            _ => return Err(RtspError::InvalidParameter(param_name.to_string())),
        }
        Ok(())
    }
    
    pub fn get_parameter(&self, param_name: &str) -> Result<Option<&str>, RtspError> {
        match param_name {
            "wfd_client_rtp_ports" => Ok(self.client_rtp_ports.as_deref()),
            "wfd_video_formats" => Ok(self.video_formats.as_deref()),
            "wfd_audio_codecs" => Ok(self.audio_codecs.as_deref()),
            "wfd_display_edid" => Ok(self.display_edid.as_deref()),
            "wfd_coupled_sink" => Ok(self.coupled_sink.as_deref()),
            "wfd_uibc_capability" => Ok(self.uibc_capability.as_deref()),
            "wfd_standby_resume_capability" => Ok(self.standby_resume_capability.as_deref()),
            "wfd_content_protection" => Ok(self.content_protection.as_deref()),
            _ => Err(RtspError::InvalidParameter(param_name.to_string())),
        }
    }
}

// State machine for RTSP session
#[derive(Debug, Clone, PartialEq)]
pub enum SessionState {
    Init,
    OptionsReceived,
    GetParamReceived,
    SetParamReceived,
    Play,
    Teardown,
}

// Active session state
#[derive(Debug, Clone)]
pub struct RtspSession {
    pub session_id: String,
    pub state: SessionState,
    pub capabilities: WfdCapabilities,
    pub parameters: HashMap<String, String>,
}

impl RtspSession {
    pub fn new(session_id: String) -> Self {
        RtspSession {
            session_id,
            state: SessionState::Init,
            capabilities: WfdCapabilities::new(),
            parameters: HashMap::new(),
        }
    }

    pub fn transition_to(&mut self, new_state: SessionState) {
        self.state = new_state;
    }

    pub fn process_options(&mut self) -> Result<String, RtspError> {
        self.transition_to(SessionState::OptionsReceived);
        Ok("Public: OPTIONS, GET_PARAMETER, SET_PARAMETER, PLAY, TEARDOWN\r\n".to_string())
    }

    pub fn process_get_parameter(&mut self, params: &[&str]) -> Result<String, RtspError> {
        let mut response = String::new();
        
        for param in params {
            let value = self.capabilities.get_parameter(param)?;
            if let Some(val) = value {
                response.push_str(&format!("{}: {}\r\n", param, val));
            }
        }
        
        self.transition_to(SessionState::GetParamReceived);
        Ok(response)
    }

    pub fn process_set_parameter(&mut self, params: &HashMap<String, String>) -> Result<String, RtspError> {
        for (param_name, value) in params {
            self.capabilities.set_parameter(param_name, value)?;
            self.parameters.insert(param_name.clone(), value.clone());
        }
        
        self.transition_to(SessionState::SetParamReceived);
        Ok("200 OK\r\n".to_string())
    }

    pub fn process_play(&mut self) -> Result<String, RtspError> {
        self.transition_to(SessionState::Play);
        // For now, just return a basic play response
        Ok("RTP-Info: url=rtsp://server.example.com/movie/, seq=123456\r\n".to_string())
    }

    pub fn process_teardown(&mut self) -> Result<String, RtspError> {
        self.transition_to(SessionState::Teardown);
        Ok("200 OK\r\n".to_string())
    }
}

// Error types for RTSP operations
#[derive(thiserror::Error, Debug)]
pub enum RtspError {
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Parse Error: {0}")]
    Parse(String),
    
    #[error("Invalid Parameter: {0}")]
    InvalidParameter(String),
    
    #[error("Invalid Request Method: {0}")]
    InvalidMethod(String),
    
    #[error("Invalid State Transition")]
    InvalidStateTransition,
    
    #[error("Session Not Found")]
    SessionNotFound,
    
    #[error("Request Timeout")]
    Timeout,
    
    #[error("Protocol Violation: {0}")]
    ProtocolViolation(String),
}

// RTSP Message types
#[derive(Debug)]
pub enum RtspMessage {
    Options { cseq: u32 },
    GetParameter { cseq: u32, params: Vec<String> },
    SetParameter { cseq: u32, params: HashMap<String, String> },
    Play { cseq: u32, session: Option<String> },
    Teardown { cseq: u32, session: Option<String> },
}

impl RtspMessage {
    pub fn parse(data: &str) -> Result<Self, RtspError> {
        let lines: Vec<&str> = data.lines().collect();
        
        if lines.is_empty() {
            return Err(RtspError::Parse("Empty message".to_string()));
        }

        let first_line = lines[0];
        let parts: Vec<&str> = first_line.split_whitespace().collect();

        if parts.len() < 2 {
            return Err(RtspError::Parse("Malformed request line".to_string()));
        }

        let method = parts[0];
        let cseq_line = lines.iter()
            .find(|line| line.starts_with("CSeq:"))
            .ok_or_else(|| RtspError::Parse("Missing CSeq".to_string()))?;
        
        let cseq: u32 = cseq_line[5..].trim().parse()
            .map_err(|_| RtspError::Parse("Invalid CSeq".to_string()))?;

        match method {
            "OPTIONS" => Ok(RtspMessage::Options { cseq }),
            "GET_PARAMETER" => {
                let mut params = Vec::new();
                
                // Look for WFD parameters in the message body
                for line in lines.iter() {
                    if line.contains("wfd_") && line.contains(':') {
                        let param_parts: Vec<&str> = line.splitn(2, ':').collect();
                        if param_parts.len() == 2 {
                            params.push(param_parts[0].trim().to_string());
                        }
                    }
                }
                
                Ok(RtspMessage::GetParameter { 
                    cseq, 
                    params,
                })
            },
            "SET_PARAMETER" => {
                let mut params = HashMap::new();
                
                // Parse WFD parameters in the message body
                for i in 1..lines.len() {
                    let line = lines[i];
                    if line.contains("wfd_") && line.contains(':') {
                        let param_parts: Vec<&str> = line.splitn(2, ':').collect();
                        if param_parts.len() == 2 {
                            params.insert(
                                param_parts[0].trim().to_string(), 
                                param_parts[1].trim().to_string()
                            );
                        }
                    }
                }
                
                Ok(RtspMessage::SetParameter { 
                    cseq, 
                    params,
                })
            },
            "PLAY" => {
                let session = parse_header(&lines, "Session");
                Ok(RtspMessage::Play { cseq, session })
            },
            "TEARDOWN" => {
                let session = parse_header(&lines, "Session");
                Ok(RtspMessage::Teardown { cseq, session })
            },
            _ => Err(RtspError::InvalidMethod(method.to_string())),
        }
    }
}

fn parse_header(lines: &[&str], header: &str) -> Option<String> {
    for line in lines {
        if line.to_lowercase().starts_with(&header.to_lowercase()) {
            return Some(line[(header.len() + 1)..].trim().to_string());
        }
    }
    None
}

// RTSP Server for handling WFD negotiations
pub struct RtspServer {
    address: String,
    sessions: Arc<parking_lot::RwLock<HashMap<String, RtspSession>>>,
}

impl RtspServer {
    pub fn new(address: String) -> Self {
        RtspServer {
            address,
            sessions: Arc::new(parking_lot::RwLock::new(HashMap::new())),
        }
    }

    pub async fn start(&self) -> Result<(), RtspError> {
        let listener = TcpListener::bind(&self.address).await?;
        tracing::info!("RTSP server listening on {}", self.address);

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    tracing::info!("Connection established from {}", addr);
                    
                    let sessions = self.sessions.clone();
                    
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(socket, sessions).await {
                            tracing::error!("Error handling connection: {:?}", e);
                        }
                    });
                }
                Err(e) => {
                    tracing::error!("Failed to accept connection: {:?}", e);
                }
            }
        }
    }

    pub fn get_session(&self, session_id: &str) -> Option<RtspSession> {
        self.sessions.read().get(session_id).cloned()
    }

    pub fn create_session(&self, session_id: String) -> RtspSession {
        let session = RtspSession::new(session_id.clone());
        self.sessions.write().insert(session_id, session.clone());
        session
    }

    pub fn remove_session(&self, session_id: &str) {
        self.sessions.write().remove(session_id);
    }
}

async fn handle_connection(
    mut socket: TcpStream,
    sessions: Arc<parking_lot::RwLock<HashMap<String, RtspSession>>>
) -> Result<(), RtspError> {
    let mut buffer = [0; 4096];
    
    loop {
        let n = socket.read(&mut buffer).await?;
        if n == 0 {
            break;
        }

        let request = String::from_utf8_lossy(&buffer[..n]);
        tracing::debug!("Received request: {}", request);

        let response = match RtspMessage::parse(&request) {
            Ok(msg) => {
                match msg {
                    RtspMessage::Options { cseq } => {
                        handle_options(cseq, &sessions).await
                    }
                    RtspMessage::GetParameter { cseq, params } => {
                        handle_get_parameter(cseq, params, &sessions).await
                    }
                    RtspMessage::SetParameter { cseq, params } => {
                        handle_set_parameter(cseq, params, &sessions).await
                    }
                    RtspMessage::Play { cseq, session } => {
                        handle_play(cseq, session, &sessions).await
                    }
                    RtspMessage::Teardown { cseq, session } => {
                        handle_teardown(cseq, session, &sessions).await
                    }
                }
            }
            Err(e) => {
                tracing::error!("Error parsing message: {:?}", e);
                format!("RTSP/1.0 400 Bad Request\r\nCSeq: {}\r\n\r\n", 0)
            }
        };

        socket.write_all(response.as_bytes()).await?;
    }

    Ok(())
}

async fn handle_options(
    cseq: u32,
    sessions: &Arc<parking_lot::RwLock<HashMap<String, RtspSession>>>
) -> String {
    let session_id = format!("sess_{}", rand::random::<u64>());
    let mut session = RtspSession::new(session_id.clone());
    
    let caps_response = match session.process_options() {
        Ok(response) => response,
        Err(_) => "Public: OPTIONS, GET_PARAMETER, SET_PARAMETER, PLAY, TEARDOWN\r\n".to_string(),
    };

    sessions.write().insert(session_id, session);

    format!(
        "RTSP/1.0 200 OK\r\nCSeq: {}\r\n{}\r\n",
        cseq, caps_response
    )
}

async fn handle_get_parameter(
    cseq: u32,
    param_names: Vec<String>,
    sessions: &Arc<parking_lot::RwLock<HashMap<String, RtspSession>>>
) -> String {
    let mut lock = sessions.write();
    
    // Get the most recently created session as fallback
    let maybe_session_id = {
        lock.keys()
            .last()
            .cloned()
    };
    
    // If no session exists, return error
    if let Some(session_id) = maybe_session_id {
        if let Some(mut session) = lock.get(&session_id).cloned() {
            let param_refs: Vec<&str> = param_names.iter().map(|s| s.as_str()).collect();
            let response = match session.process_get_parameter(&param_refs) {
                Ok(response) => response,
                Err(_) => "".to_string(),
            };
            
            // Update the session in storage
            lock.insert(session_id, session);
            
            format!(
                "RTSP/1.0 200 OK\r\nCSeq: {}\r\n{}\r\n",
                cseq, response
            )
        } else {
            format!("RTSP/1.0 454 Session Not Found\r\nCSeq: {}\r\n\r\n", cseq)
        }
    } else {
        format!("RTSP/1.0 454 Session Not Found\r\nCSeq: {}\r\n\r\n", cseq)
    }
}

async fn handle_set_parameter(
    cseq: u32,
    params: HashMap<String, String>,
    sessions: &Arc<parking_lot::RwLock<HashMap<String, RtspSession>>>
) -> String {
    let mut lock = sessions.write();
    
    // Get the most recently created session as fallback
    let maybe_session_id = {
        lock.keys()
            .last()
            .cloned()
    };
    
    // If no session exists, return error
    if let Some(session_id) = maybe_session_id {
        if let Some(mut session) = lock.get(&session_id).cloned() {
            let response = match session.process_set_parameter(&params) {
                Ok(response) => response,
                Err(_) => "200 OK\r\n".to_string(),
            };
            
            // Update the session in storage
            lock.insert(session_id, session);
            
            format!(
                "RTSP/1.0 200 OK\r\nCSeq: {}\r\n{}\r\n",
                cseq, response
            )
        } else {
            format!("RTSP/1.0 454 Session Not Found\r\nCSeq: {}\r\n\r\n", cseq)
        }
    } else {
        format!("RTSP/1.0 454 Session Not Found\r\nCSeq: {}\r\n\r\n", cseq)
    }
}

async fn handle_play(
    cseq: u32,
    session_id_opt: Option<String>,
    sessions: &Arc<parking_lot::RwLock<HashMap<String, RtspSession>>>
) -> String {
    let sess_id = session_id_opt.or_else(|| {
        let lock = sessions.read();
        lock.keys().last().cloned()
    });

    if let Some(session_id) = sess_id {
        let mut lock = sessions.write();
        if let Some(mut session) = lock.remove(&session_id) {
            let response = match session.process_play() {
                Ok(response) => response,
                Err(_) => "RTP-Info: url=rtsp://server/, seq=123456\r\n".to_string(),
            };
            
            // Put the updated session back in storage
            lock.insert(session_id, session);
            
            format!("RTSP/1.0 200 OK\r\nCSeq: {}\r\n{}\r\n", cseq, response)
        } else {
            format!("RTSP/1.0 454 Session Not Found\r\nCSeq: {}\r\n\r\n", cseq)
        }
    } else {
        format!("RTSP/1.0 454 Session Not Found\r\nCSeq: {}\r\n\r\n", cseq)
    }
}

async fn handle_teardown(
    cseq: u32,
    session_id_opt: Option<String>,
    sessions: &Arc<parking_lot::RwLock<HashMap<String, RtspSession>>>
) -> String {
    if let Some(sess_id) = session_id_opt {
        let mut lock = sessions.write();
        if let Some(session) = lock.get_mut(&sess_id) {
            let response = match session.process_teardown() {
                Ok(response) => response,
                Err(_) => "200 OK\r\n".to_string(),
            };

            // Actually remove the session after processing
            drop(lock); // Explicitly release the lock
            sessions.write().remove(&sess_id);
            
            format!("RTSP/1.0 200 OK\r\nCSeq: {}\r\n{}\r\n", cseq, response)
        } else {
            format!("RTSP/1.0 454 Session Not Found\r\nCSeq: {}\r\n\r\n", cseq)
        }
    } else {
        format!("RTSP/1.0 454 Session Not Found\r\nCSeq: {}\r\n\r\n", cseq)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wfd_capabilities() {
        let mut caps = WfdCapabilities::new();
        caps.set_parameter("wfd_video_formats", "1 0 00 04 0001F437FDE63F490000000000000000")
            .unwrap();
        
        let result = caps.get_parameter("wfd_video_formats").unwrap();
        assert_eq!(result, Some("1 0 00 04 0001F437FDE63F490000000000000000"));
    }

    #[test]
    fn test_session_states() {
        let mut session = RtspSession::new("test_session".to_string());
        assert_eq!(session.state, SessionState::Init);
        
        session.transition_to(SessionState::OptionsReceived);
        assert_eq!(session.state, SessionState::OptionsReceived);
        
        session.transition_to(SessionState::Play);
        assert_eq!(session.state, SessionState::Play);
    }
}