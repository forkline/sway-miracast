//! WFD Parameter Validation Tool
//! Validates Wi-Fi Display (WFD) parameters according to WFD specification

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use swaybeam_rtsp::WfdCapabilities;

#[derive(Parser)]
#[command(name = "validate-wfd")]
#[command(about = "Validate WFD parameters for Miracast compliance")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate video format parameter
    Video {
        /// Video formats string
        #[arg(short, long)]
        format: String,
        /// Show detailed format breakdown
        #[arg(short, long)]
        detailed: bool,
    },
    /// Validate audio codec parameter
    Audio {
        /// Audio codecs string
        #[arg(short, long)]
        codec: String,
        /// Show detailed codec breakdown
        #[arg(short, long)]
        detailed: bool,
    },
    /// Validate RTP port specification
    Port {
        /// Port specification string
        #[arg(short, long)]
        spec: String,
    },
    /// Validate all WFD capabilities
    Caps {
        /// Read capabilities from file
        #[arg(short, long)]
        file: Option<String>,
        /// Interactive mode for entering parameters
        #[arg(short, long)]
        interactive: bool,
    },
    /// Negotiate codec between source and sink
    Negotiate {
        /// Sink video formats
        #[arg(short, long)]
        sink: String,
        /// Show negotiation details
        #[arg(short, long)]
        detailed: bool,
    },
    /// Generate sample WFD parameters
    Sample {
        /// Parameter type: video, audio, port, full
        #[arg(short, long, default_value = "full")]
        type_: String,
    },
}

#[derive(Debug, Clone)]
struct WfdValidationResult {
    valid: bool,
    errors: Vec<String>,
    warnings: Vec<String>,
    details: HashMap<String, String>,
    parsed_values: HashMap<String, String>,
}

impl WfdValidationResult {
    fn new() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            details: HashMap::new(),
            parsed_values: HashMap::new(),
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

    fn add_parsed(&mut self, key: String, value: String) {
        self.parsed_values.insert(key, value);
    }

