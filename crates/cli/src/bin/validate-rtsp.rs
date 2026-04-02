//! RTSP Message Validation Tool
//! Validates RTSP message format according to RFC 2326 and WFD specification

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use swaybeam_rtsp::{RtspMessage, RtspSession, SessionState};

#[derive(Parser)]
#[command(name = "validate-rtsp")]
#[command(about = "Validate RTSP messages for Miracast/WFD compliance")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate a single RTSP message
    Message {
        /// RTSP message content (or use --file)
        #[arg(short, long)]
        message: Option<String>,
        /// Read message from file
        #[arg(short, long)]
        file: Option<String>,
        /// Validate strict RFC 2326 compliance
        #[arg(short, long)]
        strict: bool,
    },
    /// Validate RTSP state transitions
    State {
        /// Initial state
        #[arg(short, long, default_value = "init")]
        from: String,
        /// Target state
        #[arg(short, long)]
        to: String,
    },
    /// Validate a complete session flow
    Session {
        /// Session messages file (one message per line)
        #[arg(short, long)]
        file: Option<String>,
        /// Interactive mode
        #[arg(short, long)]
        interactive: bool,
    },
    /// Generate valid RTSP messages for testing
    Generate {
        /// Message type to generate
        #[arg(short, long)]
        type_: String,
        /// CSeq number
        #[arg(short, long, default_value = "1")]
        cseq: u32,
    },
}

#[derive(Debug, Clone)]
struct ValidationResult {
    valid: bool,
    errors: Vec<String>,
    warnings: Vec<String>,
    details: HashMap<String, String>,
}

impl ValidationResult {
    fn new() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            details: HashMap::new(),
        }
    }

    fn add_error(&mut self, error: String) {
        self.valid = false;
        self.errors.push(error);
    }

    fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }

    fn add_detail(&mut self, key: String, value: String) {
        self.details.insert(key, value);
    }

    fn print(&self) {
        if self.valid {
            println!("✓ VALID");
        } else {
            println!("✗ INVALID");
        }

        if !self.errors.is_empty() {
            println!("\nErrors:");
            for e in &self.errors {
                println!("  ✗ {}", e);
            }
        }

        if !self.warnings.is_empty() {
            println!("\nWarnings:");
            for w in &self.warnings {
                println!("  ⚠ {}", w);
            }
        }

        if !self.details.is_empty() {
            println!("\nDetails:");
            for (k, v) in &self.details {
                println!("  {}: {}", k, v);
            }
        }
    }
}

fn validate_rtsp_message(message: &str, strict: bool) -> ValidationResult {
    let mut result = ValidationResult::new();

    let lines: Vec<&str> = message.lines().collect();

    if lines.is_empty() {
        result.add_error("Empty message".to_string());
        return result;
    }

    let first_line = lines[0];
    let parts: Vec<&str> = first_line.split_whitespace().collect();

    if parts.len() < 3 {
        result.add_error(format!(
            "Malformed request line: expected at least 3 parts, got {}",
            parts.len()
        ));
        return result;
    }

    let method = parts[0];
    let target = parts[1];
    let version = parts[2];

    result.add_detail("Method".to_string(), method.to_string());
    result.add_detail("Target".to_string(), target.to_string());
    result.add_detail("Version".to_string(), version.to_string());

    let valid_methods = [
        "OPTIONS",
        "GET_PARAMETER",
        "SET_PARAMETER",
        "PLAY",
        "TEARDOWN",
    ];
    if !valid_methods.contains(&method) {
        result.add_error(format!("Invalid method: {}", method));
    }

    if version != "RTSP/1.0" {
        result.add_error(format!("Invalid RTSP version: {}", version));
    }

    if strict && target != "*" && !target.starts_with("rtsp://") {
        result.add_warning(format!("Target should be '*' or rtsp:// URL: {}", target));
    }

    let mut found_cseq = false;
    let mut cseq_value: Option<u32> = None;

    for line in &lines[1..] {
        if line.is_empty() {
            break;
        }

        let header_parts: Vec<&str> = line.splitn(2, ':').collect();
        if header_parts.len() == 2 {
            let header_name = header_parts[0].trim();
            let header_value = header_parts[1].trim();

            if header_name == "CSeq" {
                found_cseq = true;
                match header_value.parse::<u32>() {
                    Ok(v) => {
                        cseq_value = Some(v);
                        result.add_detail("CSeq".to_string(), v.to_string());
                    }
                    Err(_) => {
                        result.add_error(format!("Invalid CSeq value: {}", header_value));
                    }
                }
            }
        }
    }

    if !found_cseq {
        result.add_error("Missing required CSeq header".to_string());
    }

    if cseq_value.is_none() && found_cseq {
        result.add_error("CSeq header present but value is invalid".to_string());
    }

    let body_start = lines.iter().position(|l| l.is_empty());
    if let Some(idx) = body_start {
        let body_lines = &lines[idx + 1..];
        if !body_lines.is_empty() {
            result.add_detail("Body lines".to_string(), body_lines.len().to_string());

            for body_line in body_lines {
                if body_line.contains("wfd_") && !body_line.contains(':') {
                    result.add_warning(format!("WFD parameter line missing colon: {}", body_line));
                }
            }
        }
    }

    if strict {
        if !message.ends_with("\r\n\r\n") {
            result.add_error("Message must end with double CRLF (\\r\\n\\r\\n)".to_string());
        }

        for line in &lines {
            if !line.is_empty() && !message.contains("\r\n") {
                result.add_warning("Lines should use CRLF (\\r\\n) line endings".to_string());
                break;
            }
        }
    }

    let parsed = RtspMessage::parse(message);
    match parsed {
        Ok(msg) => {
            let msg_type = match msg {
                RtspMessage::Options { .. } => "OPTIONS",
                RtspMessage::GetParameter { .. } => "GET_PARAMETER",
                RtspMessage::SetParameter { .. } => "SET_PARAMETER",
                RtspMessage::Setup { .. } => "SETUP",
                RtspMessage::Play { .. } => "PLAY",
                RtspMessage::Teardown { .. } => "TEARDOWN",
            };
            result.add_detail("Parsed type".to_string(), msg_type.to_string());
        }
        Err(e) => {
            result.add_error(format!("Parser error: {}", e));
        }
    }

    result
}

