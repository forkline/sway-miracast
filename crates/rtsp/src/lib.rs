use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

/// Negotiated video codec for the WFD connection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NegotiatedCodec {
    H264,
    H265,
    AV1,
}

/// Wi-Fi Display (WFD) capabilities structure containing device display and streaming properties
///
/// Represents the capabilities that are exchanged between Miracast source and sink during
/// the WFD negotiation phase. These capabilities define the video, audio, and supported
/// features that both devices should agree on before beginning streaming.
///
/// # Examples
///
/// ```
/// use swaybeam_rtsp::WfdCapabilities;
///
/// let mut caps = WfdCapabilities::new();
/// caps.set_parameter("wfd_video_formats", "1 0 00 04 0001F437FDE63F490000000000000000").unwrap();
/// caps.set_parameter("wfd_audio_codecs", "AAC 00000002 00").unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct WfdCapabilities {
    /// Client RTP port information used for video/audio streaming
    pub client_rtp_ports: Option<String>,
    /// Supported video formats including profile, level, and resolutions
    pub video_formats: Option<String>,
    /// Available audio codecs and capabilities
    pub audio_codecs: Option<String>,
    /// Extended display identification data (optional)
    pub display_edid: Option<String>,
    /// Coupled sink capability (for interactive control)
    pub coupled_sink: Option<String>,
    /// User input back channel (UIBC) capability information
    pub uibc_capability: Option<String>,
    /// Standby/resume capability status
    pub standby_resume_capability: Option<String>,
    /// Content protection methods (HDCP, etc.)
    pub content_protection: Option<String>,
}

impl Default for WfdCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

impl WfdCapabilities {
    /// Creates a new WfdCapabilities instance with all capabilities set to None
    ///
    /// # Returns
    /// A WfdCapabilities instance with all capability fields unset
    ///
    /// # Examples
    ///
    /// ```
    /// use swaybeam_rtsp::WfdCapabilities;
    ///
    /// let caps = WfdCapabilities::new();
    /// assert!(caps.video_formats.is_none());
    /// assert!(caps.audio_codecs.is_none());
    /// ```
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

    /// Sets a WFD parameter by name and value
    ///
    /// Processes a parameter name-value pair and stores it in the appropriate field
    /// of the capabilities structure. This method is typically called during SET_PARAMETER
    /// message processing.
    ///
    /// # Arguments
    /// * `param_name` - The name of the parameter to set (e.g., "wfd_video_formats")
    /// * `value` - The value to assign to the parameter
    ///
    /// # Returns
    /// * `Ok(())` if the parameter was successfully set
    /// * `Err(RtspError::InvalidParameter)` if the parameter name is unknown
    ///
    /// # Examples
    ///
    /// ```
    /// use swaybeam_rtsp::WfdCapabilities;
    ///
    /// let mut caps = WfdCapabilities::new();
    /// caps.set_parameter("wfd_video_formats", "test_format").unwrap();
    /// assert_eq!(caps.video_formats.as_ref().unwrap(), "test_format");
    /// ```
    pub fn set_parameter(&mut self, param_name: &str, value: &str) -> Result<(), RtspError> {
        match param_name {
            "wfd_client_rtp_ports" => self.client_rtp_ports = Some(value.to_string()),
            "wfd_video_formats" => self.video_formats = Some(value.to_string()),
            "wfd_audio_codecs" => self.audio_codecs = Some(value.to_string()),
            "wfd_display_edid" => self.display_edid = Some(value.to_string()),
            "wfd_coupled_sink" => self.coupled_sink = Some(value.to_string()),
            "wfd_uibc_capability" => self.uibc_capability = Some(value.to_string()),
            "wfd_standby_resume_capability" => {
                self.standby_resume_capability = Some(value.to_string())
            }
            "wfd_content_protection" => self.content_protection = Some(value.to_string()),
            _ => return Err(RtspError::InvalidParameter(param_name.to_string())),
        }
        Ok(())
    }

