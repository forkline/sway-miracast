//! Integration tests for the full daemon workflow with mocked components

mod common;
use common::*;

use std::collections::HashMap;
use std::time::Duration;
use tokio;

use miracast_doctor::{check_sway_with_runner, check_pipewire_with_runner, 
                     check_gstreamer_with_runner, check_network_manager_with_runner, 
                     check_wpa_supplicant_with_runner, CheckResult};
use miracast_net::{P2pManager, P2pConfig, Sink};
use miracast_rtsp::{RtspServer, WfdCapabilities, SessionState, RtspSession, RtspMessage};

#[tokio::test]
async fn test_full_session_lifecycle_discover_connect() {
    // Test the discovery and initial connection phases
    let test_fixtures = TestFixtures::new();
    let mut net_runner = NetMockCommandRunner::new();
    
    // Mock network commands for discovery
    net_runner.add_response("nmcli", &["device", "status"], Ok(miracast_net::CommandOutput {
        stdout: b"wlan0 p2p-dev-wlan0 connected\n".to_vec(),
        stderr: vec![],
        status: true,
    }));
    
    net_runner.add_response("nmcli", &["device", "wifi", "rescan"], Ok(miracast_net::CommandOutput {
        stdout: vec![],
        stderr: vec![],
        status: true,
    }));
    
    net_runner.add_response("nmcli", &["device", "wifi", "list", "--fields", "NAME,DEVICE,MAC,BARS"], Ok(
        miracast_net::CommandOutput {
            stdout: b"NAME                     DEVICE           MAC               BARS\nTest Miracast Device    p2p-test         AA:BB:CC:DD:EE:FF  ****\nAnother Device          p2p-another      11:22:33:44:55:66  ***".to_vec(),
            stderr: vec![],
            status: true,
        }
    ));
    
    // Set up connection mocks
    net_runner.add_response("nmcli", &["device", "wifi", "connect", "Test Miracast Device"], Ok(
        miracast_net::CommandOutput {
            stdout: b"Connection activated\n".to_vec(),
            stderr: vec![],
            status: true,
        }
    ));
    
    net_runner.add_response("nmcli", &["device", "show", "wlan0"], Ok(
        miracast_net::CommandOutput {
            stdout: b"GENERAL.DEVICE:                         p2p-wlan0-0\nGENERAL.TYPE:                           wifi-p2p\nGENERAL.STATE:                          connected\nIP4.ADDRESS[1]:                         192.168.1.100/24\n".to_vec(),
            stderr: vec![],
            status: true,
        }
    ));
    
    let config = P2pConfig {
        interface_name: "wlan0".to_string(),
        group_name: "test_group".to_string(),
    };
    
    let manager = P2pManager::new_with_command_runner(config, net_runner).unwrap();
    
    // Discover phase
    let discovered_sinks = manager.discover_sinks(Duration::from_secs(10)).unwrap();
    assert!(!discovered_sinks.is_empty());
    
    // Pick the first sink
    let sink = &discovered_sinks[0];
    assert_eq!(sink.name, "Test Miracast Device");
    
    // Connect phase
    let connection = manager.connect(sink).unwrap();
    assert_eq!(connection.get_sink().name, "Test Miracast Device");
    assert_eq!(connection.get_sink().ip_address.as_ref().unwrap(), "192.168.1.100");
}

#[tokio::test]
async fn test_full_session_lifecycle_negotiate_stream() {
    // Test the RTSP negotiation and stream setup phases
    let mut rtsp_caps = WfdCapabilities::new();
    
    // Test that WFD capabilities can be set according to negotiation
    rtsp_caps.set_parameter("wfd_video_formats", "1 0 00 04 0001F437FDE63F490000000000000000").unwrap();
    rtsp_caps.set_parameter("wfd_audio_codecs", "AAC 00000002 00").unwrap();
    rtsp_caps.set_parameter("wfd_client_rtp_ports", "RTP/UDP/AVP/TCP;unicast 12345 0-255").unwrap();
    
    let caps_video = rtsp_caps.get_parameter("wfd_video_formats").unwrap().unwrap();
    let caps_audio = rtsp_caps.get_parameter("wfd_audio_codecs").unwrap().unwrap();
    
    assert_eq!(caps_video, "1 0 00 04 0001F437FDE63F490000000000000000");
    assert_eq!(caps_audio, "AAC 00000002 00");
    
    // Initialize a session to test negotiation flow
    let session_id = "test_session_123".to_string();
    let mut session = RtspSession::new(session_id);
    
    // Simulate the RTSP negotiation phases
    // OPTIONS phase
    let opts_resp = session.process_options().unwrap();
    assert_eq!(session.state, SessionState::OptionsReceived);
    assert!(opts_resp.contains("Public:"));
    
    // SET_PARAMETER phase
    let mut parameters = HashMap::new();
    parameters.insert("wfd_video_formats".to_string(), "1 0 00 04 0001F437FDE63F490000000000000000".to_string());
    parameters.insert("wfd_audio_codecs".to_string(), "AAC 00000002 00".to_string());
    
    let set_resp = session.process_set_parameter(&parameters).unwrap();
    assert_eq!(session.state, SessionState::SetParamReceived);
    assert!(set_resp.contains("200 OK"));
    
    // GET_PARAMETER phase
    let get_resp = session.process_get_parameter(&["wfd_video_formats", "wfd_audio_codecs"]).unwrap();
    assert_eq!(session.state, SessionState::GetParamReceived);
    assert!(get_resp.contains("wfd_video_formats:"));
    assert!(get_resp.contains("wfd_audio_codecs:"));
    
    // PLAY phase
    let play_resp = session.process_play().unwrap();
    assert_eq!(session.state, SessionState::Play);
    assert!(play_resp.contains("RTP-Info:"));
}

