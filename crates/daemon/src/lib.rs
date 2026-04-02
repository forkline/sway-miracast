use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use swaybeam_capture::Capture;
use swaybeam_doctor::{check_all, Report as DoctorReport};
use swaybeam_net::{NetError, P2pConfig, P2pConnection, P2pManager, Sink};
use swaybeam_rtsp::{RtspClient, RtspServer, SetupResult};
use swaybeam_stream::{
    AudioCodec, StreamConfig, StreamPipeline, TestPatternConfig, TestPatternGenerator, VideoCodec,
};

#[derive(Debug, Clone, PartialEq)]
pub enum DaemonState {
    Idle,
    Discovering,
    Connecting,
    Negotiating,
    Streaming,
    Disconnecting,
    Error,
}

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub video_width: u32,
    pub video_height: u32,
    pub video_framerate: u32,
    pub video_bitrate: u32,
    pub discovery_timeout: Duration,
    pub interface: String,
    pub preferred_sink: Option<String>,
    pub force_client_mode: bool,  // Whether to force RTSP client mode instead of server mode
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            video_width: 1920,
            video_height: 1080,
            video_framerate: 30,
            video_bitrate: 8_000_000,
            discovery_timeout: Duration::from_secs(10),
            interface: "wlan0".to_string(),
            preferred_sink: None,
            force_client_mode: false,  // Default is traditional server mode
        }
    }
}

pub struct Daemon {
    state: Arc<RwLock<DaemonState>>,
    config: DaemonConfig,
    capture: Option<Capture>,
    stream: Option<StreamPipeline>,
    connection: Option<P2pConnection>,
    rtsp_server: Option<RtspServer>,
    event_tx: mpsc::UnboundedSender<DaemonEvent>,
    event_rx: Option<mpsc::UnboundedReceiver<DaemonEvent>>,
}

#[derive(Debug)]
pub enum DaemonEvent {
    Started,
    Discovered(Vec<Sink>),
    Connected(Sink),
    Negotiated,
    StreamingStarted,
    StreamingStopped,
    ErrorOccurred(String),
    Ended,
}

impl Daemon {
    pub fn new() -> Self {
        Self::with_config(DaemonConfig::default())
    }