    /// Gets the value of a WFD parameter by name
    ///
    /// Retrieves the current value of the specified parameter, or None if it hasn't been set.
    /// This method is typically used during GET_PARAMETER message processing.
    ///
    /// # Arguments
    /// * `param_name` - The name of the parameter to retrieve (e.g., "wfd_video_formats")
    ///
    /// # Returns
    /// * `Ok(Some(&str))` - The current value of the parameter if it exists
    /// * `Ok(None)` - If the parameter has not been set
    /// * `Err(RtspError::InvalidParameter)` - If the parameter name is unknown
    ///
    /// # Examples
    ///
    /// ```
    /// use swaybeam_rtsp::WfdCapabilities;
    ///
    /// let mut caps = WfdCapabilities::new();
    /// caps.video_formats = Some("test_format".to_string());
    ///
    /// assert_eq!(caps.get_parameter("wfd_video_formats").unwrap(), Some("test_format"));
    /// assert_eq!(caps.get_parameter("wfd_unknown").is_err(), true);
    /// ```
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

impl WfdCapabilities {
    /// Negotiate the best video codec based on sink capabilities
    pub fn negotiate_video_codec(&self) -> NegotiatedCodec {
        // Parse video_formats to determine supported codecs
        if let Some(formats) = &self.video_formats {
            // H.265 has better support in modern 4K TVs - check first
            if self.supports_h265(formats) {
                return NegotiatedCodec::H265;
            }
            // H.264 is universally supported
            if self.supports_h264(formats) {
                return NegotiatedCodec::H264;
            }
        }
        // Default to H.264
        NegotiatedCodec::H264
    }

    fn supports_h264(&self, formats: &str) -> bool {
        // H.264 is indicated by specific bits in the format string
        // Standard Miracast always supports H.264
        formats.contains("00") || formats.contains("01") || formats.contains("02")
    }

    fn supports_h265(&self, formats: &str) -> bool {
        // H.265 support is indicated in extended WFD
        // Check for H.265 indicator in format string
        formats.contains("HEVC") || formats.contains("hevc") || self.has_hevc_flag(formats)
    }

    fn has_hevc_flag(&self, formats: &str) -> bool {
        // Parse hex format for HEVC indicator
        // Extended WFD uses specific bit patterns
        let parts: Vec<&str> = formats.split_whitespace().collect();
        if parts.len() >= 4 {
            // Check video formats bitmask (typically the last component)
            if let Ok(mask) = u64::from_str_radix(parts[3], 16) {
                // H.265 is bit 4 in the lower bits (0x10)
                // 000000000000001F would mean bits 0:4 are set (H264 + some H265)
                return (mask & 0x0000000000000010) != 0; // Check bit 4 for H.265
            }
        }
        false
    }

    /// Create source capabilities advertising all supported codecs
    pub fn source_capabilities() -> Self {
        WfdCapabilities {
            video_formats: Some(Self::build_video_formats()),
            audio_codecs: Some(Self::build_audio_codecs()),
            ..Default::default()
        }
    }

    fn build_video_formats() -> String {
        // Build WFD video formats string advertising H.264, H.265, AV1
        // Format: "01 02 10 0000000000000000" (example)
        // Version 01, preferred display mode 02 (native), UIBC 10
        // Video formats: bitmask of supported codecs
        format!("01 01 00 {:016X}", Self::supported_codecs_mask())
    }

    fn supported_codecs_mask() -> u64 {
        // Bitmask of all supported video codecs
        // H.264: bits 0-3 (all profiles)
        // H.265: bits 4-7 (if supported)
        // AV1: bits 8-11 (if supported, extension)
        let mut mask: u64 = 0;

        // H.264 baseline, main, high profiles
        mask |= 0x0001; // H.264 baseline
        mask |= 0x0002; // H.264 main
        mask |= 0x0004; // H.264 high

        // H.265 main profile
        mask |= 0x0010; // H.265 main

        // AV1 (extended WFD, may not be standard)
        // mask |= 0x0100; // AV1

        mask
    }