#[tokio::test]
async fn test_full_session_lifecycle_stop() {
    // Test the teardown phase of the session
    let session_id = "test_session_teardown".to_string();
    let mut session = RtspSession::new(session_id);
    
    // Go through some phases first to ensure proper teardown
    session.process_options().unwrap();
    session.process_play().unwrap();
    assert_eq!(session.state, SessionState::Play);
    
    // TEARDOWN phase
    let teardown_resp = session.process_teardown().unwrap();
    assert_eq!(session.state, SessionState::Teardown);
    assert!(teardown_resp.contains("200 OK"));
}

#[tokio::test]
async fn test_error_handling_at_each_step_discovery() {
    // Test error handling during discovery step
    let mut net_runner = NetMockCommandRunner::new();
    
    // Mock to make discovery process fail
    net_runner.add_response("nmcli", &["device", "status"], Ok(miracast_net::CommandOutput {
        stdout: b"wlan0 p2p-dev-wlan0 connected\n".to_vec(),
        stderr: vec![],
        status: true,
    }));
    
    net_runner.add_response("nmcli", &["device", "wifi", "rescan"], Ok(miracast_net::CommandOutput {
        stdout: vec![],
        stderr: vec![],
        status: false, // Make rescan fail
    }));
    
    let config = P2pConfig {
        interface_name: "wlan0".to_string(),
        group_name: "test_group".to_string(),
    };
    
    let manager = P2pManager::new_with_command_runner(config, net_runner).unwrap();
    
    // Should fail during discovery due to rescan failure
    let discovery_result = manager.discover_sinks(Duration::from_secs(5));
    assert!(discovery_result.is_err());
    
    match discovery_result.unwrap_err() {
        miracast_net::NetError::CommandFailed(msg) => {
            assert!(msg.contains("nmcli rescan failed"));
        }
        _ => panic!("Expected CommandFailed error"),
    }
}

#[tokio::test]
async fn test_error_handling_at_each_step_connect() {
    // Test error handling during connection step
    let mut net_runner = NetMockCommandRunner::new();
    
    // Mock for connection failure scenario
    net_runner.add_response("nmcli", &["device", "status"], Ok(miracast_net::CommandOutput {
        stdout: b"wlan0 p2p-dev-wlan0 connected\n".to_vec(),
        stderr: vec![],
        status: true,
    }));
    
    net_runner.add_response("nmcli", &["device", "wifi", "rescan"], Ok(miracast_net::CommandOutput {
        stdout: vec![],
        stderr: vec![],
        status: true,
    }));
    
    net_runner.add_response("nmcli", &["device", "wifi", "list", "--fields", "NAME,DEVICE,MAC,BARS"], Ok(
        miracast_net::CommandOutput {
            stdout: b"NAME                     DEVICE           MAC               BARS\nBad Device             p2p-baddev       CC:DD:EE:FF:GG:HH  *".to_vec(),
            stderr: vec![],
            status: true,
        }
    ));
    
    // Cause connection to fail
    net_runner.add_response("nmcli", &["device", "wifi", "connect", "Bad Device"], Ok(
        miracast_net::CommandOutput {
            stdout: vec![],
            stderr: b"Error: Connection failed\n".to_vec(),
            status: false, // Connection failed
        }
    ));
    
    let config = P2pConfig {
        interface_name: "wlan0".to_string(),
        group_name: "test_group".to_string(),
    };
    
    let manager = P2pManager::new_with_command_runner(config, net_runner).unwrap();
    
    let discovered_sinks = manager.discover_sinks(Duration::from_secs(10)).unwrap();
    assert!(!discovered_sinks.is_empty());
    
    let sink = &discovered_sinks[0];
    assert_eq!(sink.name, "Bad Device");
    
    // Attempting to connect should fail
    let connect_result = manager.connect(sink);
    assert!(connect_result.is_err());
    
    match connect_result.unwrap_err() {
        miracast_net::NetError::CommandFailed(msg) => {
            assert!(msg.contains("Connecting to Bad Device failed"));
        }
        _ => panic!("Expected CommandFailed error"),
    }
}