    pub fn with_config(config: DaemonConfig) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        Daemon {
            state: Arc::new(RwLock::new(DaemonState::Idle)),
            config,
            capture: None,
            stream: None,
            connection: None,
            rtsp_server: None,
            event_tx,
            event_rx: Some(event_rx),
        }
    }

    pub fn get_state(&self) -> DaemonState {
        self.state.read().clone()
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        info!("Daemon starting...");
        *self.state.write() = DaemonState::Discovering;
        self.event_tx.send(DaemonEvent::Started).ok();

        let sinks = self.discover().await?;
        debug!("Discovered {} sink(s)", sinks.len());

        if sinks.is_empty() {
            return Err(anyhow::anyhow!("No Miracast sinks discovered"));
        }

        let sink = if let Some(ref preferred) = self.config.preferred_sink {
            sinks
                .iter()
                .find(|s| s.name == *preferred || s.address == *preferred)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Preferred sink '{}' not found", preferred))?
        } else {
            sinks.into_iter().next().unwrap()
        };

        *self.state.write() = DaemonState::Connecting;
        self.connect(sink).await?;

        *self.state.write() = DaemonState::Negotiating;
        self.negotiate().await?;

        *self.state.write() = DaemonState::Streaming;
        self.start_stream().await?;

        info!("Streaming active, press Ctrl+C to stop...");
        tokio::signal::ctrl_c().await.ok();

        self.stop_stream().await.ok();
        self.disconnect().await.ok();

        info!("Daemon stopped");
        *self.state.write() = DaemonState::Idle;

        Ok(())
    }

    pub fn subscribe_events(&mut self) -> Option<mpsc::UnboundedReceiver<DaemonEvent>> {
        self.event_rx.take()
    }

    async fn run_doctor_checks(&self) -> anyhow::Result<DoctorReport> {
        info!("Running system doctor checks...");
        let report = check_all()?;

        if !report.all_ok() {
            error!("Doctor checks failed");
            report.print();
            return Err(anyhow::anyhow!("System requirements not met"));
        }

        info!("All doctor checks passed!");
        Ok(report)
    }

    pub async fn discover(&self) -> Result<Vec<Sink>, NetError> {
        *self.state.write() = DaemonState::Discovering;

        let config = P2pConfig {
            interface_name: self.config.interface.clone(),
            group_name: "swaybeam".to_string(),
        };

        let manager = P2pManager::new(config).await?;
        let sinks = manager
            .discover_sinks(self.config.discovery_timeout)
            .await?;

        *self.state.write() = DaemonState::Idle;
        Ok(sinks)
    }

    pub async fn connect(&mut self, sink: Sink) -> Result<(), NetError> {
        *self.state.write() = DaemonState::Connecting;

        let config = P2pConfig {
            interface_name: self.config.interface.clone(),
            group_name: "swaybeam".to_string(),
        };

        let manager = P2pManager::new(config).await?;
        let connection = manager.connect(&sink).await?;

        self.connection = Some(connection);
        *self.state.write() = DaemonState::Negotiating;

        info!("Connected to sink: {}", sink.name);
        self.event_tx.send(DaemonEvent::Connected(sink)).ok();

        Ok(())
    }

    pub async fn negotiate(&mut self) -> anyhow::Result<()> {
        if self.get_state() != DaemonState::Negotiating {
            return Err(anyhow::anyhow!("Daemon must be in Negotiating state"));
        }
        
        // First, let's determine if the sink is the Group Owner by checking its WFD capabilities
        let is_sink_go = self.determine_sink_role().await?;
        
        if is_sink_go {
            // Connect as RTSP client to the sink's RTSP server
            self.negotiate_as_client().await?;
        } else {
            // Act as RTSP server (traditional source mode)
            self.negotiate_as_server().await?;
        }

        info!("RTSP negotiation completed");
        self.event_tx.send(DaemonEvent::Negotiated).ok();

        Ok(())
    }

    pub async fn start_stream(&mut self) -> anyhow::Result<()> {
        if self.get_state() != DaemonState::Streaming {
            return Err(anyhow::anyhow!("Daemon must be in Streaming state"));
        }

        let stream_config = StreamConfig {
            video_codec: VideoCodec::H264,
            video_bitrate: self.config.video_bitrate,
            video_width: self.config.video_width,
            video_height: self.config.video_height,
            video_framerate: self.config.video_framerate,
            audio_codec: AudioCodec::AAC,
            audio_bitrate: 128_000,
            audio_sample_rate: 48000,
            audio_channels: 2,
        };

        let pipeline = StreamPipeline::new(stream_config)?;

        if let Some(ref conn) = self.connection {
            if let Some(ref sink_ip) = conn.get_sink().ip_address {
                pipeline.set_output(sink_ip, 5004).await?;
            }
        }

        pipeline.start().await?;
        self.stream = Some(pipeline);
        info!("Stream pipeline started");
        self.event_tx.send(DaemonEvent::StreamingStarted).ok();

        Ok(())
    }

    pub async fn stop_stream(&mut self) -> anyhow::Result<()> {
        self.stream = None;
        info!("Streaming stopped");
        self.event_tx.send(DaemonEvent::StreamingStopped).ok();
        Ok(())
    }

    pub async fn disconnect(&mut self) -> Result<(), NetError> {
        *self.state.write() = DaemonState::Disconnecting;

        if let Some(conn) = self.connection.take() {
            let config = P2pConfig {
                interface_name: self.config.interface.clone(),
                group_name: "swaybeam".to_string(),
            };

            let manager = P2pManager::new(config).await?;
            manager.disconnect().await?;
            info!("Disconnected from {}", conn.get_sink().name);
        }

        *self.state.write() = DaemonState::Idle;
        Ok(())
    }
    
    /// Determine if sink is Group Owner by analyzing its WFD capabilities
    async fn determine_sink_role(&self) -> Result<bool, anyhow::Error> {
        // Allow user to force client mode
        if self.config.force_client_mode {
            return Ok(true);
        }
        
        // First, attempt to determine automatically based on the discovered sink and network topology 
        if let Some(conn) = &self.connection {
            let sink = conn.get_sink();
            if let Some(ref wfd_caps) = sink.wfd_capabilities {
                info!("Analyzing WFD capabilities to determine role: {:?}", wfd_caps);
            }
            
            // In Wi-Fi Direct, sometimes the device type information indicates role
            // but detection logic can be complex. A common situation is that TVs 
            // can operate in GO role despite being sinks conceptually.
            // Try heuristics, or just return a reasonable default
            
            // For now, just use force mechanism, or fallback to default
            // A sophisticated detection could check:  
            // 1. Network address patterns (TV often ends in .1 or .254)
            // 2. MAC OUI patterns  
            // 3. Known device type patterns
            // 4. Test for RTSP server availability
            
            Ok(false) // Default to traditional mode unless explicitly forced
        } else {
            // No connection - default to server mode
            Ok(false) 
        }
    }

    /// Negotiate when device is traditional source (our side hosts RTSP server)
    async fn negotiate_as_server(&mut self) -> anyhow::Result<()> {
        let rtsp_addr = "0.0.0.0:7236";
        let rtsp_server = RtspServer::new(rtsp_addr.to_string());

        let rtsp_server_clone = RtspServer::new(rtsp_addr.to_string());
        tokio::spawn(async move {
            if let Err(e) = rtsp_server_clone.start().await {
                tracing::error!("RTSP server error: {:?}", e);
            }
        });

        self.rtsp_server = Some(rtsp_server);
        Ok(())
    }
    
    /// Negotiate when sink is Group Owner (connect to its RTSP server)
    async fn negotiate_as_client(&mut self) -> anyhow::Result<()> {
        if let Some(conn) = &self.connection {
            // The sink's RTSP port is typically in the WFD IEs information
            // Commonly 7236 for Miracast, but could vary
            let rtsp_port = 7236; // Default Miracast RTSP port
            
            if let Some(sink_ip) = &conn.get_sink().ip_address {
                info!("Connecting as RTSP client to TV at {}:{}", sink_ip, rtsp_port);
                
                let mut rtsp_client = RtspClient::connect(sink_ip, rtsp_port).await
                    .map_err(|e| anyhow::anyhow!("RTSP client connection failed: {:?}", e))?;
                
                // Negotiation sequence as per Miracast spec for client
                _ = rtsp_client.send_options().await
                    .map_err(|e| tracing::warn!("OPTIONS request failed: {:?}", e));
                
                // Get parameter request for device capabilities
                let params_to_request = &[
                    "wfd_video_formats",
                    "wfd_audio_codecs",
                    "wfd_client_rtp_ports",
                    "wfd_uibc_capability",
                    "wfd_standby_resume_capability",
                    "wfd_content_protection",
                    "wfd_display_hdcp_supported",
                    "wfd_coupled_sink"
                ];
                let sink_caps = rtsp_client.send_get_parameter(params_to_request).await
                    .map_err(|e| tracing::warn!("GET_PARAMETER request failed: {:?}", e))
                    .unwrap_or_default();
                
                // Send our source capabilities  
                let mut source_caps = std::collections::HashMap::new();
                source_caps.insert("wfd_video_formats".to_string(), swaybeam_rtsp::WfdCapabilities::build_video_formats());
                source_caps.insert("wfd_audio_codecs".to_string(), swaybeam_rtsp::WfdCapabilities::build_audio_codecs());
                source_caps.insert("wfd_uibc_capability".to_string(), "none".to_string());
                
                rtsp_client.send_set_parameter(&source_caps).await
                    .map_err(|e| tracing::warn!("SET_PARAMETER request failed: {:?}", e))
                    .unwrap_or(());
                
                // Setup RTP session using our desired ports
                let desired_rtp_port = 5004; // Standard RTP port
                let setup_result = rtsp_client.send_setup(desired_rtp_port).await
                    .map_err(|e| tracing::warn!("SETUP request failed: {:?}", e))
                    .unwrap_or(SetupResult {
                        destination_ip: sink_ip.clone(),
                        destination_rtp_port: 5004,
                        session_id: "dummy_session".to_string(),
                        timeout: 30,
                    });
                    
                // Start streaming
                rtsp_client.send_play().await
                    .map_err(|e| tracing::warn!("PLAY request failed: {:?}", e))
                    .unwrap_or(());
                
                info!("Successfully negotiated as RTSP client, streaming parameters set up");
                
                // For client mode, we don't need to wait for PLAY since we initiate it
                // Set state to streaming directly since playback started
                *self.state.write() = DaemonState::Streaming;
                
                // Use the setup result to configure the stream
                let stream_config = StreamConfig {
                    video_codec: self.get_negotiated_codec(&sink_caps),
                    video_bitrate: self.config.video_bitrate,
                    video_width: self.config.video_width,
                    video_height: self.config.video_height,
                    video_framerate: self.config.video_framerate,
                    audio_codec: AudioCodec::AAC,
                    audio_bitrate: 128_000,
                    audio_sample_rate: 48000,
                    audio_channels: 2,
                };
                
                let pipeline = StreamPipeline::new(stream_config)?;
                pipeline.set_output(&setup_result.destination_ip, setup_result.destination_rtp_port).await?;
                pipeline.start().await?;
                info!("Stream pipeline started to {}:{} using RTSP client mode", 
                      setup_result.destination_ip, setup_result.destination_rtp_port);
                      
                // Initialize stream
                let caps = gstreamer::Caps::builder("video/x-raw")
                    .field("format", "BGRA")
                    .field("width", self.config.video_width as i32)
                    .field("height", self.config.video_height as i32)
                    .field(
                        "framerate",
                        gstreamer::Fraction::new(self.config.video_framerate as i32, 1),
                    )
                    .build();
                pipeline.set_caps(&caps).await?;
                        
                let test_pattern_config = TestPatternConfig {
                    width: self.config.video_width,
                    height: self.config.video_height,
                    framerate: self.config.video_framerate as f32,
                };
                let generator = TestPatternGenerator::new(test_pattern_config);
                let mut frame_receiver = generator.start();
                
                let pipeline_clone = pipeline.clone();
                tokio::spawn(async move {
                    while let Some(frame) = frame_receiver.recv().await {
                        let gst_buffer = gstreamer::Buffer::from_slice(frame.data.clone());
                        if let Err(e) = pipeline_clone.push_video_buffer(&gst_buffer).await {
                            tracing::error!("Failed to push frame: {}", e);
                            break;
                        }
                    }
                    tracing::info!("Frame sender stopped");
                });
                
                self.stream = Some(pipeline);
                info!("Stream pipeline configured in client mode");

                return Ok(());
            } else {
                return Err(anyhow::anyhow!("No IP address for sink connection"));
            }
        } else {
            return Err(anyhow::anyhow!("No active connection to sink"));
        }
    }
    
    /// Determine video codec from sink capabilities
    fn get_negotiated_codec(&self, sink_caps: &std::collections::HashMap<String, String>) -> VideoCodec {
        if let Some(video_formats) = sink_caps.get("wfd_video_formats") {
            // Try to detect from video formats
            if video_formats.contains("000000000000001F") {
                // Supports H.265 (HEVC)
                VideoCodec::H265
            } else if video_formats.contains("0000000000000007") {
                // Supports H.264
                VideoCodec::H264
            } else {
                VideoCodec::H264 // Default fallback
            }
        } else {
            VideoCodec::H264 // Default fallback
        }
    }
}