    fn build_audio_codecs() -> String {
        // AAC is the primary codec, LPCM optional
        // Format: "AAC 00000001 00 LPCM 00000001 00"
        "AAC 00000001 00".to_string()
    }
}

/// Represents the current state of an RTSP/WFD session
///
/// The state machine drives the negotiation process between Miracast source and sink,
/// tracking which phase of the RTSP/WFD protocol the connection is currently executing.
/// This follows the state machine outlined in the Miracast and RTSP specifications.
///
/// # States
///
/// * `Init`: Initial state when the session begins
/// * `OptionsReceived`: After OPTIONS command received and processed
/// * `GetParamReceived`: After one or more GET_PARAMETER commands processed
/// * `SetParamReceived`: After SET_PARAMETER commands have been processed
/// * `Play`: Streaming is active, session in operating mode
/// * `Teardown`: Session has been terminated after TEARDOWN command
///
/// # Examples
///
/// ```
/// use swaybeam_rtsp::SessionState;
///
/// let state = SessionState::Init;
/// match state {
///     SessionState::Init => println!("Starting negotiation"),
///     SessionState::Play => println!("Streaming active"),
///     _ => println!("Other state"),
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum SessionState {
    Init,
    OptionsReceived,
    GetParamReceived,
    SetParamReceived,
    Play,
    Teardown,
}

/// Maintains the state information for an individual RTSP/WFD session
///
/// Contains all the relevant information for an ongoing Miracast session including
/// connection state, negotiated capabilities, and session-specific parameters.
/// Each active RTSP connection corresponds to exactly one `RtspSession` instance.
///
/// # Examples
///
/// ```
/// use swaybeam_rtsp::{RtspSession, WfdCapabilities, SessionState};
///
/// let session = RtspSession::new("session_123".to_string());
/// assert_eq!(session.session_id, "session_123");
/// assert_eq!(session.state, SessionState::Init);
/// ```
#[derive(Debug, Clone)]
pub struct RtspSession {
    /// Unique identifier for this session instance
    pub session_id: String,
    /// Current negotiation state in the RTSP/WFD state machine
    pub state: SessionState,
    /// Negotiated Wi-Fi Display capabilities for this session
    pub capabilities: WfdCapabilities,
    /// Additional session parameters beyond WFD standard
    pub parameters: HashMap<String, String>,
    /// Negotiated video codec determined during WFD negotiation
    pub negotiated_codec: Option<NegotiatedCodec>,
}

impl RtspSession {
    /// Creates a new RTSP session with provided session ID
    ///
    /// Initializes a session in the `Init` state with empty capabilities and parameters.
    /// The session ID can be any unique identifier, though typically generated using a
    /// random or timestamp-based approach.
    ///
    /// # Arguments
    /// * `session_id` - A unique identifier for this session
    ///
    /// # Returns
    /// A new RtspSession configured with the specified ID and default values
    ///
    /// # Examples
    ///
    /// ```rust
    /// use swaybeam_rtsp::RtspSession;
    ///
    /// let session = RtspSession::new("test_session".to_string());
    /// assert_eq!(session.session_id, "test_session");
    /// ```
    pub fn new(session_id: String) -> Self {
        RtspSession {
            session_id,
            state: SessionState::Init,
            capabilities: WfdCapabilities::new(),
            parameters: HashMap::new(),
            negotiated_codec: None,
        }
    }

    /// Transitions the session to a new state in the RTSP/WFD state machine
    ///
    /// Updates the session's state field. This method provides direct access to state
    /// transitions, though generally the session will update its own state through the
    /// various processing methods (`process_options`, `process_play`, etc.).
    ///
    /// # Arguments
    /// * `new_state` - The state to transition to
    ///
    /// # Examples
    ///
    /// ```rust
    /// use swaybeam_rtsp::{RtspSession, SessionState};
    ///
    /// let mut session = RtspSession::new("test_session".to_string());
    /// session.transition_to(SessionState::Play);
    /// assert_eq!(session.state, SessionState::Play);
    /// ```
    pub fn transition_to(&mut self, new_state: SessionState) {
        self.state = new_state;
    }

