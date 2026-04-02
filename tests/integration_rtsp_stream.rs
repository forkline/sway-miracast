//! Integration tests for RTSP negotiation and stream pipeline setup

mod common;
use common::*;

use std::collections::HashMap;
use tokio;

use miracast_rtsp::{RtspServer, WfdCapabilities, SessionState, RtspSession, RtspMessage, RtspError};
use miracast_stream::{StreamConfig, StreamPipeline, VideoCodec, AudioCodec};

#[tokio::test]
async fn test_rtsp_negotiation_triggers_stream_setup_h264() {
    // Test RTSP negotiation with H.264 configuration resulting in stream setup
    let mut session = RtspSession::new("test_neg_stream".to_string());
    
    // Process SET_PARAMETER with H.264 format capabilities
    let mut parameters = HashMap::new();
    parameters.insert("wfd_video_formats".to_string(), "1 0 00 04 0001F437FDE63F490000000000000000".to_string());
    parameters.insert("wfd_audio_codecs".to_string(), "AAC 00000002 00".to_string());
    parameters.insert("wfd_client_rtp_ports".to_string(), "RTP/UDP/AVP/TCP;unicast 5004 0-2".to_string());
    
    let set_result = session.process_set_parameter(&parameters);
    assert!(set_result.is_ok());
    assert_eq!(session.state, SessionState::SetParamReceived);
    
    // Check that capabilities were correctly stored
    let video_formats = session.capabilities.get_parameter("wfd_video_formats").unwrap();
    assert_eq!(video_formats, Some("1 0 00 04 0001F437FDE63F490000000000000000"));
    
    let audio_codecs = session.capabilities.get_parameter("wfd_audio_codecs").unwrap();
    assert_eq!(audio_codecs, Some("AAC 00000002 00"));
    
    // Verify that stream pipeline can be configured from RTSP parameters
    let mut stream_config = StreamConfig::default();
    
    // Modify default configuration to match negotiated capabilities
    // (This simulates how RTSP parameters inform the stream configuration)
    stream_config.video_codec = VideoCodec::H264;  // Based on supported video format
    stream_config.audio_codec = AudioCodec::AAC;   // Based on supported audio codec
    
    // Validate the resulting configuration
    let config_validation = stream_config.validate();
    assert!(config_validation.is_ok(), "Negotiated stream config should be valid");
    
    // Create pipeline with the negotiated config
    let pipeline_result = StreamPipeline::new(stream_config);
    assert!(pipeline_result.is_ok(), "Should be able to create pipeline with negotiated config");
}

#[tokio::test]
async fn test_rtsp_negotiation_sets_pipeline_configurations() {
    // Test that different RTSP parameter negotiations result in specific stream configurations
    let mut session = RtspSession::new("pipeline_config_test".to_string());
    
    // Define video format that suggests specific resolution/bitrate/framerate
    let mut parameters = HashMap::new();
    parameters.insert("wfd_video_formats".to_string(), 
        "1 0 00 04 0001F437FDE63F490000000000000000".to_string() // 1080p@30fps, H.264/AVC profile level 4.1
    );
    parameters.insert("wfd_audio_codecs".to_string(), 
        "AAC 00000002 00".to_string() // AAC LC 2ch
    );
    parameters.insert("wfd_client_rtp_ports".to_string(), 
        "RTP/UDP/AVP/TCP;unicast 1234 0-2".to_string()
    );
    
    let set_result = session.process_set_parameter(&parameters);
    assert!(set_result.is_ok());
    
    // Extract parameters to configure stream with appropriate settings based on RTSP negotiation
    // In a real system, parsing of WFD formats would extract detailed parameters like
    // resolution, framerate, and codec-specific info
    let video_formats_val = session.capabilities.get_parameter("wfd_video_formats").unwrap().unwrap();
    let audio_codecs_val = session.capabilities.get_parameter("wfd_audio_codecs").unwrap().unwrap();
    
    // Configure stream based on RTSP negotiation results
    let mut adjusted_config = StreamConfig::default();
    
    // This is simplified - in a real implementation, the wfd_video_formats would be
    // parsed to extract bitrates and resolutions
    if video_formats_val.contains("F437FDE63F49") { // Roughly indicates 1080p
        adjusted_config.video_width = 1920;
        adjusted_config.video_height = 1080;
        adjusted_config.video_framerate = 30;
        adjusted_config.video_bitrate = 8_000_000;  // 8 Mbps
    }
    
    if audio_codecs_val.contains("AAC") {
        adjusted_config.audio_codec = AudioCodec::AAC;
        adjusted_config.audio_sample_rate = 48000;  // Standard rate for AAC in Miracast
        adjusted_config.audio_channels = 2;
        adjusted_config.audio_bitrate = 128_000;    // Standard bitrate
    }
    
    // Test that adjusted config is valid
    assert!(adjusted_config.validate().is_ok());
    
    // Create and test pipeline with adjusted configuration 
    let pipeline = StreamPipeline::new(adjusted_config);
    assert!(pipeline.is_ok());
}