fn parse_state(s: &str) -> Option<SessionState> {
    match s.to_lowercase().as_str() {
        "init" => Some(SessionState::Init),
        "optionsreceived" | "options" => Some(SessionState::OptionsReceived),
        "getparamreceived" | "getparam" | "get_parameter" => Some(SessionState::GetParamReceived),
        "setparamreceived" | "setparam" | "set_parameter" => Some(SessionState::SetParamReceived),
        "play" => Some(SessionState::Play),
        "teardown" => Some(SessionState::Teardown),
        _ => None,
    }
}

fn validate_state_transition(from: &str, to: &str) -> ValidationResult {
    let mut result = ValidationResult::new();

    let from_state = parse_state(from);
    let to_state = parse_state(to);

    if from_state.is_none() {
        result.add_error(format!("Unknown source state: {}", from));
        return result;
    }

    if to_state.is_none() {
        result.add_error(format!("Unknown target state: {}", to));
        return result;
    }

    let from_state = from_state.unwrap();
    let to_state = to_state.unwrap();

    result.add_detail("From state".to_string(), format!("{:?}", from_state));
    result.add_detail("To state".to_string(), format!("{:?}", to_state));

    let valid_transitions: HashMap<String, Vec<String>> = HashMap::from([
        ("Init".to_string(), vec!["OptionsReceived".to_string()]),
        (
            "OptionsReceived".to_string(),
            vec![
                "GetParamReceived".to_string(),
                "SetParamReceived".to_string(),
            ],
        ),
        (
            "GetParamReceived".to_string(),
            vec!["SetParamReceived".to_string(), "Play".to_string()],
        ),
        (
            "SetParamReceived".to_string(),
            vec!["GetParamReceived".to_string(), "Play".to_string()],
        ),
        (
            "Play".to_string(),
            vec!["Teardown".to_string(), "Init".to_string()],
        ),
        ("Teardown".to_string(), vec!["Init".to_string()]),
    ]);

    let from_key = format!("{:?}", from_state);
    let to_key = format!("{:?}", to_state);

    if let Some(allowed) = valid_transitions.get(&from_key) {
        if allowed.contains(&to_key) {
            result.add_detail("Transition".to_string(), "Valid".to_string());
        } else {
            result.add_error(format!(
                "Invalid transition from {:?} to {:?}. Allowed: {:?}",
                from_state, to_state, allowed
            ));
        }
    } else {
        result.add_warning(format!("No defined transitions from {:?}", from_state));
    }

    result
}

fn validate_session_flow(messages: Vec<String>) -> ValidationResult {
    let mut result = ValidationResult::new();
    let mut session = RtspSession::new("validation_session".to_string());
    let mut state_history = vec![session.state.clone()];

    result.add_detail("Initial state".to_string(), format!("{:?}", session.state));

    for (i, msg) in messages.iter().enumerate() {
        let msg_result = validate_rtsp_message(msg, false);
        if !msg_result.valid {
            result.add_error(format!("Message {} invalid: {}", i + 1, msg));
            continue;
        }

        let parsed = RtspMessage::parse(msg);
        match parsed {
            Ok(rtsp_msg) => {
                let prev_state = session.state.clone();
                match rtsp_msg {
                    RtspMessage::Options { cseq: _ } => {
                        let _ = session.process_options();
                    }
                    RtspMessage::GetParameter { cseq: _, params } => {
                        let param_refs: Vec<&str> = params.iter().map(|s| s.as_str()).collect();
                        let _ = session.process_get_parameter(&param_refs);
                    }
                    RtspMessage::SetParameter { cseq: _, params } => {
                        let _ = session.process_set_parameter(&params);
                    }
                    RtspMessage::Setup {
                        cseq: _,
                        session: _,
                        transport: _,
                    } => {
                        // Handle SETUP in session state
                        let fallback_transport =
                            Some("RTP/AVP;unicast;client_port=5004".to_string());
                        let _ = session.process_setup(fallback_transport);
                    }
                    RtspMessage::Play {
                        cseq: _,
                        session: _,
                    } => {
                        let _ = session.process_play();
                    }
                    RtspMessage::Teardown {
                        cseq: _,
                        session: _,
                    } => {
                        let _ = session.process_teardown();
                    }
                }
                let new_state = session.state.clone();
                if new_state != prev_state {
                    state_history.push(new_state.clone());
                    result.add_detail(
                        format!("Message {} state", i + 1),
                        format!("{:?} -> {:?}", prev_state, new_state),
                    );
                }
            }
            Err(e) => {
                result.add_error(format!("Failed to parse message {}: {}", i + 1, e));
            }
        }
    }

    result.add_detail("Final state".to_string(), format!("{:?}", session.state));
    result.add_detail("State history".to_string(), format!("{:?}", state_history));

    result
}