    /// Processes an OPTIONS RTSP command and updates session state accordingly
    ///
    /// Handles an RTSP OPTIONS request by returning the available methods the
    /// RTSP server supports. According to the Miracast specification, this should
    /// return the public methods for Wi-Fi Display negotiation.
    ///
    /// # Returns
    /// * `Ok(String)` - Response containing supported public methods
    /// * `Err(RtspError)` - If something fails during processing
    ///
    /// # Examples
    ///
    /// ```rust
    /// use swaybeam_rtsp::RtspSession;
    ///
    /// let mut session = RtspSession::new("test_session".to_string());
    /// let response = session.process_options().unwrap();
    /// assert!(response.contains("OPTIONS"));
    /// assert!(response.contains("GET_PARAMETER"));
    /// assert!(response.contains("SET_PARAMETER"));
    /// assert!(response.contains("PLAY"));
    /// assert!(response.contains("TEARDOWN"));
    /// ```
    pub fn process_options(&mut self) -> Result<String, RtspError> {
        self.transition_to(SessionState::OptionsReceived);
        Ok("Public: OPTIONS, GET_PARAMETER, SET_PARAMETER, PLAY, TEARDOWN\r\n".to_string())
    }

    /// Processes a GET_PARAMETER RTSP command for the specified parameter names
    ///
    /// Responds with the values of the requested parameters from the session's
    /// WFD capabilities. This is used to query the currently negotiated values
    /// during parameter agreement.
    ///
    /// # Arguments
    /// * `params` - List of parameter names to return in the response
    ///
    /// # Returns
    /// * `Ok(String)` - Formatted response containing parameter name-value pairs
    /// * `Err(RtspError)` - If parameter retrieval fails
    pub fn process_get_parameter(&mut self, params: &[&str]) -> Result<String, RtspError> {
        let mut response = String::new();

        for param in params {
            let value = self.capabilities.get_parameter(param)?;
            if let Some(val) = value {
                // Special handling for video formats to return negotiated codec info
                if *param == "wfd_video_formats" {
                    response.push_str(&self.build_video_formats_response());
                } else {
                    response.push_str(&format!("{}: {}\r\n", param, val));
                }
            }
        }

        self.transition_to(SessionState::GetParamReceived);
        Ok(response)
    }

    /// Processes a SET_PARAMETER RTSP command with provided parameter map
    ///
    /// Stores the given parameters in the session's WFD capabilities structure
    /// and updates the session parameters map. This is used for negotiating
    /// video formats, audio capabilities, and other Wi-Fi Display settings.
    ///
    /// # Arguments
    /// * `params` - Map of parameter names to their values
    ///
    /// # Returns
    /// * `Ok(String)` - Confirmation response
    /// * `Err(RtspError)` - If parameter validation fails
    pub fn process_set_parameter(
        &mut self,
        params: &HashMap<String, String>,
    ) -> Result<String, RtspError> {
        for (param_name, value) in params {
            self.capabilities.set_parameter(param_name, value)?;
            self.parameters.insert(param_name.clone(), value.clone());
        }

        // After receiving video formats, negotiate codec
        if self.capabilities.video_formats.is_some() {
            self.negotiated_codec = Some(self.capabilities.negotiate_video_codec());
        }

        self.transition_to(SessionState::SetParamReceived);
        Ok("200 OK\r\n".to_string())
    }

    /// Builds the response for wfd_video_formats parameter based on negotiated codec
    pub fn build_video_formats_response(&self) -> String {
        match self.negotiated_codec {
            Some(NegotiatedCodec::H265) => {
                "wfd_video_formats: 01 01 00 000000000000001F\r\n".to_string()
            }
            Some(NegotiatedCodec::AV1) => {
                "wfd_video_formats: 01 01 00 0000000000000100\r\n".to_string()
            }
            _ => {
                // H.264 default
                "wfd_video_formats: 01 01 00 0000000000000007\r\n".to_string()
            }
        }
    }

    /// Returns the negotiated video codec
    pub fn get_negotiated_codec(&self) -> Option<NegotiatedCodec> {
        self.negotiated_codec
    }

    /// Processes a PLAY command to begin streaming
    ///
    /// Transitions the session to the Play state, indicating that streaming
    /// has begun or is ready to begin. Generates the necessary status response.
    ///
    /// # Returns
    /// * `Ok(String)` - Response confirming PLAY command has started
    /// * `Err(RtspError)` - If operation fails
    pub fn process_play(&mut self) -> Result<String, RtspError> {
        self.transition_to(SessionState::Play);
        // For now, just return a basic play response
        Ok("RTP-Info: url=rtsp://server.example.com/movie/, seq=123456\r\n".to_string())
    }