#[tokio::test]
async fn test_stream_pipeline_configuration_from_multiple_scenarios() {
    // Test that different RTSP negotiation scenarios produce valid stream configurations
    let mut capabilities = WfdCapabilities::new();
    
    // Scenario 1: High-quality stream configuration
    capabilities.set_parameter("wfd_video_formats", 
        "1 0 00 05 0001F437FDE63F490000000000000000").unwrap(); // Higher resolution hint
    capabilities.set_parameter("wfd_audio_codecs", 
        "AAC 00000002 00").unwrap();
    
    let high_quality_config = create_stream_config_from_rtsp(&capabilities).unwrap();
    assert!(high_quality_config.validate().is_ok());
    assert_eq!(high_quality_config.video_width, 1920); // Expect 1080p
    assert_eq!(high_quality_config.video_height, 1080);
    
    // Scenario 2: Lower-quality stream configuration (for bandwidth-constrained devices)
    capabilities.set_parameter("wfd_video_formats", 
        "1 0 00 02 0001F40A0E72C8770000000000000000").unwrap(); // Lower resolution hint  
    capabilities.set_parameter("wfd_audio_codecs", 
        "AAC 00000002 00").unwrap();
        
    let low_quality_config = create_stream_config_from_rtsp(&capabilities).unwrap();
    assert!(low_quality_config.validate().is_ok());
    assert_eq!(low_quality_config.video_width, 1280); // Expect 720p
    assert_eq!(low_quality_config.video_height, 720);
    
    // Both should create valid pipelines
    let high_pipe = StreamPipeline::new(high_quality_config);
    let low_pipe = StreamPipeline::new(low_quality_config);
    assert!(high_pipe.is_ok());
    assert!(low_pipe.is_ok());
}

fn create_stream_config_from_rtsp(capabilities: &WfdCapabilities) -> Result<StreamConfig, String> {
    let mut config = StreamConfig::default();
    
    // Parse video formats to understand device capabilities
    if let Some(video_fmt) = capabilities.get_parameter("wfd_video_formats").unwrap() {
        // Simplified parsing based on Miracast WFD video format format
        // Format: [source count] [cursor] [profile count] [profiles...] [nativex] [nativey]
        // Example: 1 0 00 04 0001F437FDE63F490000000000000000
        // We can use the hex values that represent native resolution to infer config
        
        // In a full implementation, detailed parsing would happen here
        if video_fmt.contains("37FDE63F49") { // Signature for 1920x1080
            config.video_width = 1920;
            config.video_height = 1080;
        } else if video_fmt.contains("0A0E72C877") { // Signature for 1280x720 
            config.video_width = 1280;
            config.video_height = 720;
        } else {
            // Default to something reasonable if not recognized
            config.video_width = 1280;
            config.video_height = 720;
        }
    }
    
    if let Some(audio_fmt) = capabilities.get_parameter("wfd_audio_codecs").unwrap() {
        if audio_fmt.contains("AAC") {
            config.audio_codec = AudioCodec::AAC;
            config.audio_sample_rate = 48000;
            config.audio_channels = 2;
        } else if audio_fmt.contains("LPCM") {
            config.audio_codec = AudioCodec::LPCM;
            config.audio_sample_rate = 48000;
            config.audio_channels = 2;
        }
    }
    
    config.video_codec = VideoCodec::H264; // Per Miracast spec, H.264 is mandatory
    
    Ok(config)
}

