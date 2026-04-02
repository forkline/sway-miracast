//! Common utilities for integration tests across crates

use std::collections::HashMap;
use std::os::unix::io::RawFd;
use std::process::{Command, Output};

use miracast_doctor::{CheckResult, CommandRunner as DoctorCommandRunner, Report};
use miracast_net::{
    CommandOutput as NetCommandOutput, CommandRunner as NetCommandRunner,
    MockCommandRunner as NetMockCommandRunner, NetError, P2pConfig, P2pConnection, P2pManager,
    RealCommandRunner as NetRealCommandRunner, Sink,
};
use miracast_stream::{StreamConfig, StreamPipeline};

/// Wrapper to combine the various command runners used by different crates
pub struct CombinedMockCommandRunner {
    doctor_responses: std::sync::Mutex<HashMap<String, std::io::Result<Output>>>,
    net_responses: std::sync::Mutex<HashMap<String, Result<NetCommandOutput, NetError>>>,
}

impl CombinedMockCommandRunner {
    pub fn new() -> Self {
        Self {
            doctor_responses: std::sync::Mutex::new(HashMap::new()),
            net_responses: std::sync::Mutex::new(HashMap::new()),
        }
    }

    pub fn add_doctor_response(
        &self,
        program: &str,
        args: &[&str],
        result: std::io::Result<Output>,
    ) {
        let key = format!("{} {}", program, args.join(" "));
        self.doctor_responses.lock().unwrap().insert(key, result);
    }

    pub fn add_net_response(
        &self,
        cmd: &str,
        args: &[&str],
        result: Result<NetCommandOutput, NetError>,
    ) {
        let key = format!("{} {}", cmd, args.join(" "));
        self.net_responses.lock().unwrap().insert(key, result);
    }
}

impl DoctorCommandRunner for CombinedMockCommandRunner {
    fn run_command_with_args(&self, program: &str, args: &[&str]) -> std::io::Result<Output> {
        let key = format!("{} {}", program, args.join(" "));
        let responses_guard = self.doctor_responses.lock().unwrap();

        responses_guard.get(&key).cloned().unwrap_or_else(|| {
            // Default response for unknown commands
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Command not mocked",
            ))
        })
    }
}

impl NetCommandRunner for CombinedMockCommandRunner {
    fn run(&self, cmd: &str, args: &[&str]) -> Result<NetCommandOutput, NetError> {
        let key = format!("{} {}", cmd, args.join(" "));
        let responses_guard = self.net_responses.lock().unwrap();

        responses_guard.get(&key).cloned().unwrap_or_else(|| {
            // Default response for unknown commands
            Err(NetError::CommandFailed(format!(
                "Command {} not mocked: {}",
                cmd,
                args.join(" ")
            )))
        })
    }
}

/// Mock implementation of capture for testing
pub struct MockCapture {
    config: String,
}

impl MockCapture {
    pub fn new(config: String) -> Self {
        Self { config }
    }

    pub fn start(&self) -> Result<MockStream, TestError> {
        // Create a default stream config based on the capture config
        let stream_config = StreamConfig::default();

        // Override with any stream-specific settings from capture config if needed
        Ok(MockStream::new(stream_config))
    }

    pub fn config(&self) -> &str {
        &self.config
    }
}

/// Mock implementation of stream for testing
pub struct MockStream {
    config: StreamConfig,
    state: StreamState,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StreamState {
    Initialized,
    Started,
    Stopped,
}

impl MockStream {
    pub fn new(config: StreamConfig) -> Self {
        Self {
            config,
            state: StreamState::Initialized,
        }
    }

    pub fn start(&mut self) -> Result<(), TestError> {
        self.state = StreamState::Started;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), TestError> {
        self.state = StreamState::Stopped;
        Ok(())
    }

    pub fn config(&self) -> &StreamConfig {
        &self.config
    }

    pub fn state(&self) -> &StreamState {
        &self.state
    }
}

/// Common test error type for integration tests
#[derive(Debug, Clone)]
pub enum TestError {
    StreamError(String),
    NetworkError(String),
    CaptureError(String),
}

impl std::fmt::Display for TestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestError::StreamError(msg) => write!(f, "Stream error: {}", msg),
            TestError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            TestError::CaptureError(msg) => write!(f, "Capture error: {}", msg),
        }
    }
}

impl std::error::Error for TestError {}

/// Setup common test fixtures
pub struct TestFixtures {
    pub test_doctor_report: Report,
    pub test_sink: Sink,
}

impl TestFixtures {
    pub fn new() -> Self {
        Self {
            test_doctor_report: Report {
                sway_result: CheckResult::ok("Sway OK"),
                pipewire_result: CheckResult::ok("PipeWire OK"),
                gstreamer_result: CheckResult::ok("GStreamer OK"),
                network_manager_result: CheckResult::ok("NM OK"),
                wpa_supplicant_result: CheckResult::ok("WPA OK"),
                xdg_desktop_portal_result: CheckResult::ok("XDG Portal OK"),
            },
            test_sink: Sink {
                name: "TestSink".to_string(),
                address: "AA:BB:CC:DD:EE:FF".to_string(),
                ip_address: Some("192.168.1.100".to_string()),
            },
        }
    }
}

// Helper function for checking integration test assertions
pub fn assert_doctor_passes(report: &Report) -> bool {
    report.all_ok()
}

pub fn assert_stream_config_valid(config: &StreamConfig) -> Result<(), String> {
    config.validate().map_err(|e| e.to_string())
}

pub fn assert_connection_successful(connection: &P2pConnection) -> Result<(), String> {
    if connection.get_sink().ip_address.is_some() {
        Ok(())
    } else {
        Err("Connection failed - no IP address assigned".to_string())
    }
}