    fn print(&self) {
        if self.valid {
            println!("✓ VALID");
        } else {
            println!("✗ INVALID");
        }

        if !self.parsed_values.is_empty() {
            println!("\nParsed Values:");
            for (k, v) in &self.parsed_values {
                println!("  {}: {}", k, v);
            }
        }

        if !self.details.is_empty() {
            println!("\nDetails:");
            for (k, v) in &self.details {
                println!("  {}: {}", k, v);
            }
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

fn validate_video_formats(format: &str, detailed: bool) -> WfdValidationResult {
    let mut result = WfdValidationResult::new();

    if format.is_empty() {
        result.add_error("Empty video formats string".to_string());
        return result;
    }

    let parts: Vec<&str> = format.split_whitespace().collect();

    result.add_parsed("Raw input".to_string(), format.to_string());
    result.add_parsed("Field count".to_string(), parts.len().to_string());

    if parts.len() < 4 {
        result.add_error(format!("Expected at least 4 fields, got {}", parts.len()));
        return result;
    }

    let version = parts[0];
    match version.parse::<u8>() {
        Ok(v) => {
            if v <= 2 {
                result.add_parsed("Version".to_string(), format!("WFD {}", v));
            } else {
                result.add_warning(format!("Unknown WFD version: {}", v));
            }
        }
        Err(_) => {
            result.add_error(format!("Invalid version field: {}", version));
        }
    }

    let preferred_mode = parts[1];
    match preferred_mode.parse::<u8>() {
        Ok(v) => {
            let mode_desc = match v {
                0 => "Secondary sink",
                1 => "Preferred - source prefers sink",
                2 => "Source preferred",
                _ => "Unknown",
            };
            result.add_parsed("Display mode".to_string(), mode_desc.to_string());
        }
        Err(_) => {
            result.add_error(format!("Invalid preferred mode: {}", preferred_mode));
        }
    }

    let uibc = parts[2];
    match uibc.parse::<u8>() {
        Ok(v) => {
            let uibc_desc = if v == 0 {
                "UIBC not supported"
            } else {
                "UIBC supported"
            };
            result.add_parsed("UIBC".to_string(), uibc_desc.to_string());
        }
        Err(_) => {
            result.add_warning(format!("Invalid UIBC field: {}", uibc));
        }
    }

    let codec_mask = parts[3];
    if codec_mask.len() != 16 {
        result.add_warning(format!(
            "Codec mask should be 16 hex digits, got {}",
            codec_mask.len()
        ));
    }

    match u64::from_str_radix(codec_mask, 16) {
        Ok(mask) => {
            result.add_parsed("Codec mask (hex)".to_string(), codec_mask.to_string());
            result.add_parsed("Codec mask (dec)".to_string(), mask.to_string());

            let h264_baseline = (mask & 0x0001) != 0;
            let h264_main = (mask & 0x0002) != 0;
            let h264_high = (mask & 0x0004) != 0;
            let h265_main = (mask & 0x0010) != 0;

            result.add_parsed(
                "H.264 Baseline".to_string(),
                if h264_baseline { "✓" } else { "✗" }.to_string(),
            );
            result.add_parsed(
                "H.264 Main".to_string(),
                if h264_main { "✓" } else { "✗" }.to_string(),
            );
            result.add_parsed(
                "H.264 High".to_string(),
                if h264_high { "✓" } else { "✗" }.to_string(),
            );
            result.add_parsed(
                "H.265 Main".to_string(),
                if h265_main { "✓" } else { "✗" }.to_string(),
            );

            if !h264_baseline && !h264_main && !h264_high {
                result.add_error("H.264 support is mandatory for WFD".to_string());
            }

            if detailed {
                let cea_resolutions: HashMap<u8, (u32, u32, u32)> = HashMap::from([
                    (0, (640, 480, 60)),
                    (1, (720, 480, 60)),
                    (4, (1280, 720, 60)),
                    (5, (1920, 1080, 30)),
                    (6, (1920, 1080, 60)),
                    (16, (1920, 1080, 24)),
                    (31, (3840, 2160, 30)),
                ]);

                println!("\nSupported resolutions (CEA bitmask):");
                for (bit, (w, h, fps)) in &cea_resolutions {
                    let supported = (mask & (1 << *bit)) != 0;
                    println!(
                        "  {}x{} @ {}Hz (bit {}): {}",
                        w,
                        h,
                        fps,
                        bit,
                        if supported { "✓" } else { "✗" }
                    );
                }
            }
        }
        Err(_) => {
            result.add_error(format!("Invalid codec mask hex: {}", codec_mask));
        }
    }

    if detailed && parts.len() > 4 {
        println!("\nAdditional fields:");
        for (i, part) in parts.iter().skip(4).enumerate() {
            println!("  Field {}: {}", i + 5, part);
        }
    }

    result
}

fn validate_audio_codecs(codec: &str, detailed: bool) -> WfdValidationResult {
    let mut result = WfdValidationResult::new();

    if codec.is_empty() {
        result.add_error("Empty audio codecs string".to_string());
        return result;
    }

    result.add_parsed("Raw input".to_string(), codec.to_string());

    let codec_entries: Vec<&str> = codec.split_whitespace().collect();

    if codec_entries.is_empty() {
        result.add_error("No audio codec entries found".to_string());
        return result;
    }

    let codec_type = codec_entries[0];
    let supported_codecs = ["AAC", "LPCM", "AC3", "AAC-LC"];

    if supported_codecs.contains(&codec_type) {
        result.add_parsed("Codec type".to_string(), codec_type.to_string());
    } else {
        result.add_warning(format!("Unknown codec type: {}", codec_type));
        result.add_parsed("Codec type".to_string(), codec_type.to_string());
    }

    if codec_entries.len() >= 2 {
        let modes = codec_entries[1];
        match u32::from_str_radix(modes, 16) {
            Ok(mode_mask) => {
                result.add_parsed("Modes mask".to_string(), format!("0x{}", modes));

                if detailed {
                    println!("\nAudio modes:");
                    let modes_list = [
                        (0x01, "2-channel stereo"),
                        (0x02, "5.1 surround"),
                        (0x04, "7.1 surround"),
                    ];
                    for (bit, desc) in &modes_list {
                        let supported = (mode_mask & *bit) != 0;
                        println!(
                            "  {} (bit {}): {}",
                            desc,
                            bit,
                            if supported { "✓" } else { "✗" }
                        );
                    }
                }
            }
            Err(_) => {
                result.add_warning(format!("Invalid modes mask: {}", modes));
            }
        }
    }

    if codec_entries.len() >= 3 {
        let latency = codec_entries[2];
        result.add_parsed("Latency".to_string(), latency.to_string());
    }

    if !codec_type.starts_with("AAC") && !codec_type.starts_with("LPCM") {
        result.add_warning("AAC or LPCM is mandatory for WFD 1.0".to_string());
    }

    result
}

fn validate_rtp_port_spec(spec: &str) -> WfdValidationResult {
    let mut result = WfdValidationResult::new();

    if spec.is_empty() {
        result.add_error("Empty RTP port specification".to_string());
        return result;
    }

    result.add_parsed("Raw input".to_string(), spec.to_string());

    if !spec.contains("RTP/AVP") {
        result.add_error("Missing RTP/AVP protocol identifier".to_string());
    } else {
        result.add_parsed("Protocol".to_string(), "RTP/AVP".to_string());
    }

    if !spec.contains("unicast") && !spec.contains("multicast") {
        result.add_warning("Missing unicast/multicast specification".to_string());
    } else if spec.contains("unicast") {
        result.add_parsed("Transport".to_string(), "unicast".to_string());
    } else {
        result.add_parsed("Transport".to_string(), "multicast".to_string());
    }

    let port_pattern = regex_lite_match(spec);
    if let Some(port_str) = port_pattern {
        match port_str.parse::<u16>() {
            Ok(port) => {
                if port >= 1024 {
                    result.add_parsed("RTP Port".to_string(), port.to_string());
                } else {
                    result.add_error(format!("Port {} out of valid range (1024-65535)", port));
                }
            }
            Err(_) => {
                result.add_error(format!("Invalid port number: {}", port_str));
            }
        }
    } else {
        result.add_error("No port number found in specification".to_string());
    }

    if spec.contains("mode=play") {
        result.add_parsed("Mode".to_string(), "play".to_string());
    } else {
        result.add_warning("Missing mode=play specification".to_string());
    }

    result
}

fn regex_lite_match(s: &str) -> Option<String> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    for part in parts {
        if part.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(num) = part.parse::<u64>() {
                if num > 0 && num <= 65535 {
                    return Some(part.to_string());
                }
            }
        }
    }
    None
}

fn validate_full_capabilities(params: HashMap<String, String>) -> WfdValidationResult {
    let mut result = WfdValidationResult::new();

    let mut caps = WfdCapabilities::new();

    for (key, value) in &params {
        match caps.set_parameter(key, value) {
            Ok(_) => {
                result.add_parsed(key.clone(), value.clone());
            }
            Err(e) => {
                result.add_error(format!("Failed to set {}: {}", key, e));
            }
        }
    }

    if caps.video_formats.is_some() {
        let video_result = validate_video_formats(caps.video_formats.as_ref().unwrap(), false);
        if !video_result.valid {
            result.add_error("Video formats validation failed".to_string());
        }
        for (k, v) in video_result.parsed_values {
            result.add_detail(format!("video.{}", k), v);
        }
    } else {
        result.add_warning("Missing wfd_video_formats parameter".to_string());
    }

    if caps.audio_codecs.is_some() {
        let audio_result = validate_audio_codecs(caps.audio_codecs.as_ref().unwrap(), false);
        if !audio_result.valid {
            result.add_error("Audio codecs validation failed".to_string());
        }
        for (k, v) in audio_result.parsed_values {
            result.add_detail(format!("audio.{}", k), v);
        }
    } else {
        result.add_warning("Missing wfd_audio_codecs parameter".to_string());
    }

    if caps.client_rtp_ports.is_some() {
        let port_result = validate_rtp_port_spec(caps.client_rtp_ports.as_ref().unwrap());
        if !port_result.valid {
            result.add_error("RTP ports validation failed".to_string());
        }
        for (k, v) in port_result.parsed_values {
            result.add_detail(format!("rtp.{}", k), v);
        }
    } else {
        result.add_warning("Missing wfd_client_rtp_ports parameter".to_string());
    }

    let negotiated = caps.negotiate_video_codec();
    result.add_detail("Negotiated codec".to_string(), format!("{:?}", negotiated));

    result
}

fn negotiate_codec(sink_formats: &str, detailed: bool) -> WfdValidationResult {
    let mut result = WfdValidationResult::new();

    let mut sink_caps = WfdCapabilities::new();
    sink_caps.video_formats = Some(sink_formats.to_string());

    let source_caps = WfdCapabilities::source_capabilities();

    result.add_parsed("Sink formats".to_string(), sink_formats.to_string());
    if let Some(src_formats) = &source_caps.video_formats {
        result.add_parsed("Source formats".to_string(), src_formats.clone());
    }

    let negotiated = sink_caps.negotiate_video_codec();
    result.add_parsed("Result".to_string(), format!("{:?}", negotiated));

    if detailed {
        println!("\nNegotiation process:");
        println!("  1. Parse sink capabilities");
        println!("  2. Check for H.265 support (preferred for 4K)");
        println!("  3. Fall back to H.264 if H.265 not available");
        println!("  4. Result: {:?}", negotiated);

        let mask_result = validate_video_formats(sink_formats, true);
        for (k, v) in mask_result.parsed_values {
            if k.contains("H.265") || k.contains("H.264") {
                result.add_detail(k, v);
            }
        }
    }

    result
}

fn generate_sample(type_: &str) -> String {
    match type_.to_lowercase().as_str() {
        "video" => WfdCapabilities::source_capabilities()
            .video_formats
            .unwrap_or_default(),
        "audio" => WfdCapabilities::source_capabilities()
            .audio_codecs
            .unwrap_or_default(),
        "port" => "RTP/AVP/UDP;unicast 19000 0 mode=play".to_string(),
        "full" => {
            let caps = WfdCapabilities::source_capabilities();
            let mut output = String::new();
            if let Some(v) = caps.video_formats {
                output.push_str(&format!("wfd_video_formats: {}\n", v));
            }
            if let Some(a) = caps.audio_codecs {
                output.push_str(&format!("wfd_audio_codecs: {}\n", a));
            }
            output.push_str("wfd_client_rtp_ports: RTP/AVP/UDP;unicast 19000 0 mode=play\n");
            output
        }
        _ => "Unknown sample type".to_string(),
    }
}

fn run_interactive_caps() -> Result<()> {
    println!("WFD Capabilities Interactive Validator");
    println!("Enter WFD parameters as 'name: value'. Empty line to validate, 'quit' to exit.\n");

    let stdin = std::io::stdin();
    let mut params: HashMap<String, String> = HashMap::new();

    loop {
        print!("> ");
        std::io::stdout().flush()?;

        let mut line = String::new();
        stdin.lock().read_line(&mut line)?;
        let trimmed = line.trim();

        if trimmed == "quit" || trimmed == "exit" {
            break;
        }

        if trimmed.is_empty() {
            if params.is_empty() {
                println!("No parameters to validate.");
                continue;
            }

            println!("\n--- Validating WFD Capabilities ---");
            let result = validate_full_capabilities(params.clone());
            result.print();
            println!("\n--- Parameters cleared ---\n");
            params.clear();
            continue;
        }

        let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
        if parts.len() == 2 {
            let name = parts[0].trim().to_string();
            let value = parts[1].trim().to_string();
            params.insert(name, value);
            println!("Parameter '{}' added.", parts[0].trim());
        } else {
            println!("Invalid format. Use 'name: value'");
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Video { format, detailed } => {
            println!("=== Video Format Validation ===\n");
            let result = validate_video_formats(&format, detailed);
            result.print();
        }
        Commands::Audio { codec, detailed } => {
            println!("=== Audio Codec Validation ===\n");
            let result = validate_audio_codecs(&codec, detailed);
            result.print();
        }
        Commands::Port { spec } => {
            println!("=== RTP Port Specification Validation ===\n");
            let result = validate_rtp_port_spec(&spec);
            result.print();
        }
        Commands::Caps { file, interactive } => {
            if interactive {
                run_interactive_caps()?;
            } else if let Some(path) = file {
                let content = std::fs::read_to_string(&path)?;
                let params: HashMap<String, String> = content
                    .lines()
                    .filter_map(|line| {
                        let parts: Vec<&str> = line.splitn(2, ':').collect();
                        if parts.len() == 2 {
                            Some((parts[0].trim().to_string(), parts[1].trim().to_string()))
                        } else {
                            None
                        }
                    })
                    .collect();

                println!("=== WFD Capabilities Validation ===\n");
                println!("Parameters: {}", params.len());
                let result = validate_full_capabilities(params);
                result.print();
            } else {
                return Err(anyhow::anyhow!(
                    "Either --file or --interactive must be provided"
                ));
            }
        }
        Commands::Negotiate { sink, detailed } => {
            println!("=== Codec Negotiation ===\n");
            let result = negotiate_codec(&sink, detailed);
            result.print();
        }
        Commands::Sample { type_ } => {
            println!("=== Sample WFD Parameters ===\n");
            println!("{}", generate_sample(&type_));
        }
    }

    Ok(())
}