#[tokio::test]
async fn test_error_handling_in_rtsp_negotiation() {
    // Test error handling in RTSP negotiation steps
    let session_id = "err_test_session".to_string();
    let mut session = RtspSession::new(session_id);
    
    // Test malformed parameter setting
    let mut parameters = HashMap::new();
    parameters.insert("invalid_wfd_param".to_string(), "some_value".to_string());
    
    let set_result = session.process_set_parameter(&parameters);
    assert!(set_result.is_err());
    
    match set_result.unwrap_err() {
        miracast_rtsp::RtspError::InvalidParameter(param) => {
            assert_eq!(param, "invalid_wfd_param");
        }
        _ => panic!("Expected InvalidParameter error"),
    }
    
    // Test getting non-existent parameter
    let get_result = session.process_get_parameter(&["wfd_nonexistent_param"]);
    assert!(get_result.is_err());
    
    match get_result.unwrap_err() {
        miracast_rtsp::RtspError::InvalidParameter(param) => {
            assert_eq!(param, "wfd_nonexistent_param");
        }
        _ => panic!("Expected InvalidParameter error"),
    }
}

#[tokio::test]
async fn test_end_to_end_workflow_integration() {
    // End-to-end test combining all steps with mocked components to simulate full flow
    // without requiring real hardware/components
    
    // Step 1: Doctor should pass for environment
    // Skip actual check since tests are done in isolation, but we assume environment is properly set up
    
    // Step 2: Discovery
    let mut net_runner = NetMockCommandRunner::new();
    
    net_runner.add_response("nmcli", &["device", "status"], Ok(miracast_net::CommandOutput {
        stdout: b"wlan0 p2p-dev-wlan0 connected\n".to_vec(),
        stderr: vec![],
        status: true,
    }));
    
    net_runner.add_response("nmcli", &["device", "wifi", "rescan"], Ok(miracast_net::CommandOutput {
        stdout: vec![],
        stderr: vec![],
        status: true,
    }));
    
    net_runner.add_response("nmcli", &["device", "wifi", "list", "--fields", "NAME,DEVICE,MAC,BARS"], Ok(
        miracast_net::CommandOutput {
            stdout: b"NAME              DEVICE    MAC               BARS\nTargetDevice      p2pdev    AA:BB:CC:DD:EE:FF  ****".to_vec(),
            stderr: vec![],
            status: true,
        }
    ));
    
    net_runner.add_response("nmcli", &["device", "wifi", "connect", "TargetDevice"], Ok(
        miracast_net::CommandOutput {
            stdout: b"Device 'p2p-target-123' successfully activated\n".to_vec(),
            stderr: vec![],
            status: true,
        }
    ));
    
    net_runner.add_response("nmcli", &["device", "show", "wlan0"], Ok(
        miracast_net::CommandOutput {
            stdout: b"GENERAL.DEVICE:                         p2p-wlan0\nIP4.ADDRESS[1]:                         192.168.2.100/24\n".to_vec(),
            stderr: vec![],
            status: true,
        }
    ));
    
    let config = P2pConfig {
        interface_name: "wlan0".to_string(),
        group_name: "end_to_end_group".to_string(),
    };
    
    let manager = P2pManager::new_with_command_runner(config, net_runner).unwrap();
    
    // Discover
    let sinks = manager.discover_sinks(Duration::from_secs(10)).unwrap();
    assert_eq!(sinks.len(), 1);
    let target_sink = &sinks[0];
    
    // Connect
    let connection = manager.connect(target_sink).unwrap();
    assert_eq!(connection.get_sink().ip_address.as_ref().unwrap(), "192.168.2.100");
    
    // Step 3: Negotiation (RTSP setup)
    let mut rtsp_session = RtspSession::new("end_to_end_session".to_string());
    
    // Options
    assert!(rtsp_session.process_options().is_ok());
    assert_eq!(rtsp_session.state, SessionState::OptionsReceived);
    
    // Set parameters (negotiate capabilities)
    let mut cap_params = HashMap::new();
    cap_params.insert("wfd_video_formats".to_string(), "1 0 00 04 0001F437FDE63F490000000000000000".to_string());
    cap_params.insert("wfd_audio_codecs".to_string(), "AAC 00000002 00".to_string());
    cap_params.insert("wfd_client_rtp_ports".to_string(), "RTP/UDP/AVP/TCP;unicast 12345 0-255".to_string());
    
    assert!(rtsp_session.process_set_parameter(&cap_params).is_ok());
    assert_eq!(rtsp_session.state, SessionState::SetParamReceived);
    
    // Get parameters (confirm negotiation)
    let get_result = rtsp_session.process_get_parameter(&["wfd_video_formats", "wfd_audio_codecs"]);
    assert!(get_result.is_ok());
    assert_eq!(rtsp_session.state, SessionState::GetParamReceived);
    
    // Step 4: Play/Streaming
    assert!(rtsp_session.process_play().is_ok());
    assert_eq!(rtsp_session.state, SessionState::Play);
    
    // Step 5: Stop/Teardown
    assert!(rtsp_session.process_teardown().is_ok());
    assert_eq!(rtsp_session.state, SessionState::Teardown);
    
    // Verify that we went through the complete flow without errors in our integrated workflow
}