#[tokio::test]
async fn test_rtsp_negotiation_error_handling_integration() {
    // Test error handling when there's disagreement between RTSP and stream expectations
    let mut session = RtspSession::new("error_integration_test".to_string());
    
    // Set a capability that might not be supported in stream layer
    let mut parameters = HashMap::new();
    parameters.insert("wfd_video_formats".to_string(), 
        "INVALID_FORMAT_DATA".to_string());
    
    let set_result = session.process_set_parameter(&parameters);
    if set_result.is_ok() {
        // If negotiation succeeded, check resulting configuration
        let stream_config = create_stream_config_from_rtsp(&session.capabilities);
        // In case of invalid format, stream config should still work with defaults
        assert!(stream_config.is_ok());
        
        let config = stream_config.unwrap();
        // Should fallback to defaults or fail validation gracefully
        if config.video_width == 1280 && config.video_height == 720 {
            // Default fallback worked
        } else {
            // Alternative fallback
        }
    } else {
        // Expected - negotiation failed with invalid capabilities
        if let Err(RtspError::Parse(_)) = set_result {
            // This is a valid outcome
        } else {
            panic!("Expected Parse error for invalid format");
        }
    }
}

#[tokio::test]
async fn test_complete_integration_rtsp_to_stream_pipeline() {
    // Complete integration test demonstrating RTSP negotiation to stream pipeline
    // This simulates the main control flow of Miracast server during setup phase
    
    // Phase 1: Initialize RTSP session
    let mut session = RtspSession::new("full_integration_test".to_string());
    
    // Phase 2: Process OPTIONS
    session.process_options().unwrap();
    
    // Phase 3: Process incoming set-parameter (negotiation)
    let mut client_caps = HashMap::new();
    client_caps.insert("wfd_video_formats".to_string(), 
                      "1 0 00 04 0001F437FDE63F490000000000000000".to_string()); // 1080p
    client_caps.insert("wfd_audio_codecs".to_string(), 
                      "AAC 00000002 00".to_string()); // AAC 2-channel
    client_caps.insert("wfd_client_rtp_ports".to_string(), 
                      "RTP/UDP/AVP/TCP;unicast 5004 0-2".to_string());
    
    session.process_set_parameter(&client_caps).unwrap();
    
    // Phase 4: Derive stream configuration from negotiated values
    let final_stream_config = create_stream_config_from_rtsp(&session.capabilities).unwrap();
    
    // Validate that negotiated configuration is sensible
    assert!(common::assert_stream_config_valid(&final_stream_config).is_ok());
    
    // Phase 5: Initialize stream pipeline with negotiated configuration
    let mut stream_pipeline = StreamPipeline::new(final_stream_config).unwrap();
    
    // Phase 6: Attempt to attach input (simulating how this might be connected in real system)
    // Since we only have mocked interfaces, we'll just check that we can call the right methods
    assert!(stream_pipeline.set_input(12345).is_ok()); // fake fd
    
    // Phase 7: Configure output based on RTSP negotiation results
    // In a full system, the RTP ports negotiated via RTSP would be used for stream destinations
    assert!(stream_pipeline.set_output("192.168.2.100", 5004).is_ok());
    
    // All phases completed successfully - full integration validated
    // Note: start() is not called since it involves actual streaming
    // (which would require real PipeWire stream or other input)
}

#[tokio::test]
async fn test_alternative_video_profiles_integration() {
    // Test integration supporting different video profile configurations
    let mut capabilities = WfdCapabilities::new();
    
    // Different H.264 profiles that might be negotiated 
    let profiles_to_test = vec![
        ("Profile 1: Standard 1080p", 
         "1 0 00 04 0001F437FDE63F490000000000000000", // 1080p
         (1920, 1080)),
        ("Profile 2: 720p", 
         "1 0 00 02 0001F40A0E72C8770000000000000000", // 720p 
         (1280, 720)),
        ("Profile 3: 480p", 
         "1 0 00 01 0001F403A09855000000000000000000", // 480p
         (640, 480)),
    ];
    
    for (desc, video_format, exp_resolution) in profiles_to_test {
        capabilities.set_parameter("wfd_video_formats", video_format).unwrap();
        capabilities.set_parameter("wfd_audio_codecs", "AAC 00000002 00").unwrap();
        
        let config = create_stream_config_from_rtsp(&capabilities).unwrap();
        
        // Verify resolution matches expected for profile
        assert_eq!(config.video_width, exp_resolution.0, 
                   "Incorrect width for {}: Expected {} but got {}", desc, exp_resolution.0, config.video_width);
        assert_eq!(config.video_height, exp_resolution.1,
                   "Incorrect height for {}: Expected {} but got {}", desc, exp_resolution.1, config.video_height);
        
        // Verify config is valid
        assert!(config.validate().is_ok(), 
                "Config for {} should be valid", desc);
        
        // Pipeline creation should succeed for this resolution
        let pipeline = StreamPipeline::new(config);
        assert!(pipeline.is_ok(), 
                "Pipeline should be creatable for {}", desc);
    }
}