    /// Processes a TEARDOWN command to end the session
    ///
    /// Transitions the session to the Teardown state, ending the RTSP session
    /// and indicating that the connection will be closed.
    ///
    /// # Returns
    /// * `Ok(String)` - Response confirming TEARDOWN was processed
    /// * `Err(RtspError)` - If operation fails
    pub fn process_teardown(&mut self) -> Result<String, RtspError> {
        self.transition_to(SessionState::Teardown);
        Ok("200 OK\r\n".to_string())
    }
}

/// Comprehensive error type for RTSP and WFD protocol operations
///
/// Enumerates all possible error conditions that can occur during RTSP/WFD
/// communication, including system-level issues (IO), protocol violations,
/// and invalid state transitions. This enables precise error handling and
/// debugging of Miracast connection issues.
///
/// # Examples
///
/// ```
/// # use swaybeam_rtsp::RtspError;
/// use std::io;
///
/// let io_error = io::Error::new(io::ErrorKind::ConnectionAborted, "Connection lost");
/// let rtsp_error: RtspError = io_error.into();
///
/// match rtsp_error {
///     RtspError::Io(ioe) => eprintln!("System error: {}", ioe),
///     RtspError::ProtocolViolation(msg) => eprintln!("Protocol error: {}", msg),
///     RtspError::SessionNotFound => eprintln!("Inactive session"),
///     _ => eprintln!("Other error"),
/// }
/// ```
#[derive(thiserror::Error, Debug)]
pub enum RtspError {
    #[error("IO Error: {0}")]
    /// System-level I/O error (connection lost, socket errors, disk issues)
    Io(#[from] std::io::Error),

    #[error("Parse Error: {0}")]
    /// Error during message parsing (malformed RTSP requests, wrong format)
    Parse(String),

    #[error("Invalid Parameter: {0}")]
    /// Attempt to access/set an unsupported WFD parameter
    InvalidParameter(String),

    #[error("Invalid Request Method: {0}")]
    /// RTSP method not recognized/supported (e.g., DESCRIBE which isn't used in WFD)
    InvalidMethod(String),

    #[error("Invalid State Transition")]
    /// Attempt to perform operation inappropriate for current session state
    InvalidStateTransition,

    #[error("Session Not Found")]
    /// Operation referenced a non-existent or expired session ID
    SessionNotFound,

    #[error("Request Timeout")]
    /// Request did not receive response within expected timeframe
    Timeout,

    #[error("Protocol Violation: {0}")]
    /// Protocol-level error (violates Miracast/WFD/RTSP specification)
    ProtocolViolation(String),
}

/// Representation of different RTSP message types processed by the server
///
/// Parses and categorizes incoming RTSP requests into typed variants to enable
/// specific handling for each command in accordance with the Miracast specification.
///
/// # Variants
///
/// * `Options` - Capabilities request with sequence number
/// * `GetParameter` - Parameter query request with sequence number and specific parameter names
/// * `SetParameter` - Parameter configuration request with sequence number and parameter values
/// * `Play` - Stream activation request with sequence number and optional session
/// * `Teardown` - Session termination request with sequence and session ID
///
/// # Examples
///
/// ```rust
/// # use swaybeam_rtsp::{RtspMessage, SessionState};
/// let msg = RtspMessage::Options { cseq: 1 };
///
/// match msg {
///     RtspMessage::Options { cseq } => println!("Processing options, sequence {}", cseq),
///     _ => println!("Other message"),
/// }
/// ```
#[derive(Debug)]
pub enum RtspMessage {
    Options {
        cseq: u32,
    },
    GetParameter {
        cseq: u32,
        params: Vec<String>,
    },
    SetParameter {
        cseq: u32,
        params: HashMap<String, String>,
    },
    Play {
        cseq: u32,
        session: Option<String>,
    },
    Teardown {
        cseq: u32,
        session: Option<String>,
    },
}

impl RtspMessage {
    /// Parse an RTSP message string into one of the known message types
    ///
    /// Performs RTSP message parsing according to the RTSP specification (RFC 2326),
    /// extracting the method, CSeq (command sequence), and method-specific parameters.
    /// This is fundamental for RTSP message routing and state machine execution.
    ///
    /// # Arguments
    /// * `data` - Raw RTSP message string to parse
    ///
    /// # Returns
    /// * `Ok(RtspMessage)` - Successfully parsed message with extracted parameters
    /// * `Err(RtspError)` - Parsing failed due to malformed message
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use swaybeam_rtsp::RtspMessage;
    ///
    /// let data = "OPTIONS * RTSP/1.0\r\nCSeq: 1\r\n\r\n";
    /// let msg = RtspMessage::parse(data).unwrap();
    ///
    /// match msg {
    ///     RtspMessage::Options { cseq } => assert_eq!(cseq, 1),
    ///     _ => panic!("Wrong type"),
    /// }
    /// ```
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
        let cseq_line = lines
            .iter()
            .find(|line| line.starts_with("CSeq:"))
            .ok_or_else(|| RtspError::Parse("Missing CSeq".to_string()))?;

