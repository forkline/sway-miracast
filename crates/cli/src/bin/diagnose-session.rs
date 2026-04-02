//! Session Diagnostic Tool for Miracast
//! Diagnoses session issues, logs and analyzes session flow, simulates sink behaviors

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::time::{Duration, Instant};
use swaybeam_rtsp::{NegotiatedCodec, RtspMessage, RtspSession, SessionState, WfdCapabilities};

#[derive(Parser)]
#[command(name = "diagnose-session")]
#[command(about = "Diagnose Miracast/WFD session issues")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze a session log file
    Analyze {
        /// Session log file
        #[arg(short, long)]
        file: String,
        /// Show timing analysis
        #[arg(short, long)]
        timing: bool,
        /// Show state machine analysis
        #[arg(short, long)]
        states: bool,
    },
    /// Simulate different sink behaviors
    Simulate {
        /// Sink type: standard, slow, aggressive, minimal
        #[arg(short, long, default_value = "standard")]
        sink: String,
        /// Run full session simulation
        #[arg(short, long)]
        full: bool,
    },
    /// Diagnose common session problems
    Diagnose {
        /// Problem description or error message
        #[arg(short, long)]
        error: Option<String>,
        /// Interactive problem diagnosis
        #[arg(short, long)]
        interactive: bool,
    },
    /// Generate session report
    Report {
        /// Session data file
        #[arg(short, long)]
        file: Option<String>,
        /// Output format: text, json
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

#[derive(Debug)]
#[allow(dead_code)]
struct SessionLogEntry {
    timestamp: Duration,
    direction: String,
    message_type: String,
    raw_message: String,
    parsed_successful: bool,
}

#[derive(Debug, Clone)]
struct SessionAnalysis {
    total_messages: usize,
    message_counts: HashMap<String, usize>,
    state_transitions: Vec<(SessionState, SessionState, Duration)>,
    timing_stats: HashMap<String, Duration>,
    errors: Vec<String>,
    warnings: Vec<String>,
    negotiated_codec: Option<NegotiatedCodec>,
    final_state: SessionState,
}

impl SessionAnalysis {
    fn new() -> Self {
        Self {
            total_messages: 0,
            message_counts: HashMap::new(),
            state_transitions: Vec::new(),
            timing_stats: HashMap::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
            negotiated_codec: None,
            final_state: SessionState::Init,
        }
    }

    fn print(&self) {
        println!("\n=== Session Analysis Report ===\n");

        println!("Messages:");
        println!("  Total: {}", self.total_messages);
        for (type_, count) in &self.message_counts {
            println!("  {}: {}", type_, count);
        }

        println!("\nState Transitions:");
        for (from, to, duration) in &self.state_transitions {
            println!(
                "  {:?} -> {:?} ({:.2}ms)",
                from,
                to,
                duration.as_secs_f64() * 1000.0
            );
        }
        println!("  Final state: {:?}", self.final_state);

        println!("\nTiming:");
        for (phase, duration) in &self.timing_stats {
            println!("  {}: {:.2}s", phase, duration.as_secs_f64());
        }

        println!("\nCodec:");
        match self.negotiated_codec {
            Some(codec) => println!("  Negotiated: {:?}", codec),
            None => println!("  Not negotiated"),
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
    }
}

fn parse_session_log(content: &str) -> Vec<SessionLogEntry> {
    let mut entries = Vec::new();
    let mut start_time: Option<Instant> = None;

    for line in content.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.splitn(4, '|').collect();
        if parts.len() >= 3 {
            let time_str = parts[0].trim();
            let direction = parts[1].trim().to_string();
            let msg_type = parts[2].trim().to_string();
            let raw = if parts.len() == 4 {
                parts[3].trim().to_string()
            } else {
                "".to_string()
            };

            let timestamp = parse_timestamp(time_str);

            if start_time.is_none() {
                start_time = Some(Instant::now());
            }

            let parsed = RtspMessage::parse(&raw).ok();

            entries.push(SessionLogEntry {
                timestamp,
                direction,
                message_type: msg_type,
                raw_message: raw,
                parsed_successful: parsed.is_some(),
            });
        } else {
            let timestamp = if start_time.is_none() {
                start_time = Some(Instant::now());
                Duration::ZERO
            } else {
                Duration::from_millis(entries.len() as u64 * 100)
            };

            let parsed = RtspMessage::parse(line).ok();
            let msg_type = match &parsed {
                Some(RtspMessage::Options { .. }) => "OPTIONS",
                Some(RtspMessage::GetParameter { .. }) => "GET_PARAMETER",
                Some(RtspMessage::SetParameter { .. }) => "SET_PARAMETER",
                Some(RtspMessage::Play { .. }) => "PLAY",
                Some(RtspMessage::Teardown { .. }) => "TEARDOWN",
                None => "UNKNOWN",
            };

            entries.push(SessionLogEntry {
                timestamp,
                direction: "IN".to_string(),
                message_type: msg_type.to_string(),
                raw_message: line.to_string(),
                parsed_successful: parsed.is_some(),
            });
        }
    }

    entries
}

fn parse_timestamp(s: &str) -> Duration {
    if s.contains(':') || s.contains('.') {
        let parts: Vec<&str> = s.split(&[':', '.'][..]).collect();
        if parts.len() >= 2 {
            let secs: u64 = parts[0].parse().unwrap_or(0);
            let ms: u64 = parts[1].parse().unwrap_or(0);
            return Duration::from_secs(secs) + Duration::from_millis(ms);
        }
    }
    if let Ok(ms) = s.parse::<u64>() {
        return Duration::from_millis(ms);
    }
    Duration::ZERO
}

fn analyze_session(entries: Vec<SessionLogEntry>) -> SessionAnalysis {
    let mut analysis = SessionAnalysis::new();
    let mut session = RtspSession::new("analysis_session".to_string());
    let _prev_state = session.state.clone();
    let mut state_start = Duration::ZERO;

    analysis.total_messages = entries.len();

    for entry in &entries {
        *analysis
            .message_counts
            .entry(entry.message_type.clone())
            .or_insert(0) += 1;

        if !entry.parsed_successful && !entry.raw_message.is_empty() {
            analysis.errors.push(format!(
                "Failed to parse message at {:?}: {}",
                entry.timestamp, entry.raw_message
            ));
        }

        let msg = RtspMessage::parse(&entry.raw_message).ok();
        if let Some(parsed) = msg {
            let before_state = session.state.clone();

            match parsed {
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

            if session.state != before_state {
                let duration = entry.timestamp - state_start;
                analysis.state_transitions.push((
                    before_state.clone(),
                    session.state.clone(),
                    duration,
                ));
                state_start = entry.timestamp;
            }
        }
    }

    analysis.negotiated_codec = session.negotiated_codec;
    analysis.final_state = session.state.clone();

    if !entries.is_empty() {
        analysis.timing_stats.insert(
            "Total duration".to_string(),
            entries.last().unwrap().timestamp,
        );
    }

    if !analysis.state_transitions.is_empty() {
        let init_to_options = analysis
            .state_transitions
            .iter()
            .find(|(f, _, _)| format!("{:?}", f) == format!("{:?}", SessionState::Init));
        if let Some((_, _, d)) = init_to_options {
            analysis
                .timing_stats
                .insert("OPTIONS phase".to_string(), *d);
        }

        let options_to_setparam = analysis
            .state_transitions
            .iter()
            .find(|(f, _, _)| format!("{:?}", f) == format!("{:?}", SessionState::OptionsReceived));
        if let Some((_, _, d)) = options_to_setparam {
            analysis
                .timing_stats
                .insert("Negotiation phase".to_string(), *d);
        }

        let to_play = analysis
            .state_transitions
            .iter()
            .find(|(_, t, _)| format!("{:?}", t) == format!("{:?}", SessionState::Play));
        if let Some((_, _, d)) = to_play {
            analysis.timing_stats.insert("Play setup".to_string(), *d);
        }
    }

    if session.capabilities.video_formats.is_none() {
        analysis
            .warnings
            .push("No video formats exchanged".to_string());
    }
    if session.capabilities.audio_codecs.is_none() {
        analysis
            .warnings
            .push("No audio codecs exchanged".to_string());
    }
    if session.capabilities.client_rtp_ports.is_none() {
        analysis.warnings.push("No RTP ports specified".to_string());
    }

    analysis
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum SinkBehavior {
    Standard,
    Slow,
    Aggressive,
    Minimal,
    Custom(HashMap<String, String>),
}

impl SinkBehavior {
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "standard" => Ok(SinkBehavior::Standard),
            "slow" => Ok(SinkBehavior::Slow),
            "aggressive" => Ok(SinkBehavior::Aggressive),
            "minimal" => Ok(SinkBehavior::Minimal),
            _ => Err(anyhow::anyhow!("Unknown sink type: {}", s)),
        }
    }

    fn get_capabilities(&self) -> WfdCapabilities {
        match self {
            SinkBehavior::Standard => WfdCapabilities {
                video_formats: Some("01 01 00 0000000000000007".to_string()),
                audio_codecs: Some("AAC 00000001 00".to_string()),
                client_rtp_ports: Some("RTP/AVP/UDP;unicast 19000 0 mode=play".to_string()),
                ..Default::default()
            },
            SinkBehavior::Slow => WfdCapabilities {
                video_formats: Some("01 02 00 0000000000000003".to_string()),
                audio_codecs: Some("AAC 00000001 00".to_string()),
                client_rtp_ports: Some("RTP/AVP/UDP;unicast 19000 0 mode=play".to_string()),
                ..Default::default()
            },
            SinkBehavior::Aggressive => WfdCapabilities {
                video_formats: Some("01 01 00 000000000000001F".to_string()),
                audio_codecs: Some("AAC 00000003 00 LPCM 00000001 00".to_string()),
                client_rtp_ports: Some("RTP/AVP/UDP;unicast 19000 0 mode=play".to_string()),
                ..Default::default()
            },
            SinkBehavior::Minimal => WfdCapabilities {
                video_formats: Some("00 04 0001F437FDE63F490000000000000000".to_string()),
                audio_codecs: Some("AAC 00000001 00".to_string()),
                client_rtp_ports: Some("RTP/AVP/UDP;unicast 19000 0 mode=play".to_string()),
                ..Default::default()
            },
            SinkBehavior::Custom(params) => {
                let mut caps = WfdCapabilities::new();
                for (k, v) in params {
                    let _ = caps.set_parameter(k, v);
                }
                caps
            }
        }
    }
}

fn simulate_session(sink_type: &str, full: bool) -> Result<()> {
    let sink = SinkBehavior::from_str(sink_type)?;
    let sink_caps = sink.get_capabilities();

    println!("=== Simulating Session with {} Sink ===\n", sink_type);
    println!("Sink capabilities:");
    if let Some(v) = &sink_caps.video_formats {
        println!("  Video: {}", v);
    }
    if let Some(a) = &sink_caps.audio_codecs {
        println!("  Audio: {}", a);
    }
    if let Some(p) = &sink_caps.client_rtp_ports {
        println!("  Ports: {}", p);
    }

    println!("\n--- Starting Session Simulation ---\n");

    let mut session = RtspSession::new("sim_session".to_string());
    let mut timeline: Vec<(String, SessionState, String)> = Vec::new();

    timeline.push((
        "OPTIONS received".to_string(),
        SessionState::Init,
        "Source <- Sink: OPTIONS * RTSP/1.0 CSeq: 1".to_string(),
    ));

    let options_response = session.process_options()?;
    timeline.push((
        "OPTIONS response".to_string(),
        session.state.clone(),
        format!(
            "Source -> Sink: RTSP/1.0 200 OK CSeq: 1 {}",
            options_response
        ),
    ));

    timeline.push((
        "GET_PARAMETER received".to_string(),
        session.state.clone(),
        "Source <- Sink: GET_PARAMETER CSeq: 2 wfd_video_formats".to_string(),
    ));

    let get_response = session.process_get_parameter(&["wfd_video_formats"])?;
    timeline.push((
        "GET_PARAMETER response".to_string(),
        session.state.clone(),
        format!("Source -> Sink: RTSP/1.0 200 OK CSeq: 2 {}", get_response),
    ));

    let mut params = HashMap::new();
    if let Some(v) = &sink_caps.video_formats {
        params.insert("wfd_video_formats".to_string(), v.clone());
    }
    if let Some(a) = &sink_caps.audio_codecs {
        params.insert("wfd_audio_codecs".to_string(), a.clone());
    }
    if let Some(p) = &sink_caps.client_rtp_ports {
        params.insert("wfd_client_rtp_ports".to_string(), p.clone());
    }

    timeline.push((
        "SET_PARAMETER received".to_string(),
        session.state.clone(),
        format!(
            "Source <- Sink: SET_PARAMETER CSeq: 3 wfd_video_formats: {}",
            sink_caps.video_formats.as_ref().unwrap_or(&"".to_string())
        ),
    ));

    let set_response = session.process_set_parameter(&params)?;
    timeline.push((
        "SET_PARAMETER response".to_string(),
        session.state.clone(),
        format!("Source -> Sink: RTSP/1.0 200 OK CSeq: 3 {}", set_response),
    ));

    if full {
        timeline.push((
            "PLAY received".to_string(),
            session.state.clone(),
            "Source <- Sink: PLAY CSeq: 4 Session: sim_session".to_string(),
        ));

        let play_response = session.process_play()?;
        timeline.push((
            "PLAY response".to_string(),
            session.state.clone(),
            format!("Source -> Sink: RTSP/1.0 200 OK CSeq: 4 {}", play_response),
        ));

        timeline.push((
            "Streaming active".to_string(),
            session.state.clone(),
            "Source -> Sink: RTP video stream".to_string(),
        ));

        println!("--- Streaming Phase (simulated 5 seconds) ---");

        timeline.push((
            "TEARDOWN received".to_string(),
            SessionState::Play,
            "Source <- Sink: TEARDOWN CSeq: 5 Session: sim_session".to_string(),
        ));

        let teardown_response = session.process_teardown()?;
        timeline.push((
            "TEARDOWN response".to_string(),
            session.state.clone(),
            format!(
                "Source -> Sink: RTSP/1.0 200 OK CSeq: 5 {}",
                teardown_response
            ),
        ));
    }

    println!("--- Session Timeline ---\n");
    for (event, state, msg) in &timeline {
        println!("[{:?}] {}", state, event);
        println!("  {}", msg);
        println!();
    }

    println!("--- Session Summary ---\n");
    println!("Final state: {:?}", session.state);
    println!(
        "Negotiated codec: {:?}",
        session.negotiated_codec.unwrap_or(NegotiatedCodec::H264)
    );

    Ok(())
}

struct ProblemDiagnoser {
    problems: Vec<(String, String, Vec<String>)>,
}

impl ProblemDiagnoser {
    fn new() -> Self {
        Self {
            problems: vec![
                (
                    "Connection timeout".to_string(),
                    "P2P connection failed to establish".to_string(),
                    vec![
                        "Check Wi-Fi Direct is enabled on both devices".to_string(),
                        "Verify network manager supports P2P".to_string(),
                        "Check firewall isn't blocking P2P ports".to_string(),
                    ],
                ),
                (
                    "RTSP negotiation failure".to_string(),
                    "Session not established after OPTIONS".to_string(),
                    vec![
                        "Verify RTSP server is running on port 7236".to_string(),
                        "Check CSeq values are incrementing".to_string(),
                        "Ensure session header is included after OPTIONS".to_string(),
                    ],
                ),
                (
                    "Codec mismatch".to_string(),
                    "Video codec negotiation failed".to_string(),
                    vec![
                        "Check sink supports H.264 baseline profile".to_string(),
                        "Verify wfd_video_formats parameter is valid".to_string(),
                        "Try different resolution/framerate settings".to_string(),
                    ],
                ),
                (
                    "Stream startup failure".to_string(),
                    "Pipeline failed to start streaming".to_string(),
                    vec![
                        "Check GStreamer plugins are installed".to_string(),
                        "Verify PipeWire is capturing screen".to_string(),
                        "Check RTP port is open and accessible".to_string(),
                    ],
                ),
                (
                    "Audio not working".to_string(),
                    "Audio stream not reaching sink".to_string(),
                    vec![
                        "Verify AAC codec is supported by sink".to_string(),
                        "Check audio bitrate is within limits".to_string(),
                        "Ensure audio RTP port is configured".to_string(),
                    ],
                ),
                (
                    "Poor video quality".to_string(),
                    "Stream is choppy or low resolution".to_string(),
                    vec![
                        "Reduce video bitrate to match network".to_string(),
                        "Lower resolution to 720p".to_string(),
                        "Check Wi-Fi signal strength".to_string(),
                    ],
                ),
            ],
        }
    }

    fn diagnose(&self, description: &str) -> Vec<(String, String, Vec<String>)> {
        let desc_lower = description.to_lowercase();

        self.problems
            .iter()
            .filter(|(name, desc, _)| {
                desc_lower.contains(&name.to_lowercase())
                    || desc_lower.contains(&desc.to_lowercase())
                    || desc_lower.contains("timeout")
                    || desc_lower.contains("failed")
                    || desc_lower.contains("error")
            })
            .cloned()
            .collect()
    }

    fn list_problems(&self) {
        println!("Known problems:");
        for (i, (name, desc, _)) in self.problems.iter().enumerate() {
            println!("{}. {} - {}", i + 1, name, desc);
        }
    }
}

fn run_interactive_diagnosis() -> Result<()> {
    println!("Session Problem Interactive Diagnoser");
    println!("Enter problem description or select a known problem number.");
    println!("'list' to show known problems, 'quit' to exit.\n");

    let diagnoser = ProblemDiagnoser::new();
    let stdin = io::stdin();

    loop {
        print!("> ");
        io::stdout().flush()?;

        let mut line = String::new();
        stdin.lock().read_line(&mut line)?;
        let trimmed = line.trim();

        if trimmed == "quit" || trimmed == "exit" {
            break;
        }

        if trimmed == "list" {
            diagnoser.list_problems();
            continue;
        }

        if let Ok(num) = trimmed.parse::<usize>() {
            if num > 0 && num <= diagnoser.problems.len() {
                let (name, desc, solutions) = &diagnoser.problems[num - 1];
                println!("\n=== {} ===", name);
                println!("Description: {}", desc);
                println!("\nSolutions:");
                for s in solutions {
                    println!("  • {}", s);
                }
                println!();
                continue;
            }
        }

        let results = diagnoser.diagnose(trimmed);
        if results.is_empty() {
            println!("No matching problems found. Try 'list' to see known problems.");
        } else {
            println!("\nMatching problems:\n");
            for (name, desc, solutions) in results {
                println!("=== {} ===", name);
                println!("Description: {}", desc);
                println!("Solutions:");
                for s in solutions {
                    println!("  • {}", s);
                }
                println!();
            }
        }
    }

    Ok(())
}

fn generate_report(file: Option<&str>, format: &str) -> Result<()> {
    let entries = if let Some(path) = file {
        let content = std::fs::read_to_string(path)?;
        parse_session_log(&content)
    } else {
        println!(
            "No file provided. Enter session messages (empty line to finish, 'quit' to exit):"
        );
        let stdin = io::stdin();
        let mut lines = Vec::new();

        loop {
            print!("> ");
            io::stdout().flush()?;

            let mut line = String::new();
            stdin.lock().read_line(&mut line)?;
            let trimmed = line.trim();

            if trimmed == "quit" {
                break;
            }

            if trimmed.is_empty() {
                break;
            }

            lines.push(trimmed.to_string());
        }

        lines
            .iter()
            .map(|l| {
                let parsed = RtspMessage::parse(l).ok();
                let msg_type = match &parsed {
                    Some(RtspMessage::Options { .. }) => "OPTIONS",
                    Some(RtspMessage::GetParameter { .. }) => "GET_PARAMETER",
                    Some(RtspMessage::SetParameter { .. }) => "SET_PARAMETER",
                    Some(RtspMessage::Play { .. }) => "PLAY",
                    Some(RtspMessage::Teardown { .. }) => "TEARDOWN",
                    None => "UNKNOWN",
                };
                SessionLogEntry {
                    timestamp: Duration::from_millis(0),
                    direction: "IN".to_string(),
                    message_type: msg_type.to_string(),
                    raw_message: l.clone(),
                    parsed_successful: parsed.is_some(),
                }
            })
            .collect()
    };

    let analysis = analyze_session(entries);

    if format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "total_messages": analysis.total_messages,
                "message_counts": analysis.message_counts,
                "state_transitions": analysis.state_transitions.iter().map(|(f, t, d)| {
                    serde_json::json!({
                        "from": format!("{:?}", f),
                        "to": format!("{:?}", t),
                        "duration_ms": d.as_millis()
                    })
                }).collect::<Vec<_>>(),
                "timing": analysis.timing_stats.iter().map(|(k, v)| {
                    serde_json::json!({
                        "phase": k,
                        "duration_s": v.as_secs_f64()
                    })
                }).collect::<Vec<_>>(),
                "negotiated_codec": format!("{:?}", analysis.negotiated_codec),
                "final_state": format!("{:?}", analysis.final_state),
                "errors": analysis.errors,
                "warnings": analysis.warnings
            }))?
        );
    } else {
        analysis.print();
    }

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze {
            file,
            timing,
            states,
        } => {
            println!("=== Session Log Analysis ===\n");
            let content = std::fs::read_to_string(&file)?;
            let entries = parse_session_log(&content);
            let analysis = analyze_session(entries);

            if states {
                println!("--- State Machine Analysis ---\n");
                for (from, to, duration) in &analysis.state_transitions {
                    println!("{:?} -> {:?} ({:.2}ms)", from, to, duration.as_millis());
                }
                println!("\nFinal state: {:?}", analysis.final_state);
            }

            if timing {
                println!("\n--- Timing Analysis ---\n");
                for (phase, duration) in &analysis.timing_stats {
                    println!("{}: {:.2}s", phase, duration.as_secs_f64());
                }
            }

            if !timing && !states {
                analysis.print();
            }
        }
        Commands::Simulate { sink, full } => {
            simulate_session(&sink, full)?;
        }
        Commands::Diagnose { error, interactive } => {
            if interactive {
                run_interactive_diagnosis()?;
            } else if let Some(err) = error {
                println!("=== Diagnosing: {} ===\n", err);
                let diagnoser = ProblemDiagnoser::new();
                let results = diagnoser.diagnose(&err);

                if results.is_empty() {
                    diagnoser.list_problems();
                } else {
                    for (name, desc, solutions) in results {
                        println!("=== {} ===", name);
                        println!("Description: {}", desc);
                        println!("Solutions:");
                        for s in solutions {
                            println!("  • {}", s);
                        }
                        println!();
                    }
                }
            } else {
                return Err(anyhow::anyhow!(
                    "Either --error or --interactive must be provided"
                ));
            }
        }
        Commands::Report { file, format } => {
            println!("=== Session Report ===\n");
            generate_report(file.as_deref(), &format)?;
        }
    }

    Ok(())
}