fn generate_message(type_: &str, cseq: u32) -> Result<String> {
    match type_.to_lowercase().as_str() {
        "options" => Ok(format!(
            "OPTIONS * RTSP/1.0\r\nCSeq: {}\r\nRequire: org.wfa.wfd1.0\r\n\r\n",
            cseq
        )),
        "get_parameter" => Ok(format!(
            "GET_PARAMETER rtsp://localhost/wfd1.0 RTSP/1.0\r\nCSeq: {}\r\nSession: sess_123\r\n\r\nwfd_video_formats\r\n",
            cseq
        )),
        "set_parameter" => Ok(format!(
            "SET_PARAMETER rtsp://localhost/wfd1.0 RTSP/1.0\r\nCSeq: {}\r\nSession: sess_123\r\n\r\nwfd_video_formats: 01 01 00 0000000000000007\r\n",
            cseq
        )),
        "play" => Ok(format!(
            "PLAY rtsp://localhost/wfd1.0/stream RTSP/1.0\r\nCSeq: {}\r\nSession: sess_123\r\n\r\n",
            cseq
        )),
        "teardown" => Ok(format!(
            "TEARDOWN rtsp://localhost/wfd1.0 RTSP/1.0\r\nCSeq: {}\r\nSession: sess_123\r\n\r\n",
            cseq
        )),
        _ => Err(anyhow::anyhow!("Unknown message type: {}", type_)),
    }
}

fn run_interactive_session() -> Result<()> {
    println!("RTSP Session Interactive Validator");
    println!("Enter RTSP messages (one per line). Empty line to validate, 'quit' to exit.\n");

    let stdin = io::stdin();
    let mut messages: Vec<String> = Vec::new();

    loop {
        print!("> ");
        io::stdout().flush()?;

        let mut line = String::new();
        stdin.lock().read_line(&mut line)?;
        let trimmed = line.trim();

        if trimmed == "quit" || trimmed == "exit" {
            break;
        }

        if trimmed.is_empty() {
            if messages.is_empty() {
                println!("No messages to validate.");
                continue;
            }

            println!("\n--- Validating Session Flow ---");
            let result = validate_session_flow(messages.clone());
            result.print();
            println!("\n--- Session cleared ---\n");
            messages.clear();
            continue;
        }

        messages.push(trimmed.to_string());
        println!("Message {} added.", messages.len());
    }

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Message {
            message,
            file,
            strict,
        } => {
            let msg_content = if let Some(path) = file {
                std::fs::read_to_string(&path)?
            } else if let Some(m) = message {
                m
            } else {
                return Err(anyhow::anyhow!(
                    "Either --message or --file must be provided"
                ));
            };

            println!("=== RTSP Message Validation ===\n");
            let result = validate_rtsp_message(&msg_content, strict);
            result.print();
        }
        Commands::State { from, to } => {
            println!("=== RTSP State Transition Validation ===\n");
            let result = validate_state_transition(&from, &to);
            result.print();
        }
        Commands::Session { file, interactive } => {
            if interactive {
                run_interactive_session()?;
            } else if let Some(path) = file {
                let content = std::fs::read_to_string(&path)?;
                let messages: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                println!("=== Session Flow Validation ===\n");
                println!("Messages: {}", messages.len());
                let result = validate_session_flow(messages);
                result.print();
            } else {
                return Err(anyhow::anyhow!(
                    "Either --file or --interactive must be provided"
                ));
            }
        }
        Commands::Generate { type_, cseq } => {
            let msg = generate_message(&type_, cseq)?;
            println!("=== Generated RTSP Message ===\n");
            println!("{}", msg);

            println!("\n--- Validation ---");
            let result = validate_rtsp_message(&msg, true);
            if result.valid {
                println!("Generated message is valid.");
            } else {
                println!("Generated message has issues:");
                result.print();
            }
        }
    }

    Ok(())
}