        let cseq: u32 = cseq_line[5..]
            .trim()
            .parse()
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

                Ok(RtspMessage::GetParameter { cseq, params })
            }
            "SET_PARAMETER" => {
                let mut params = HashMap::new();

                // Parse WFD parameters in the message body
                for line in lines.iter().skip(1) {
                    if line.contains("wfd_") && line.contains(':') {
                        let param_parts: Vec<&str> = line.splitn(2, ':').collect();
                        if param_parts.len() == 2 {
                            params.insert(
                                param_parts[0].trim().to_string(),
                                param_parts[1].trim().to_string(),
                            );
                        }
                    }
                }

                Ok(RtspMessage::SetParameter { cseq, params })
            }
            "PLAY" => {
                let session = parse_header(&lines, "Session");
                Ok(RtspMessage::Play { cseq, session })
            }
            "TEARDOWN" => {
                let session = parse_header(&lines, "Session");
                Ok(RtspMessage::Teardown { cseq, session })
            }
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

/// RTSP Server implementation for handling Miracast/WFD negotiations
///
/// Listens on a configured TCP port for incoming RTSP connections and maintains
/// concurrent session state for multiple connected clients. Handles all aspects of
/// the RTSP/WFD protocol including message parsing, state machine execution, and
/// connection management according to the Miracast specification.
///
/// # Examples
///
/// Basic usage:
///
/// ```no_run
/// # use swaybeam_rtsp::RtspServer;
///
/// #[tokio::main]
/// async fn main() {
///     let server = RtspServer::new("127.0.0.1:7236".to_string());
///     server.start().await.expect("Server failed to start");
/// }
/// ```
#[derive(Debug)]
pub struct RtspServer {
    /// Network address the server binds to (e.g., "127.0.0.1:7236")
    address: String,
    /// Thread-safe collection of active sessions indexed by session ID
    sessions: Arc<parking_lot::RwLock<HashMap<String, RtspSession>>>,
    /// Token used to signal cancellation for graceful server shutdown
    cancellation_token: CancellationToken,
}

impl RtspServer {
    /// Creates a new RTSP server instance with the provided bind address
    ///
    /// # Arguments
    /// * `address` - IP:port combination to bind the server to (e.g., "127.0.0.1:7236")
    ///
    /// # Returns
    /// A new server instance with empty session store
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use swaybeam_rtsp::RtspServer;
    /// let server = RtspServer::new("127.0.0.1:7236".to_string());
    /// // Server is ready to accept connections
    /// ```
    pub fn new(address: String) -> Self {
        RtspServer {
            address,
            sessions: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            cancellation_token: CancellationToken::new(),
        }
    }

    /// Starts the RTSP server and begins listening for connections
    ///
    /// This is an async method that indefinitely listens for TCP connections on the
    /// configured address. Each connection is handled concurrently in a separate tokio
    /// task while session state is managed in shared storage. The method blocks until
    /// the server encounters an unrecoverable error.
    ///
    /// # Returns
    /// * `Ok(())` - Server shut down successfully on cancellation
    /// * `Err(RtspError::Io)` - Socket binding or connection acceptance failed
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use swaybeam_rtsp::RtspServer;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let server = RtspServer::new("127.0.0.1:7236".to_string());
    ///     server.start().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn start(&self) -> Result<(), RtspError> {
        let listener = TcpListener::bind(&self.address).await?;
        tracing::info!("RTSP server listening on {}", self.address);

        loop {
            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((stream, addr)) => {
                            tracing::info!("Connection established from {}", addr);

                            let sessions = self.sessions.clone();

                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(stream, sessions).await {
                                    tracing::error!("Error handling connection: {:?}", e);
                                }
                            });
                        }
                        Err(e) => {
                            tracing::error!("Failed to accept connection: {:?}", e);
                        }
                    }
                }
                _ = self.cancellation_token.cancelled() => {
                    tracing::info!("RTSP server shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Retrieve a session by its ID from the session store
    ///
    /// # Arguments
    /// * `session_id` - Session ID to look up in active sessions
    ///
    /// # Returns
    /// * `Some(RtspSession)` - Session found
    /// * `None` - Session does not exist or has expired
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use swaybeam_rtsp::{RtspServer, RtspSession};
    ///
    /// let server = RtspServer::new("127.0.0.1:0".to_string());
    /// let session = server.create_session("test_123".to_string());
    ///
    /// let retrieved = server.get_session("test_123");
    /// assert!(retrieved.is_some());
    /// assert_eq!(retrieved.unwrap().session_id, "test_123");
    /// ```
    pub fn get_session(&self, session_id: &str) -> Option<RtspSession> {
        self.sessions.read().get(session_id).cloned()
    }

    /// Creates and registers a new session in the server's session store
    ///
    /// # Arguments
    /// * `session_id` - The unique identifier to use for the new session
    ///
    /// # Returns
    /// The complete newly-created session instance (also stored internally)
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use swaybeam_rtsp::RtspServer;
    ///
    /// let server = RtspServer::new("127.0.0.1:0".to_string());
    /// let session = server.create_session("new_session_456".to_string());
    /// assert_eq!(session.session_id, "new_session_456");
    ///
    /// // The session is also stored internally
    /// let stored = server.get_session("new_session_456");
    /// assert!(stored.is_some());
    /// ```
    pub fn create_session(&self, session_id: String) -> RtspSession {
        let session = RtspSession::new(session_id.clone());
        self.sessions.write().insert(session_id, session.clone());
        session
    }

    /// Removes a session from the server's session store
    ///
    /// Typically called when a TEARDOWN command completes or the connection drops
    /// unexpectedly to free up resources.
    ///
    /// # Arguments
    /// * `session_id` - The session ID to remove from the session store
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use swaybeam_rtsp::RtspServer;
    ///
    /// let server = RtspServer::new("127.0.0.1:0".to_string());
    /// server.create_session("temp_session".to_string());
    /// assert!(server.get_session("temp_session").is_some());
    ///
    /// server.remove_session("temp_session");
    /// assert!(server.get_session("temp_session").is_none());
    /// ```
    pub fn remove_session(&self, session_id: &str) {
        self.sessions.write().remove(session_id);
    }

    /// Signal the RTSP server to shut down gracefully
    ///
    /// Cancels the internal cancellation token which causes the server's start() method
    /// to exit its main loop. This allows for graceful shutdown of the RTSP server.
    pub fn stop(&self) {
        self.cancellation_token.cancel();
    }
}

impl Drop for RtspServer {
    fn drop(&mut self) {
        self.cancellation_token.cancel();
    }
}

async fn handle_connection(
    mut socket: TcpStream,
    sessions: Arc<parking_lot::RwLock<HashMap<String, RtspSession>>>,
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
            Ok(msg) => match msg {
                RtspMessage::Options { cseq } => handle_options(cseq, &sessions).await,
                RtspMessage::GetParameter { cseq, params } => {
                    handle_get_parameter(cseq, params, &sessions).await
                }
                RtspMessage::SetParameter { cseq, params } => {
                    handle_set_parameter(cseq, params, &sessions).await
                }
                RtspMessage::Play { cseq, session } => handle_play(cseq, session, &sessions).await,
                RtspMessage::Teardown { cseq, session } => {
                    handle_teardown(cseq, session, &sessions).await
                }
            },
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
    sessions: &Arc<parking_lot::RwLock<HashMap<String, RtspSession>>>,
) -> String {
    let session_id = format!("sess_{}", rand::random::<u64>());
    let mut session = RtspSession::new(session_id.clone());

    let caps_response = match session.process_options() {
        Ok(response) => response,
        Err(_) => "Public: OPTIONS, GET_PARAMETER, SET_PARAMETER, PLAY, TEARDOWN\r\n".to_string(),
    };

    sessions.write().insert(session_id, session);

    format!("RTSP/1.0 200 OK\r\nCSeq: {}\r\n{}\r\n", cseq, caps_response)
}

async fn handle_get_parameter(
    cseq: u32,
    param_names: Vec<String>,
    sessions: &Arc<parking_lot::RwLock<HashMap<String, RtspSession>>>,
) -> String {
    let mut lock = sessions.write();

    let maybe_session_id = lock.keys().last().cloned();

    if let Some(session_id) = maybe_session_id {
        if let Some(session) = lock.get_mut(&session_id) {
            let param_refs: Vec<&str> = param_names.iter().map(|s| s.as_str()).collect();
            let response = match session.process_get_parameter(&param_refs) {
                Ok(response) => response,
                Err(_) => "".to_string(),
            };

            format!("RTSP/1.0 200 OK\r\nCSeq: {}\r\n{}\r\n", cseq, response)
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
    sessions: &Arc<parking_lot::RwLock<HashMap<String, RtspSession>>>,
) -> String {
    let mut lock = sessions.write();

    let maybe_session_id = lock.keys().last().cloned();

    if let Some(session_id) = maybe_session_id {
        if let Some(session) = lock.get_mut(&session_id) {
            let response = match session.process_set_parameter(&params) {
                Ok(response) => response,
                Err(_) => "200 OK\r\n".to_string(),
            };

            format!("RTSP/1.0 200 OK\r\nCSeq: {}\r\n{}\r\n", cseq, response)
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
    sessions: &Arc<parking_lot::RwLock<HashMap<String, RtspSession>>>,
) -> String {
    let sess_id = session_id_opt.or_else(|| {
        let lock = sessions.read();
        lock.keys().last().cloned()
    });

    if let Some(session_id) = sess_id {
        let mut lock = sessions.write();
        if let Some(session) = lock.get_mut(&session_id) {
            let response = match session.process_play() {
                Ok(response) => response,
                Err(_) => "RTP-info: url=rtsp://server/, seq=123456\r\n".to_string(),
            };

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
    sessions: &Arc<parking_lot::RwLock<HashMap<String, RtspSession>>>,
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
        caps.set_parameter(
            "wfd_video_formats",
            "1 0 00 04 0001F437FDE63F490000000000000000",
        )
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

    #[tokio::test]
    async fn test_shutdown_mechanism() {
        // Create a test server
        let server = RtspServer::new("127.0.0.1:0".to_string());
        let server_handle = server.cancellation_token.clone();

        // Spawn a task to test the cancellation token functionality
        let shutdown_task = tokio::spawn(async move {
            // Server will wait for cancellation token
            server_handle.cancelled().await;
            // If we get here, cancellation was triggered
            42 // arbitrary value to show the task completed
        });

        // Give a small delay, then trigger shutdown
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        server.stop();

        // Wait for task to complete (indicating cancellation was triggered)
        let result =
            tokio::time::timeout(tokio::time::Duration::from_millis(100), shutdown_task).await;
        assert!(result.is_ok(), "Cancellation task should have completed");

        let final_result = result.unwrap();
        assert!(
            final_result.is_ok(),
            "Task should have completed successfully"
        );
        assert_eq!(
            final_result.unwrap(),
            42,
            "Should return our expected value"
        );
    }

    #[test]
    fn test_codec_negotiation_h264() {
        let mut caps = WfdCapabilities::new();
        caps.video_formats = Some("01 01 00 0000000000000007".to_string());
        assert_eq!(caps.negotiate_video_codec(), NegotiatedCodec::H264);
    }

    #[test]
    fn test_codec_negotiation_h265() {
        let mut caps = WfdCapabilities::new();
        caps.video_formats = Some("01 01 00 000000000000001F".to_string());
        assert_eq!(caps.negotiate_video_codec(), NegotiatedCodec::H265);
    }

    #[test]
    fn test_source_capabilities() {
        let caps = WfdCapabilities::source_capabilities();
        assert!(caps.video_formats.is_some());
        assert!(caps.audio_codecs.is_some());
    }
}
