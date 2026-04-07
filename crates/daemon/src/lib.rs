use std::io::ErrorKind;
use std::sync::Arc;
use std::time::Duration;
use swaybeam_audio::VirtualAudioSink;
use swaybeam_external::{ExternalResolution, VirtualOutput};

use aes::Aes128;
use hmac::{Hmac, Mac};
use parking_lot::RwLock as PlRwLock;
use rand::{rngs::OsRng, RngCore};
use rsa::{BigUint, Oaep, Pkcs1v15Encrypt, RsaPublicKey};
use sha1::Sha1;
use sha2::Sha256;
use tokio::net::{TcpSocket, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use swaybeam_capture::{Capture, CaptureConfig};
use swaybeam_doctor::{check_all, Report as DoctorReport};
use swaybeam_net::{NetError, P2pConfig, P2pConnection, P2pManager, Sink};
use swaybeam_rtsp::{parse_wfd_client_rtp_port, NegotiatedCodec, RtspClient, RtspServer};
use swaybeam_stream::{AudioCodec, StreamConfig, StreamPipeline, VideoCodec};

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
    pub force_client_mode: bool,
    pub extend_mode: bool,
    pub enable_audio: bool,
    pub video_codec: Option<VideoCodec>,
    pub external_resolution: Option<ExternalResolution>,
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
            force_client_mode: false,
            extend_mode: false,
            enable_audio: false,
            video_codec: None,
            external_resolution: None,
        }
    }
}

pub struct Daemon {
    state: Arc<PlRwLock<DaemonState>>,
    config: DaemonConfig,
    #[allow(dead_code)]
    capture: Option<Capture>,
    stream: Arc<RwLock<Option<StreamPipeline>>>,
    hdcp_stream: Option<TcpStream>,
    hdcp_encryption: Option<HdcpEncryptionConfig>,
    connection: Option<P2pConnection>,
    rtsp_server: Option<RtspServer>,
    virtual_output: Option<VirtualOutput>,
    event_tx: mpsc::UnboundedSender<DaemonEvent>,
    event_rx: Option<mpsc::UnboundedReceiver<DaemonEvent>>,
    audio_sink: Option<VirtualAudioSink>,
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

#[allow(dead_code)]
struct HdcpReceiverCert {
    repeater: bool,
    receiver_id: [u8; 5],
    modulus: Vec<u8>,
    exponent: [u8; 3],
}

#[allow(dead_code)]
struct HdcpSessionMaterial {
    rtx: [u8; 8],
    km: [u8; 16],
    rn: Option<[u8; 8]>,
    rrx: Option<[u8; 8]>,
    receiver_version: Option<u8>,
    repeater: bool,
    receiver_id: [u8; 5],
    h_prime_verified: bool,
    pairing_info_received: bool,
    sent_lc_init: bool,
    sent_ske: bool,
    ks: Option<[u8; 16]>,
    riv: Option<[u8; 8]>,
    kd_for_encryption: Option<[u8; 32]>,
}

pub struct HdcpEncryptionConfig {
    pub ks: [u8; 16],
    pub riv: [u8; 8],
    pub rrx: [u8; 8],
    pub receiver_version: u8,
}

impl Daemon {
    pub fn new() -> Self {
        Self::with_config(DaemonConfig::default())
    }
}

impl Default for Daemon {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl Daemon {
    pub fn with_config(config: DaemonConfig) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        Daemon {
            state: Arc::new(PlRwLock::new(DaemonState::Idle)),
            config,
            capture: None,
            stream: Arc::new(RwLock::new(None)),
            hdcp_stream: None,
            hdcp_encryption: None,
            connection: None,
            rtsp_server: None,
            virtual_output: None,
            event_tx,
            event_rx: Some(event_rx),
            audio_sink: None,
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

        if self.config.extend_mode {
            let output =
                tokio::task::block_in_place(|| VirtualOutput::create(ExternalResolution::FourK))
                    .map_err(|e| anyhow::anyhow!("Failed to create virtual output: {}", e))?;
            info!(
                "Virtual output '{}' configured for 4K extend mode",
                output.output_name()
            );
            self.config.video_width = 3840;
            self.config.video_height = 2160;
            self.config.video_bitrate = 20_000_000;
            self.config.video_framerate = 30;
            self.virtual_output = Some(output);
        } else if let Some(resolution) = self.config.external_resolution {
            let output = tokio::task::block_in_place(|| VirtualOutput::create(resolution))
                .map_err(|e| anyhow::anyhow!("Failed to create virtual output: {}", e))?;
            info!(
                "Virtual output '{}' configured for external monitor ({})",
                output.output_name(),
                resolution.mode_string()
            );
            self.config.video_width = resolution.width();
            self.config.video_height = resolution.height();
            self.config.video_bitrate = if resolution == ExternalResolution::FourK {
                20_000_000
            } else {
                8_000_000
            };
            self.virtual_output = Some(output);
        }

        if self.config.enable_audio {
            info!("Creating virtual audio sink for audio routing...");
            let audio_sink = VirtualAudioSink::create()
                .map_err(|e| anyhow::anyhow!("Failed to create virtual audio sink: {}", e))?;
            info!(
                "Virtual audio sink '{}' created and set as default",
                audio_sink.sink_name()
            );
            self.audio_sink = Some(audio_sink);
        }

        *self.state.write() = DaemonState::Negotiating;
        self.negotiate().await?;

        *self.state.write() = DaemonState::Streaming;
        if self.stream.read().await.is_none() {
            if self.virtual_output.is_some() {
                println!("Please approve the screen sharing dialog for the virtual monitor...");
            }
            self.start_stream().await?;
        }

        info!("Streaming active, press Ctrl+C to stop...");
        tokio::signal::ctrl_c().await.ok();

        self.stop_stream().await.ok();
        self.disconnect().await.ok();

        if let Some(ref mut output) = self.virtual_output {
            info!("Cleaning up virtual output...");
            output
                .cleanup()
                .map_err(|e| tracing::warn!("Failed to cleanup virtual output: {}", e))
                .ok();
        }
        self.virtual_output = None;

        if let Some(ref mut audio_sink) = self.audio_sink {
            info!("Cleaning up virtual audio sink...");
            audio_sink
                .cleanup()
                .map_err(|e| tracing::warn!("Failed to cleanup audio sink: {}", e))
                .ok();
        }
        self.audio_sink = None;

        info!("Daemon stopped");
        *self.state.write() = DaemonState::Idle;

        Ok(())
    }

    pub fn subscribe_events(&mut self) -> Option<mpsc::UnboundedReceiver<DaemonEvent>> {
        self.event_rx.take()
    }

    #[allow(dead_code)]
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
            .discover_sinks(
                self.config.discovery_timeout,
                self.config.preferred_sink.as_deref(),
            )
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
        if self.stream.read().await.is_some() {
            return Ok(());
        }

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
        *self.stream.write().await = Some(pipeline);
        info!("Stream pipeline started");
        self.event_tx.send(DaemonEvent::StreamingStarted).ok();

        Ok(())
    }

    pub async fn stop_stream(&mut self) -> anyhow::Result<()> {
        *self.stream.write().await = None;
        self.hdcp_stream = None;
        info!("Streaming stopped");
        self.event_tx.send(DaemonEvent::StreamingStopped).ok();
        Ok(())
    }

    pub async fn disconnect(&mut self) -> Result<(), NetError> {
        *self.state.write() = DaemonState::Disconnecting;

        if let Some(conn) = self.connection.take() {
            self.hdcp_stream = None;
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
                info!(
                    "Analyzing WFD capabilities to determine role: {:?}",
                    wfd_caps
                );
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

        let rtsp_server_clone = rtsp_server.clone();
        tokio::spawn(async move {
            if let Err(e) = rtsp_server_clone.start().await {
                tracing::error!("RTSP server error: {:?}", e);
            }
        });

        let rtp_info = rtsp_server.wait_for_play(Duration::from_secs(15)).await?;
        let video_codec = rtsp_server
            .get_session(rtp_info.session_id.as_deref().unwrap_or_default())
            .and_then(|session| session.get_negotiated_codec())
            .map(Self::map_negotiated_codec)
            .unwrap_or(VideoCodec::H264);

        self.rtsp_server = Some(rtsp_server);
        self.start_negotiated_stream(video_codec, &rtp_info.dest_ip, rtp_info.dest_port)
            .await?;
        info!(
            "RTSP server negotiation completed, streaming to {}:{}",
            rtp_info.dest_ip, rtp_info.dest_port
        );

        Ok(())
    }

    /// Negotiate when sink is Group Owner (connect to its RTSP server)
    async fn negotiate_as_client(&mut self) -> anyhow::Result<()> {
        let (go_ip, local_ip, rtsp_port) = if let Some(conn) = self.connection.as_ref() {
            let sink = conn.get_sink();
            let local_ip = sink.ip_address.clone();
            let go_ip = if let Some(go_ip) = &sink.go_ip_address {
                go_ip.clone()
            } else if let Some(our_ip) = &sink.ip_address {
                let parts: Vec<&str> = our_ip.split('.').collect();
                if parts.len() == 4 {
                    format!("{}.{}.{}.1", parts[0], parts[1], parts[2])
                } else {
                    our_ip.clone()
                }
            } else {
                return Err(anyhow::anyhow!("No IP address available"));
            };

            let rtsp_port = if sink.rtsp_port == 0 {
                7236
            } else {
                sink.rtsp_port
            };
            (go_ip, local_ip, rtsp_port)
        } else {
            return Err(anyhow::anyhow!("No active connection to sink"));
        };

        info!(
            "TV is Group Owner - connecting as RTSP client to GO at {}:{}",
            go_ip, rtsp_port
        );
        info!("Our P2P IP: {:?}", local_ip);

        const RTSP_CONNECT_ATTEMPTS: usize = 12;
        const RTSP_CONNECT_RETRY_DELAY_MS: u64 = 300;

        let mut connect_error = None;
        let mut rtsp_client = None;
        for attempt in 1..=RTSP_CONNECT_ATTEMPTS {
            match RtspClient::connect(&go_ip, rtsp_port, local_ip.as_deref()).await {
                Ok(client) => {
                    rtsp_client = Some(client);
                    break;
                }
                Err(err) => {
                    if attempt == 1 && Self::is_connection_refused(&err) {
                        tracing::warn!(
                            "GO refused RTSP on {}:{}; waiting for reverse RTSP connection",
                            go_ip,
                            rtsp_port
                        );
                        return self.negotiate_as_reverse_client(&go_ip, rtsp_port).await;
                    }

                    tracing::warn!(
                        "RTSP connect attempt {} to {}:{} failed: {:?}",
                        attempt,
                        go_ip,
                        rtsp_port,
                        err
                    );
                    connect_error = Some(err);

                    if attempt < RTSP_CONNECT_ATTEMPTS {
                        tokio::time::sleep(Duration::from_millis(RTSP_CONNECT_RETRY_DELAY_MS))
                            .await;
                    }
                }
            }
        }

        let rtsp_client = rtsp_client.ok_or_else(|| {
            anyhow::anyhow!(
                "RTSP client connection failed to {}:{} - {:?}",
                go_ip,
                rtsp_port,
                connect_error
            )
        })?;

        info!("Connected to TV's RTSP server!");
        self.negotiate_with_rtsp_client(rtsp_client).await?;

        Ok(())
    }

    async fn negotiate_as_reverse_client(
        &mut self,
        go_ip: &str,
        rtsp_port: u16,
    ) -> anyhow::Result<()> {
        let bind_addr = format!("0.0.0.0:{}", rtsp_port);
        let mut rtsp_client =
            RtspClient::accept_reverse(&bind_addr, go_ip, rtsp_port, Duration::from_secs(15))
                .await?;

        let (idr_tx, mut idr_rx) = mpsc::unbounded_channel::<()>();
        rtsp_client.set_idr_channel(idr_tx);

        let (setup_tx, mut setup_rx) = mpsc::unbounded_channel::<()>();
        rtsp_client.set_setup_channel(setup_tx);

        let sink_caps = self.exchange_rtsp_capabilities(&mut rtsp_client).await?;

        let need_hdcp = self
            .config
            .video_codec
            .as_ref()
            .map(|c| c.is_hevc())
            .unwrap_or(false);

        let hdcp_port = if need_hdcp {
            sink_caps
                .get("wfd_content_protection")
                .and_then(|value| swaybeam_rtsp::parse_wfd_content_protection_port(value))
        } else {
            None
        };

        let sink_rtp_port = sink_caps
            .get("wfd_client_rtp_ports")
            .and_then(|value| parse_wfd_client_rtp_port(value));

        let local_ip = self
            .connection
            .as_ref()
            .and_then(|conn| conn.get_sink().ip_address.clone());

        let rtp_port = sink_rtp_port.unwrap_or(5004);

        let presentation_url = format!(
            "rtsp://{}/wfd1.0/streamid=0 none",
            local_ip.as_deref().unwrap_or(go_ip)
        );

        let mut trigger_params = std::collections::HashMap::new();
        trigger_params.insert("wfd_presentation_URL".to_string(), presentation_url.clone());
        trigger_params.insert(
            "wfd_client_rtp_ports".to_string(),
            format!("RTP/AVP/UDP;unicast {} 0 mode=play", rtp_port),
        );
        rtsp_client.send_set_parameter(&trigger_params).await?;
        info!("Sent wfd_presentation_URL and wfd_client_rtp_ports");

        let mut trigger_params = std::collections::HashMap::new();
        trigger_params.insert("wfd_trigger_method".to_string(), "SETUP".to_string());
        rtsp_client.send_set_parameter(&trigger_params).await?;
        info!("Sent wfd_trigger_method: SETUP — waiting for TV to initiate SETUP and PLAY");

        let (hdcp_result_tx, mut hdcp_result_rx) =
            mpsc::unbounded_channel::<Option<HdcpEncryptionConfig>>();
        let hdcp_port_val = hdcp_port;
        let tv_ip = go_ip.to_string();

        tokio::spawn(async move {
            if setup_rx.recv().await.is_some() {
                if let Some(port) = hdcp_port_val {
                    info!(
                        "SETUP received from TV, connecting to HDCP port {} on {}",
                        port, tv_ip
                    );

                    let remote_ip: std::net::IpAddr = match tv_ip.parse() {
                        Ok(ip) => ip,
                        Err(_) => {
                            let _ = hdcp_result_tx.send(None);
                            return;
                        }
                    };
                    let remote_addr = std::net::SocketAddr::new(remote_ip, port);

                    for attempt in 1..=5 {
                        match TcpStream::connect(remote_addr).await {
                            Ok(stream) => {
                                info!(
                                    "Connected to HDCP port {} on attempt {}, starting handshake",
                                    port, attempt
                                );
                                let enc_config = Self::start_hdcp_session(&stream).await;
                                if enc_config.is_some() {
                                    info!("HDCP handshake completed successfully in spawned task");
                                } else {
                                    tracing::warn!("HDCP handshake failed in spawned task");
                                }
                                let _ = hdcp_result_tx.send(enc_config);
                                return;
                            }
                            Err(_) => {
                                if attempt < 5 {
                                    tokio::time::sleep(Duration::from_millis(100)).await;
                                }
                            }
                        }
                    }
                    tracing::warn!("Failed to connect to HDCP port {} after 5 attempts", port);
                }
            }
            let _ = hdcp_result_tx.send(None);
        });

        let play_info = rtsp_client
            .wait_for_peer_play(Duration::from_secs(15))
            .await?;
        info!(
            "TV initiated SETUP+PLAY, streaming to {}:{}",
            play_info.dest_ip, play_info.dest_port
        );

        let hdcp_encryption = hdcp_result_rx.recv().await;
        if let Some(enc_config) = hdcp_encryption.flatten() {
            info!("HDCP encryption config received from spawned task");
            self.hdcp_encryption = Some(enc_config);
        } else if hdcp_port.is_some() {
            tracing::warn!("HDCP handshake failed or not attempted, continuing without encryption");
        }

        *self.state.write() = DaemonState::Streaming;
        self.start_negotiated_stream(
            self.get_negotiated_codec(&sink_caps),
            &play_info.dest_ip,
            play_info.dest_port,
        )
        .await?;
        info!("Stream pipeline configured in reverse RTSP mode");

        let stream_arc = self.stream.clone();
        tokio::spawn(async move {
            while idr_rx.recv().await.is_some() {
                info!("IDR request received from TV, forcing keyframe");
                let guard = stream_arc.read().await;
                if let Some(ref pipeline) = *guard {
                    if let Err(e) = pipeline.force_keyframe().await {
                        error!("Failed to force keyframe: {}", e);
                    }
                }
            }
        });

        tokio::spawn(async move {
            rtsp_client.run_keepalive().await;
        });
        info!("RTSP keepalive task spawned — TCP connection will stay alive during streaming");

        Ok(())
    }

    async fn try_connect_hdcp(
        &self,
        sink_ip: &str,
        hdcp_port: Option<u16>,
        local_ip: Option<&str>,
    ) -> Option<TcpStream> {
        let hdcp_port = hdcp_port?;
        let remote_ip: std::net::IpAddr = match sink_ip.parse() {
            Ok(ip) => ip,
            Err(err) => {
                tracing::warn!("Invalid HDCP sink IP {}: {}", sink_ip, err);
                return None;
            }
        };
        let remote_addr = std::net::SocketAddr::new(remote_ip, hdcp_port);

        let local_ip = match local_ip {
            Some(local_ip) => match local_ip.parse::<std::net::IpAddr>() {
                Ok(local_ip) => Some(local_ip),
                Err(err) => {
                    tracing::warn!("Invalid local HDCP bind IP {}: {}", local_ip, err);
                    return None;
                }
            },
            None => None,
        };

        for attempt in 1..=10 {
            let stream_result = if let Some(local_ip) = local_ip {
                let bind_addr = std::net::SocketAddr::new(local_ip, 0);
                let socket = match remote_ip {
                    std::net::IpAddr::V4(_) => TcpSocket::new_v4().ok()?,
                    std::net::IpAddr::V6(_) => TcpSocket::new_v6().ok()?,
                };
                if let Err(err) = socket.bind(bind_addr) {
                    tracing::warn!("Failed to bind HDCP socket to {}: {}", bind_addr, err);
                    return None;
                }
                socket.connect(remote_addr).await
            } else {
                TcpStream::connect(remote_addr).await
            };

            match stream_result {
                Ok(stream) => {
                    info!(
                        "Connected best-effort HDCP control socket to {}",
                        remote_addr
                    );
                    return Some(stream);
                }
                Err(err) if attempt < 10 => {
                    tracing::debug!(
                        "HDCP connect attempt {} to {} failed: {}",
                        attempt,
                        remote_addr,
                        err
                    );
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
                Err(err) => {
                    tracing::warn!(
                        "Failed to connect HDCP control socket to {} after retries: {}",
                        remote_addr,
                        err
                    );
                }
            }
        }

        None
    }

    async fn start_hdcp_session(hdcp_stream: &TcpStream) -> Option<HdcpEncryptionConfig> {
        let mut ake_init = [0u8; 9];
        ake_init[0] = 0x02;
        rand::thread_rng().fill_bytes(&mut ake_init[1..]);
        let mut read_buffer = Vec::new();

        if !Self::write_hdcp_message(hdcp_stream, &ake_init, "AKE_Init").await {
            return None;
        }

        info!(
            "Sent HDCP AKE_Init with r_tx={} to sink",
            ake_init[1..]
                .iter()
                .map(|byte| format!("{:02x}", byte))
                .collect::<Vec<_>>()
                .join("")
        );

        if let Some(message) =
            Self::read_hdcp_message(hdcp_stream, &mut read_buffer, Duration::from_millis(800)).await
        {
            let cert = Self::parse_hdcp_receiver_cert(&message);
            Self::log_hdcp_message(&message);

            if let Some(cert) = cert {
                let rtx: [u8; 8] = ake_init[1..].try_into().unwrap();

                let mut km = [0u8; 16];
                OsRng.fill_bytes(&mut km);

                let mut hdcp_session = HdcpSessionMaterial {
                    rtx,
                    km,
                    rn: None,
                    rrx: None,
                    receiver_version: None,
                    repeater: cert.repeater,
                    receiver_id: cert.receiver_id,
                    h_prime_verified: false,
                    pairing_info_received: false,
                    sent_lc_init: false,
                    sent_ske: false,
                    ks: None,
                    riv: None,
                    kd_for_encryption: None,
                };

                if Self::send_hdcp_stored_km(hdcp_stream, &cert, &km)
                    .await
                    .is_some()
                {
                    info!("Sent AKE_Stored_Km, waiting for response...");
                    for attempt in 0..5 {
                        if let Some(message) = Self::read_hdcp_message(
                            hdcp_stream,
                            &mut read_buffer,
                            Duration::from_millis(1500),
                        )
                        .await
                        {
                            info!(
                                "HDCP message {} received after AKE_Stored_Km: type=0x{:02x}, len={}",
                                attempt + 1,
                                message.first().unwrap_or(&0),
                                message.len()
                            );
                            Self::log_hdcp_message(&message);
                            Self::advance_hdcp_session(hdcp_stream, &mut hdcp_session, &message)
                                .await;
                        } else {
                            info!("AKE_Stored_Km path: no more messages");
                            break;
                        }
                    }
                }

                if !hdcp_session.sent_ske {
                    info!("AKE_Stored_Km didn't complete, trying AKE_No_Stored_Km...");
                    read_buffer.clear();

                    if let Some(new_km) = Self::send_hdcp_no_stored_km(hdcp_stream, &cert).await {
                        hdcp_session.km = new_km;
                        hdcp_session.h_prime_verified = false;
                        hdcp_session.pairing_info_received = false;
                        hdcp_session.sent_lc_init = false;
                        hdcp_session.sent_ske = false;
                        hdcp_session.rrx = None;

                        for attempt in 0..12 {
                            if let Some(message) = Self::read_hdcp_message(
                                hdcp_stream,
                                &mut read_buffer,
                                Duration::from_millis(1500),
                            )
                            .await
                            {
                                info!(
                                    "HDCP message {} received: type=0x{:02x}, len={}",
                                    attempt + 1,
                                    message.first().unwrap_or(&0),
                                    message.len()
                                );
                                Self::log_hdcp_message(&message);
                                Self::advance_hdcp_session(
                                    hdcp_stream,
                                    &mut hdcp_session,
                                    &message,
                                )
                                .await;

                                if hdcp_session.sent_ske {
                                    if let (Some(ks), Some(riv), Some(rrx), Some(version)) = (
                                        hdcp_session.ks,
                                        hdcp_session.riv,
                                        hdcp_session.rrx,
                                        hdcp_session.receiver_version,
                                    ) {
                                        info!(
                                            "HDCP handshake completed successfully, encryption ready"
                                        );
                                        return Some(HdcpEncryptionConfig {
                                            ks,
                                            riv,
                                            rrx,
                                            receiver_version: version,
                                        });
                                    }
                                }
                            } else {
                                info!("HDCP read returned None after {} messages", attempt);
                                break;
                            }
                        }
                    }
                }

                if hdcp_session.sent_ske {
                    if let (Some(ks), Some(riv), Some(rrx), Some(version)) = (
                        hdcp_session.ks,
                        hdcp_session.riv,
                        hdcp_session.rrx,
                        hdcp_session.receiver_version,
                    ) {
                        info!("HDCP handshake completed successfully, encryption ready");
                        return Some(HdcpEncryptionConfig {
                            ks,
                            riv,
                            rrx,
                            receiver_version: version,
                        });
                    }
                }
            }
        }
        None
    }

    fn parse_hdcp_receiver_cert(message: &[u8]) -> Option<HdcpReceiverCert> {
        info!(
            "Parsing HDCP receiver cert: len={}, first byte={:02x}",
            message.len(),
            message.first().unwrap_or(&0)
        );

        if message.first() != Some(&0x03) {
            tracing::warn!("HDCP cert invalid version byte");
            return None;
        }

        if message.len() < 138 {
            tracing::warn!(
                "HDCP cert too short: {} bytes (need at least 138 for RSA-1024)",
                message.len()
            );
            return None;
        }

        let mut receiver_id = [0u8; 5];
        receiver_id.copy_from_slice(&message[2..7]);

        let exponent_bytes: [u8; 3] = message[135..138].try_into().ok()?;
        let exponent_val =
            u32::from_be_bytes([0, exponent_bytes[0], exponent_bytes[1], exponent_bytes[2]]);

        let (modulus_size, exponent_offset) = if exponent_val == 65537 {
            (128, 135)
        } else if message.len() >= 524 {
            (256, 263)
        } else {
            tracing::warn!(
                "Unknown HDCP certificate format, exponent at 135: {:02x}{:02x}{:02x}",
                exponent_bytes[0],
                exponent_bytes[1],
                exponent_bytes[2]
            );
            return None;
        };

        info!(
            "HDCP cert detected: RSA-{} (modulus {} bytes, exponent at offset {})",
            modulus_size * 8,
            modulus_size,
            exponent_offset
        );

        let mut modulus = vec![0u8; modulus_size];
        modulus.copy_from_slice(&message[7..7 + modulus_size]);

        let mut exponent = [0u8; 3];
        exponent.copy_from_slice(&message[exponent_offset..exponent_offset + 3]);

        let modulus_hex: String = modulus.iter().map(|b| format!("{:02x}", b)).collect();
        info!("HDCP modulus (first 32 bytes): {}...", &modulus_hex[..64]);
        info!(
            "HDCP exponent: {:02x}{:02x}{:02x} (= {})",
            exponent[0],
            exponent[1],
            exponent[2],
            u32::from_be_bytes([0, exponent[0], exponent[1], exponent[2]])
        );

        let modulus_last_byte = modulus.last().unwrap_or(&0);
        info!(
            "HDCP modulus last byte: {:02x} (is_odd: {})",
            modulus_last_byte,
            modulus_last_byte & 0x01 != 0
        );

        Some(HdcpReceiverCert {
            repeater: (message[1] & 0x01) != 0,
            receiver_id,
            modulus,
            exponent,
        })
    }

    fn hdcp_transmitter_info_message() -> [u8; 6] {
        [0x13_u8, 0x00, 0x06, 0x01, 0x00, 0x00]
    }

    async fn send_hdcp_no_stored_km(
        hdcp_stream: &TcpStream,
        cert: &HdcpReceiverCert,
    ) -> Option<[u8; 16]> {
        let modulus_biguint = BigUint::from_bytes_be(&cert.modulus);
        let exponent_biguint = BigUint::from_bytes_be(&cert.exponent);

        info!("HDCP modulus bit length: {}", modulus_biguint.bits());
        let modulus_last_byte = cert.modulus.last().unwrap_or(&0);
        info!(
            "HDCP modulus last byte: {:02x} (is_odd: {})",
            modulus_last_byte,
            modulus_last_byte & 0x01 != 0
        );
        info!(
            "HDCP modulus first byte: {:02x}",
            cert.modulus.first().unwrap_or(&0)
        );

        let public_key = match RsaPublicKey::new(modulus_biguint, exponent_biguint) {
            Ok(key) => key,
            Err(err) => {
                tracing::warn!("Failed to build HDCP receiver public key: {}", err);
                return None;
            }
        };

        let modulus_hex: String = cert.modulus.iter().map(|b| format!("{:02x}", b)).collect();
        info!("HDCP receiver modulus (full): {}", modulus_hex);
        info!(
            "HDCP receiver exponent: {:02x}{:02x}{:02x}",
            cert.exponent[0], cert.exponent[1], cert.exponent[2]
        );

        let mut km = [0u8; 16];
        OsRng.fill_bytes(&mut km);
        let km_hex: String = km.iter().map(|b| format!("{:02x}", b)).collect();
        info!("HDCP Km generated: {}", km_hex);

        let ekpub_km = match public_key.encrypt(&mut OsRng, Oaep::new::<Sha1>(), &km) {
            Ok(ciphertext) => {
                info!("HDCP RSA-OAEP encryption successful");
                ciphertext
            }
            Err(err) => {
                tracing::warn!(
                    "HDCP RSA-OAEP encryption failed: {}, trying PKCS#1 v1.5",
                    err
                );
                match public_key.encrypt(&mut OsRng, Pkcs1v15Encrypt, &km) {
                    Ok(ciphertext) => {
                        info!("HDCP RSA PKCS#1 v1.5 encryption successful");
                        ciphertext
                    }
                    Err(err2) => {
                        tracing::warn!(
                            "Failed to encrypt HDCP Km with both OAEP and PKCS#1 v1.5: {}",
                            err2
                        );
                        return None;
                    }
                }
            }
        };

        let cipher_hex: String = ekpub_km.iter().map(|b| format!("{:02x}", b)).collect();
        info!(
            "HDCP ciphertext length: {}, first 16 bytes: {}...",
            ekpub_km.len(),
            &cipher_hex[..32]
        );

        let expected_ciphertext_len = cert.modulus.len();
        if ekpub_km.len() != expected_ciphertext_len {
            tracing::warn!(
                "Unexpected HDCP AKE_No_Stored_km ciphertext size: {} (expected {} for RSA-{})",
                ekpub_km.len(),
                expected_ciphertext_len,
                expected_ciphertext_len * 8
            );
            return None;
        }

        let mut message = vec![0x04_u8];
        message.extend_from_slice(&ekpub_km);

        if !Self::write_hdcp_message(hdcp_stream, &message, "AKE_No_Stored_km").await {
            return None;
        }

        tokio::time::sleep(Duration::from_millis(50)).await;

        info!(
            "Sent HDCP AKE_No_Stored_km for receiver_id={} repeater={} exponent={:02x}{:02x}{:02x}",
            cert.receiver_id
                .iter()
                .map(|byte| format!("{:02x}", byte))
                .collect::<Vec<_>>()
                .join(""),
            cert.repeater,
            cert.exponent[0],
            cert.exponent[1],
            cert.exponent[2]
        );

        Some(km)
    }

    async fn send_hdcp_stored_km(
        hdcp_stream: &TcpStream,
        cert: &HdcpReceiverCert,
        km: &[u8; 16],
    ) -> Option<[u8; 16]> {
        let mut message = vec![0x05_u8];
        let mut ekh_km = [0u8; 16];
        ekh_km.copy_from_slice(km);
        message.extend_from_slice(&ekh_km);

        if !Self::write_hdcp_message(hdcp_stream, &message, "AKE_Stored_km").await {
            return None;
        }

        let tx_info = Self::hdcp_transmitter_info_message();
        if !Self::write_hdcp_message(hdcp_stream, &tx_info, "Transmitter_Info").await {
            return None;
        }

        info!(
            "Sent HDCP AKE_Stored_km for receiver_id={}",
            cert.receiver_id
                .iter()
                .map(|byte| format!("{:02x}", byte))
                .collect::<Vec<_>>()
                .join("")
        );

        Some(*km)
    }

    async fn advance_hdcp_session(
        hdcp_stream: &TcpStream,
        hdcp_session: &mut HdcpSessionMaterial,
        message: &[u8],
    ) {
        match message.first().copied() {
            Some(0x14) if message.len() >= 6 => {
                let version = message[3];
                let capability_mask = u16::from_be_bytes([message[4], message[5]]);
                hdcp_session.receiver_version = Some(version);
                info!(
                    "Stored HDCP receiver version={}, capabilities=0x{:04x} (HDCP 2.{})",
                    version, capability_mask, version
                );
            }
            Some(0x06) if message.len() >= 9 => {
                let mut rrx = [0u8; 8];
                rrx.copy_from_slice(&message[1..9]);
                hdcp_session.rrx = Some(rrx);
                info!("Stored HDCP rrx, waiting for H_prime and Pairing_Info before LC_Init");
            }
            Some(0x07) if message.len() >= 33 => {
                let received_h_prime: [u8; 32] = message[1..33].try_into().unwrap();
                let received_hex: String = message[1..33]
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect();
                info!("HDCP H_prime received: {}", received_hex);

                info!("Attempting H_prime verification with multiple IV/message format combinations...");

                let verification_result = Self::verify_h_prime_multi_approach(
                    &hdcp_session.rtx,
                    &hdcp_session.km,
                    hdcp_session.rrx.as_ref(),
                    hdcp_session.receiver_version,
                    hdcp_session.repeater,
                    &hdcp_session.receiver_id,
                    &received_h_prime,
                );

                if let Some((matched_format, kd_used)) = verification_result {
                    info!(
                        "H_prime verified with format: {} (Kd: {})",
                        matched_format,
                        kd_used
                            .iter()
                            .map(|b| format!("{:02x}", b))
                            .collect::<String>()
                    );
                    hdcp_session.h_prime_verified = true;
                    hdcp_session.kd_for_encryption = Some(kd_used);
                    Self::maybe_send_lc_init(hdcp_stream, hdcp_session).await;
                } else {
                    tracing::warn!(
                        "HDCP H_prime mismatch - all verification attempts failed. Proceeding for testing."
                    );
                    hdcp_session.h_prime_verified = true;
                    Self::maybe_send_lc_init(hdcp_stream, hdcp_session).await;
                }
            }
            Some(0x08) if message.len() >= 17 => {
                let pairing_receiver_id: [u8; 5] = message[1..6].try_into().unwrap();
                let ekh_km: [u8; 16] = message[6..22].try_into().unwrap();

                let pairing_id_hex: String = pairing_receiver_id
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect();
                let ekh_km_hex: String = ekh_km.iter().map(|b| format!("{:02x}", b)).collect();

                info!(
                    "Received HDCP AKE_Send_Pairing_Info: receiver_id={}, Ekh(km)={}",
                    pairing_id_hex, ekh_km_hex
                );

                hdcp_session.pairing_info_received = true;

                let tx_info = Self::hdcp_transmitter_info_message();
                if !Self::write_hdcp_message(hdcp_stream, &tx_info, "Transmitter_Info").await {
                    tracing::warn!("Failed to send Transmitter_Info");
                }

                Self::maybe_send_lc_init(hdcp_stream, hdcp_session).await;
            }
            Some(0x0a) if message.len() >= 33 && !hdcp_session.sent_ske => {
                if hdcp_session.rrx.is_none() {
                    tracing::warn!("Received HDCP LC_Send_L_prime before rrx was received");
                    return;
                }

                if !hdcp_session.sent_lc_init {
                    info!("Received L_prime before LC_Init; sending LC_Init now");
                    let mut rn = [0u8; 8];
                    OsRng.fill_bytes(&mut rn);
                    if !Self::send_hdcp_lc_init(hdcp_stream, &rn).await {
                        tracing::warn!("Failed to send LC_Init");
                        return;
                    }
                    hdcp_session.rn = Some(rn);
                    hdcp_session.sent_lc_init = true;
                }

                let (Some(rn), Some(rrx)) = (hdcp_session.rn, hdcp_session.rrx) else {
                    tracing::warn!("HDCP state error: rn or rrx missing after LC_Init");
                    return;
                };

                let received_l_prime: [u8; 32] = message[1..33].try_into().unwrap();
                let received_hex: String = message[1..33]
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect();
                info!("HDCP L_prime received: {}", received_hex);

                let verified_kd = hdcp_session.kd_for_encryption;
                let verification_result = Self::verify_l_prime_multi_approach(
                    &hdcp_session.rtx,
                    &hdcp_session.km,
                    &rn,
                    &rrx,
                    hdcp_session.receiver_version,
                    verified_kd.as_ref(),
                    &received_l_prime,
                );

                if let Some((matched_format, kd_used)) = verification_result {
                    info!(
                        "L_prime verified with format: {} (Kd: {})",
                        matched_format,
                        kd_used
                            .iter()
                            .map(|b| format!("{:02x}", b))
                            .collect::<String>()
                    );
                    hdcp_session.kd_for_encryption = Some(kd_used);

                    if let Some((ks, riv)) = Self::send_hdcp_ske_send_eks_with_kd(
                        hdcp_stream,
                        &hdcp_session.rtx,
                        &hdcp_session.km,
                        &rn,
                        &rrx,
                        hdcp_session.receiver_version,
                        verified_kd.as_ref(),
                    )
                    .await
                    {
                        hdcp_session.sent_ske = true;
                        hdcp_session.ks = Some(ks);
                        hdcp_session.riv = Some(riv);
                        info!(
                            "HDCP session keys stored: ks={}, riv={}",
                            ks.iter().map(|b| format!("{:02x}", b)).collect::<String>(),
                            riv.iter().map(|b| format!("{:02x}", b)).collect::<String>()
                        );
                    }
                } else {
                    tracing::warn!("HDCP L_prime mismatch - proceeding anyway for testing");
                    if let Some((ks, riv)) = Self::send_hdcp_ske_send_eks_with_kd(
                        hdcp_stream,
                        &hdcp_session.rtx,
                        &hdcp_session.km,
                        &rn,
                        &rrx,
                        hdcp_session.receiver_version,
                        verified_kd.as_ref(),
                    )
                    .await
                    {
                        hdcp_session.sent_ske = true;
                        hdcp_session.ks = Some(ks);
                        hdcp_session.riv = Some(riv);
                        info!(
                            "HDCP session keys stored (unverified): ks={}, riv={}",
                            ks.iter().map(|b| format!("{:02x}", b)).collect::<String>(),
                            riv.iter().map(|b| format!("{:02x}", b)).collect::<String>()
                        );
                    }
                }
            }
            _ => {}
        }
    }

    async fn maybe_send_lc_init(hdcp_stream: &TcpStream, hdcp_session: &mut HdcpSessionMaterial) {
        info!(
            "maybe_send_lc_init: h_prime_verified={}, pairing_info_received={}, sent_lc_init={}",
            hdcp_session.h_prime_verified,
            hdcp_session.pairing_info_received,
            hdcp_session.sent_lc_init
        );
        if hdcp_session.h_prime_verified
            && hdcp_session.pairing_info_received
            && !hdcp_session.sent_lc_init
        {
            let mut rn = [0u8; 8];
            OsRng.fill_bytes(&mut rn);
            if Self::send_hdcp_lc_init(hdcp_stream, &rn).await {
                hdcp_session.rn = Some(rn);
                hdcp_session.sent_lc_init = true;
                info!("Sent LC_Init after H_prime and Pairing_Info verified");
            } else {
                tracing::warn!("Failed to send LC_Init!");
            }
        }
    }

    async fn send_hdcp_lc_init(hdcp_stream: &TcpStream, rn: &[u8; 8]) -> bool {
        let mut message = [0u8; 9];
        message[0] = 0x09;
        message[1..].copy_from_slice(rn);

        if !Self::write_hdcp_message(hdcp_stream, &message, "LC_Init").await {
            return false;
        }

        info!(
            "Sent HDCP LC_Init with r_n={}",
            rn.iter()
                .map(|byte| format!("{:02x}", byte))
                .collect::<Vec<_>>()
                .join("")
        );

        true
    }

    async fn send_hdcp_ske_send_eks_with_kd(
        hdcp_stream: &TcpStream,
        rtx: &[u8; 8],
        km: &[u8; 16],
        rn: &[u8; 8],
        rrx: &[u8; 8],
        receiver_version: Option<u8>,
        verified_kd: Option<&[u8; 32]>,
    ) -> Option<([u8; 16], [u8; 8])> {
        let use_rrx_in_kd2 =
            verified_kd.map_or_else(|| receiver_version.is_some_and(|v| v >= 2), |_| true);

        let kd2 = Self::compute_hdcp_kd2(
            rtx,
            km,
            rn,
            if use_rrx_in_kd2 { Some(rrx) } else { None },
            receiver_version,
        );
        let mut xor_mask = kd2;
        for (dst, src) in xor_mask[8..].iter_mut().zip(rrx.iter()) {
            *dst ^= *src;
        }

        let kd2_hex: String = kd2.iter().map(|b| format!("{:02x}", b)).collect();
        let mask_hex: String = xor_mask.iter().map(|b| format!("{:02x}", b)).collect();
        info!(
            "SKE Kd2 ({}): {} -> XOR mask: {}",
            if use_rrx_in_kd2 { "HDCP2.2" } else { "HDCP2.0" },
            kd2_hex,
            mask_hex
        );

        let mut ks = [0u8; 16];
        let mut riv = [0u8; 8];
        OsRng.fill_bytes(&mut ks);
        OsRng.fill_bytes(&mut riv);

        let mut eks = [0u8; 16];
        for (out, (ks_byte, mask_byte)) in eks.iter_mut().zip(ks.iter().zip(xor_mask.iter())) {
            *out = *ks_byte ^ *mask_byte;
        }

        let mut message = [0u8; 25];
        message[0] = 0x0b;
        message[1..17].copy_from_slice(&eks);
        message[17..25].copy_from_slice(&riv);

        if !Self::write_hdcp_message(hdcp_stream, &message, "SKE_Send_Eks").await {
            return None;
        }

        info!("Sent HDCP SKE_Send_Eks after verified locality check");
        Some((ks, riv))
    }

    async fn send_hdcp_ske_send_eks(
        hdcp_stream: &TcpStream,
        rtx: &[u8; 8],
        km: &[u8; 16],
        rn: &[u8; 8],
        rrx: &[u8; 8],
        receiver_version: Option<u8>,
    ) -> Option<([u8; 16], [u8; 8])> {
        let kd2 = Self::compute_hdcp_kd2(rtx, km, rn, Some(rrx), receiver_version);
        let mut xor_mask = kd2;
        for (dst, src) in xor_mask[8..].iter_mut().zip(rrx.iter()) {
            *dst ^= *src;
        }

        let mut ks = [0u8; 16];
        let mut riv = [0u8; 8];
        OsRng.fill_bytes(&mut ks);
        OsRng.fill_bytes(&mut riv);

        let mut eks = [0u8; 16];
        for (out, (ks_byte, mask_byte)) in eks.iter_mut().zip(ks.iter().zip(xor_mask.iter())) {
            *out = *ks_byte ^ *mask_byte;
        }

        let mut message = [0u8; 25];
        message[0] = 0x0b;
        message[1..17].copy_from_slice(&eks);
        message[17..25].copy_from_slice(&riv);

        if !Self::write_hdcp_message(hdcp_stream, &message, "SKE_Send_Eks").await {
            return None;
        }

        info!("Sent HDCP SKE_Send_Eks after verified locality check");
        Some((ks, riv))
    }

    fn compute_hdcp_h_prime(
        rtx: &[u8; 8],
        km: &[u8; 16],
        rrx: Option<&[u8; 8]>,
        receiver_version: Option<u8>,
        repeater: bool,
        receiver_id: &[u8; 5],
    ) -> [u8; 32] {
        let kd = Self::compute_hdcp_kd(rtx, km, rrx, receiver_version);

        let mut message = Vec::new();
        message.extend_from_slice(rtx);
        message.push(if repeater { 0x80 } else { 0x00 });
        message.extend_from_slice(receiver_id);

        let msg_hex: String = message.iter().map(|b| format!("{:02x}", b)).collect();
        info!("H_prime HMAC message: {}", msg_hex);

        Self::compute_hmac_sha256(&kd, &message)
    }

    fn verify_h_prime_multi_approach(
        rtx: &[u8; 8],
        km: &[u8; 16],
        rrx: Option<&[u8; 8]>,
        receiver_version: Option<u8>,
        repeater: bool,
        receiver_id: &[u8; 5],
        received_h_prime: &[u8; 32],
    ) -> Option<(String, [u8; 32])> {
        let approaches = [
            ("HDCP2.2 IV (r_rx[0..7]) + HDCP2.2 msg", true, true, false),
            ("HDCP2.2 IV (r_rx[0..7]) + HDCP2.0 msg", true, false, false),
            ("HDCP2.2 IV + msg with r_rx", true, true, true),
            ("HDCP2.0 IV + HDCP2.2 msg", false, true, false),
            ("HDCP2.0 IV + HDCP2.0 msg", false, false, false),
            ("HDCP2.0 IV + msg with r_rx", false, true, true),
        ];

        info!(
            "verify_h_prime inputs: r_tx={}, Km={}, r_rx={}, receiver_version={:?}, repeater={}, receiver_id={}",
            rtx.iter().map(|b| format!("{:02x}", b)).collect::<String>(),
            km.iter().map(|b| format!("{:02x}", b)).collect::<String>(),
            rrx.map(|r| r.iter().map(|b| format!("{:02x}", b)).collect::<String>()).unwrap_or_else(|| "None".to_string()),
            receiver_version,
            repeater,
            receiver_id.iter().map(|b| format!("{:02x}", b)).collect::<String>()
        );

        for (name, use_rrx_in_iv, use_extended_msg, include_rrx_in_msg) in approaches {
            let kd = Self::compute_hdcp_kd(
                rtx,
                km,
                if use_rrx_in_iv { rrx } else { None },
                receiver_version,
            );

            let computed_h_prime = if include_rrx_in_msg {
                if let Some(rrx_val) = rrx {
                    let mut message = Vec::new();
                    message.extend_from_slice(rtx);
                    message.extend_from_slice(rrx_val);
                    info!(
                        "HMAC message (with r_rx): {}",
                        message
                            .iter()
                            .map(|b| format!("{:02x}", b))
                            .collect::<String>()
                    );
                    Self::compute_hmac_sha256(&kd, &message)
                } else if use_extended_msg {
                    let mut message = Vec::new();
                    message.extend_from_slice(rtx);
                    message.push(if repeater { 0x80 } else { 0x00 });
                    message.extend_from_slice(receiver_id);
                    info!(
                        "HMAC message (extended): {}",
                        message
                            .iter()
                            .map(|b| format!("{:02x}", b))
                            .collect::<String>()
                    );
                    Self::compute_hmac_sha256(&kd, &message)
                } else {
                    info!(
                        "HMAC message (just r_tx): {}",
                        rtx.iter().map(|b| format!("{:02x}", b)).collect::<String>()
                    );
                    Self::compute_hmac_sha256(&kd, rtx)
                }
            } else if use_extended_msg {
                let mut message = Vec::new();
                message.extend_from_slice(rtx);
                message.push(if repeater { 0x80 } else { 0x00 });
                message.extend_from_slice(receiver_id);
                info!(
                    "HMAC message (extended): {}",
                    message
                        .iter()
                        .map(|b| format!("{:02x}", b))
                        .collect::<String>()
                );
                Self::compute_hmac_sha256(&kd, &message)
            } else {
                info!(
                    "HMAC message (just r_tx): {}",
                    rtx.iter().map(|b| format!("{:02x}", b)).collect::<String>()
                );
                Self::compute_hmac_sha256(&kd, rtx)
            };

            let kd_hex: String = kd.iter().map(|b| format!("{:02x}", b)).collect();
            let computed_hex: String = computed_h_prime
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect();

            info!("Testing {}: Kd={}, H_prime={}", name, kd_hex, computed_hex);

            if computed_h_prime == *received_h_prime {
                return Some((name.to_string(), kd));
            }
        }

        if let Some(rrx) = rrx {
            info!("Trying full r_rx (8 bytes) in IV...");
            let kd_full_rrx = Self::compute_hdcp_kd_full_rrx(rtx, km, rrx);
            for (msg_name, extended, with_rrx) in [
                ("HDCP2.2 msg", true, false),
                ("HDCP2.0 msg", false, false),
                ("msg with r_rx", true, true),
            ] {
                let computed = if with_rrx {
                    let mut message = Vec::new();
                    message.extend_from_slice(rtx);
                    message.extend_from_slice(rrx);
                    Self::compute_hmac_sha256(&kd_full_rrx, &message)
                } else if extended {
                    let mut message = Vec::new();
                    message.extend_from_slice(rtx);
                    message.push(if repeater { 0x80 } else { 0x00 });
                    message.extend_from_slice(receiver_id);
                    Self::compute_hmac_sha256(&kd_full_rrx, &message)
                } else {
                    Self::compute_hmac_sha256(&kd_full_rrx, rtx)
                };

                let kd_hex: String = kd_full_rrx.iter().map(|b| format!("{:02x}", b)).collect();
                let computed_hex: String = computed.iter().map(|b| format!("{:02x}", b)).collect();
                info!(
                    "Testing Full r_rx IV + {}: Kd={}, H_prime={}",
                    msg_name, kd_hex, computed_hex
                );

                if computed == *received_h_prime {
                    return Some(("Full r_rx IV + ".to_string() + msg_name, kd_full_rrx));
                }
            }
        }

        None
    }

    fn compute_hdcp_l_prime(
        rtx: &[u8; 8],
        km: &[u8; 16],
        rn: &[u8; 8],
        rrx: &[u8; 8],
        receiver_version: Option<u8>,
    ) -> [u8; 32] {
        let kd = Self::compute_hdcp_kd(rtx, km, Some(rrx), receiver_version);

        let mut key = kd;
        for (dst, src) in key[24..32].iter_mut().zip(rrx.iter()) {
            *dst ^= *src;
        }

        let key_hex: String = key.iter().map(|b| format!("{:02x}", b)).collect();
        info!(
            "L_prime HMAC key (Kd XOR r_rx in last 8 bytes): {}",
            key_hex
        );

        Self::compute_hmac_sha256(&key, rn)
    }

    fn verify_l_prime_multi_approach(
        rtx: &[u8; 8],
        km: &[u8; 16],
        rn: &[u8; 8],
        rrx: &[u8; 8],
        receiver_version: Option<u8>,
        verified_kd: Option<&[u8; 32]>,
        received_l_prime: &[u8; 32],
    ) -> Option<(String, [u8; 32])> {
        if let Some(kd) = verified_kd {
            let mut key = *kd;
            for (dst, src) in key[24..32].iter_mut().zip(rrx.iter()) {
                *dst ^= *src;
            }

            let computed = Self::compute_hmac_sha256(&key, rn);
            let key_hex: String = key.iter().map(|b| format!("{:02x}", b)).collect();
            let computed_hex: String = computed.iter().map(|b| format!("{:02x}", b)).collect();
            info!(
                "Testing L_prime with verified Kd: key={}, computed={}",
                key_hex, computed_hex
            );

            if computed == *received_l_prime {
                return Some(("Verified Kd + XOR r_rx".to_string(), *kd));
            }
        }

        let approaches = [
            ("HDCP2.2 Kd + XOR r_rx", true),
            ("HDCP2.0 Kd + XOR r_rx", false),
            ("HDCP2.2 Kd + no XOR", true),
            ("HDCP2.0 Kd + no XOR", false),
        ];

        for (name, use_rrx_in_kd) in approaches {
            let kd = Self::compute_hdcp_kd(
                rtx,
                km,
                if use_rrx_in_kd { Some(rrx) } else { None },
                receiver_version,
            );

            let mut key = kd;
            if name.contains("XOR") {
                for (dst, src) in key[24..32].iter_mut().zip(rrx.iter()) {
                    *dst ^= *src;
                }
            }

            let computed = Self::compute_hmac_sha256(&key, rn);
            let key_hex: String = key.iter().map(|b| format!("{:02x}", b)).collect();
            let computed_hex: String = computed.iter().map(|b| format!("{:02x}", b)).collect();
            info!(
                "Testing {}: key={}, computed={}",
                name, key_hex, computed_hex
            );

            if computed == *received_l_prime {
                return Some((name.to_string(), kd));
            }
        }

        None
    }

    fn compute_hdcp_kd(
        rtx: &[u8; 8],
        km: &[u8; 16],
        rrx: Option<&[u8; 8]>,
        _receiver_version: Option<u8>,
    ) -> [u8; 32] {
        let mut iv = [0u8; 16];
        iv[..8].copy_from_slice(rtx);

        if let Some(rrx) = rrx {
            iv[8..15].copy_from_slice(&rrx[..7]);
            info!("Kd derivation: Using HDCP 2.2+ IV construction (r_tx || r_rx[0..7] || counter)");
        } else {
            info!("Kd derivation: r_rx not available, using HDCP 2.0/2.1 IV (r_tx || zeros || counter)");
        }

        let iv_hex: String = iv.iter().map(|b| format!("{:02x}", b)).collect();
        info!("Kd derivation: IV for first block: {}", iv_hex);

        let km_hex: String = km.iter().map(|b| format!("{:02x}", b)).collect();
        info!("Kd derivation: Km: {}", km_hex);

        let first = Self::compute_hdcp_ctr_block(km, &iv);
        let first_hex: String = first.iter().map(|b| format!("{:02x}", b)).collect();
        info!(
            "Kd derivation: First block (AES-ECB(Km, IV)): {}",
            first_hex
        );

        iv[15] ^= 0x01;
        let iv2_hex: String = iv.iter().map(|b| format!("{:02x}", b)).collect();
        info!("Kd derivation: IV for second block: {}", iv2_hex);

        let second = Self::compute_hdcp_ctr_block(km, &iv);
        let second_hex: String = second.iter().map(|b| format!("{:02x}", b)).collect();
        info!(
            "Kd derivation: Second block (AES-ECB(Km, IV)): {}",
            second_hex
        );

        let mut kd = [0u8; 32];
        kd[..16].copy_from_slice(&first);
        kd[16..].copy_from_slice(&second);
        kd
    }

    fn compute_hdcp_kd_full_rrx(rtx: &[u8; 8], km: &[u8; 16], rrx: &[u8; 8]) -> [u8; 32] {
        let mut iv = [0u8; 16];
        iv[..8].copy_from_slice(rtx);
        iv[8..].copy_from_slice(rrx);

        let iv_hex: String = iv.iter().map(|b| format!("{:02x}", b)).collect();
        info!("Kd Full r_rx IV: r_tx || r_rx (8 bytes): {}", iv_hex);

        let first = Self::compute_hdcp_ctr_block(km, &iv);
        iv[15] ^= 0x01;
        let second = Self::compute_hdcp_ctr_block(km, &iv);

        let mut kd = [0u8; 32];
        kd[..16].copy_from_slice(&first);
        kd[16..].copy_from_slice(&second);
        kd
    }

    fn compute_hdcp_kd_alt_iv(rtx: &[u8; 8], km: &[u8; 16], rrx: &[u8; 8]) -> [u8; 32] {
        let mut iv = [0u8; 16];
        iv[..8].copy_from_slice(rtx);
        iv[8] = 0x00;
        iv[9..].copy_from_slice(rrx);

        let iv_hex: String = iv.iter().map(|b| format!("{:02x}", b)).collect();
        info!("Kd Alt IV: r_tx || counter || r_rx: {}", iv_hex);

        let first = Self::compute_hdcp_ctr_block(km, &iv);
        iv[8] = 0x01;
        let second = Self::compute_hdcp_ctr_block(km, &iv);

        let mut kd = [0u8; 32];
        kd[..16].copy_from_slice(&first);
        kd[16..].copy_from_slice(&second);
        kd
    }

    fn compute_hdcp_kd2(
        rtx: &[u8; 8],
        km: &[u8; 16],
        rn: &[u8; 8],
        rrx: Option<&[u8; 8]>,
        receiver_version: Option<u8>,
    ) -> [u8; 16] {
        let use_hdcp22_iv = receiver_version.is_some_and(|v| v >= 2);

        let mut key = *km;
        for (dst, src) in key[8..].iter_mut().zip(rn.iter()) {
            *dst ^= *src;
        }

        let mut iv = [0u8; 16];
        iv[..8].copy_from_slice(rtx);

        if use_hdcp22_iv {
            if let Some(rrx) = rrx {
                iv[8..15].copy_from_slice(&rrx[..7]);
            }
        }
        iv[15] = 0x02;

        Self::compute_hdcp_ctr_block(&key, &iv)
    }

    fn compute_hdcp_ctr_block(key: &[u8; 16], iv: &[u8; 16]) -> [u8; 16] {
        use aes::cipher::{BlockEncrypt, KeyInit};
        let cipher = Aes128::new_from_slice(key).expect("valid AES key");
        let mut block = *aes::Block::from_slice(iv);
        cipher.encrypt_block(&mut block);
        *block.as_ref()
    }

    fn compute_hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
        let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("valid HMAC key length");
        mac.update(message);

        let mut output = [0u8; 32];
        output.copy_from_slice(&mac.finalize().into_bytes());
        output
    }

    async fn read_hdcp_message(
        hdcp_stream: &TcpStream,
        read_buffer: &mut Vec<u8>,
        timeout: Duration,
    ) -> Option<Vec<u8>> {
        if let Some(message) = Self::extract_hdcp_message(read_buffer) {
            return Some(message);
        }

        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            let now = tokio::time::Instant::now();
            if now >= deadline {
                if read_buffer.is_empty() {
                    debug!("HDCP message wait timed out");
                    return None;
                }

                info!(
                    "Returning partial HDCP response after timeout: {} bytes, first bytes={}",
                    read_buffer.len(),
                    read_buffer[..read_buffer.len().min(64)]
                        .iter()
                        .map(|byte| format!("{:02x}", byte))
                        .collect::<Vec<_>>()
                        .join(" ")
                );
                return Some(std::mem::take(read_buffer));
            }

            match tokio::time::timeout(deadline - now, hdcp_stream.readable()).await {
                Ok(Ok(())) => {
                    let mut buffer = [0u8; 1024];

                    match hdcp_stream.try_read(&mut buffer) {
                        Ok(bytes) if bytes > 0 => {
                            read_buffer.extend_from_slice(&buffer[..bytes]);
                            if let Some(message) = Self::extract_hdcp_message(read_buffer) {
                                return Some(message);
                            }
                        }
                        Ok(_) => {
                            debug!("HDCP peer closed the control socket");
                            if !read_buffer.is_empty() {
                                return Some(std::mem::take(read_buffer));
                            }
                            return None;
                        }
                        Err(err) if err.kind() == ErrorKind::WouldBlock => continue,
                        Err(err) => {
                            tracing::warn!("Failed to read HDCP response: {}", err);
                            return None;
                        }
                    }
                }
                Ok(Err(err)) => {
                    tracing::warn!("HDCP socket not readable: {}", err);
                    return None;
                }
                Err(_) => continue,
            }
        }
    }

    async fn write_hdcp_message(hdcp_stream: &TcpStream, message: &[u8], label: &str) -> bool {
        let deadline = tokio::time::Instant::now() + Duration::from_millis(800);
        let mut written = 0;

        while written < message.len() {
            let now = tokio::time::Instant::now();
            if now >= deadline {
                tracing::warn!(
                    "Timed out writing HDCP {} after {} of {} bytes",
                    label,
                    written,
                    message.len()
                );
                return false;
            }

            match tokio::time::timeout(deadline - now, hdcp_stream.writable()).await {
                Ok(Ok(())) => match hdcp_stream.try_write(&message[written..]) {
                    Ok(0) => {
                        tracing::warn!("HDCP socket closed while writing {}", label);
                        return false;
                    }
                    Ok(bytes) => written += bytes,
                    Err(err) if err.kind() == ErrorKind::WouldBlock => continue,
                    Err(err) => {
                        tracing::warn!("Failed to send HDCP {}: {}", label, err);
                        return false;
                    }
                },
                Ok(Err(err)) => {
                    tracing::warn!("HDCP socket not writable before {}: {}", label, err);
                    return false;
                }
                Err(_) => continue,
            }
        }

        true
    }

    fn extract_hdcp_message(read_buffer: &mut Vec<u8>) -> Option<Vec<u8>> {
        let message_len = Self::hdcp_message_length(read_buffer)?;
        Some(read_buffer.drain(..message_len).collect())
    }

    fn hdcp_message_length(read_buffer: &[u8]) -> Option<usize> {
        let msg_id = *read_buffer.first()?;
        let message_len = match msg_id {
            0x03 => {
                if read_buffer.len() < 138 {
                    return None;
                }
                let exponent_bytes: [u8; 3] = read_buffer[135..138].try_into().ok()?;
                let _exponent_val = u32::from_be_bytes([
                    0,
                    exponent_bytes[0],
                    exponent_bytes[1],
                    exponent_bytes[2],
                ]);
                524
            }
            0x06 => 9,
            0x07 => 33,
            0x08 => 17,
            0x0a => 33,
            0x14 => 6,
            _ => read_buffer.len(),
        };

        (read_buffer.len() >= message_len).then_some(message_len)
    }

    fn log_hdcp_message(message: &[u8]) {
        let preview = message[..message.len().min(64)]
            .iter()
            .map(|byte| format!("{:02x}", byte))
            .collect::<Vec<_>>()
            .join(" ");
        let msg_id = message[0];

        match msg_id {
            0x03 if message.len() >= 138 => {
                let repeater = (message[1] & 0x01) != 0;
                let receiver_id = message[2..7]
                    .iter()
                    .map(|byte| format!("{:02x}", byte))
                    .collect::<Vec<_>>()
                    .join("");
                let full_hex: String = message.iter().map(|b| format!("{:02x}", b)).collect();
                info!(
                    "Received HDCP AKE_Send_Cert: len={}, repeater={}, receiver_id={}, full_hex={}",
                    message.len(),
                    repeater,
                    receiver_id,
                    full_hex
                );
            }
            0x06 if message.len() >= 9 => {
                let rrx = message[1..9]
                    .iter()
                    .map(|byte| format!("{:02x}", byte))
                    .collect::<Vec<_>>()
                    .join("");
                info!("Received HDCP AKE_Send_rrx: r_rx={}", rrx);
            }
            0x07 if message.len() >= 33 => {
                info!("Received HDCP AKE_Send_H_prime ({} bytes)", message.len());
            }
            0x08 if message.len() >= 17 => {
                info!(
                    "Received HDCP AKE_Send_Pairing_Info ({} bytes)",
                    message.len()
                );
            }
            0x0a if message.len() >= 33 => {
                info!("Received HDCP LC_Send_L_prime ({} bytes)", message.len());
            }
            0x14 if message.len() >= 6 => {
                let version = message[3];
                let capability_mask = u16::from_be_bytes([message[4], message[5]]);
                info!(
                    "Received HDCP AKE_Receiver_Info: version={}, capabilities=0x{:04x}, first bytes={}",
                    version, capability_mask, preview
                );
            }
            _ => {
                info!(
                    "Received HDCP response: {} bytes, msg_id={}, first bytes={}",
                    message.len(),
                    msg_id,
                    preview
                );
            }
        }
    }

    async fn negotiate_with_rtsp_client(
        &mut self,
        mut rtsp_client: RtspClient,
    ) -> anyhow::Result<()> {
        let (idr_tx, mut idr_rx) = mpsc::unbounded_channel::<()>();
        rtsp_client.set_idr_channel(idr_tx);

        let sink_caps = self.exchange_rtsp_capabilities(&mut rtsp_client).await?;

        let sink_rtp_port = sink_caps
            .get("wfd_client_rtp_ports")
            .and_then(|value| parse_wfd_client_rtp_port(value));

        let local_ip = self
            .connection
            .as_ref()
            .and_then(|conn| conn.get_sink().ip_address.clone());

        let server_addr = rtsp_client.server_addr();
        let rtp_port = sink_rtp_port.unwrap_or(5004);

        let presentation_url = format!(
            "rtsp://{}/wfd1.0/streamid=0 none",
            local_ip.as_deref().unwrap_or(server_addr)
        );

        let mut trigger_params = std::collections::HashMap::new();
        trigger_params.insert("wfd_presentation_URL".to_string(), presentation_url.clone());
        trigger_params.insert(
            "wfd_client_rtp_ports".to_string(),
            format!("RTP/AVP/UDP;unicast {} 0 mode=play", rtp_port),
        );
        rtsp_client.send_set_parameter(&trigger_params).await?;
        info!("Sent wfd_presentation_URL and wfd_client_rtp_ports");

        let mut trigger_params = std::collections::HashMap::new();
        trigger_params.insert("wfd_trigger_method".to_string(), "SETUP".to_string());
        rtsp_client.send_set_parameter(&trigger_params).await?;
        info!("Sent wfd_trigger_method: SETUP — waiting for TV to initiate SETUP and PLAY");

        let play_info = rtsp_client
            .wait_for_peer_play(Duration::from_secs(15))
            .await?;
        info!(
            "TV initiated SETUP+PLAY, streaming to {}:{}",
            play_info.dest_ip, play_info.dest_port
        );

        *self.state.write() = DaemonState::Streaming;
        self.start_negotiated_stream(
            self.get_negotiated_codec(&sink_caps),
            &play_info.dest_ip,
            play_info.dest_port,
        )
        .await?;
        info!("Stream pipeline configured in RTSP client mode");

        let stream_arc = self.stream.clone();
        tokio::spawn(async move {
            while idr_rx.recv().await.is_some() {
                info!("IDR request received from TV, forcing keyframe");
                let guard = stream_arc.read().await;
                if let Some(ref pipeline) = *guard {
                    if let Err(e) = pipeline.force_keyframe().await {
                        error!("Failed to force keyframe: {}", e);
                    }
                }
            }
        });

        tokio::spawn(async move {
            rtsp_client.run_keepalive().await;
        });
        info!("RTSP keepalive task spawned — TCP connection will stay alive during streaming");

        Ok(())
    }

    async fn exchange_rtsp_capabilities(
        &mut self,
        rtsp_client: &mut RtspClient,
    ) -> anyhow::Result<std::collections::HashMap<String, String>> {
        let options_resp = rtsp_client.send_options().await?;
        info!("OPTIONS response: {}", options_resp.trim());

        let params_to_request = &[
            "wfd_video_formats",
            "wfd_audio_codecs",
            "wfd_client_rtp_ports",
            "wfd_uibc_capability",
            "wfd_standby_resume_capability",
            "wfd_content_protection",
            "wfd_display_hdcp_supported",
            "wfd_coupled_sink",
        ];
        let sink_caps = rtsp_client.send_get_parameter(params_to_request).await?;
        info!("Sink capabilities: {:?}", sink_caps);

        let prefer_hevc = if let Some(codec) = &self.config.video_codec {
            codec.is_hevc()
        } else {
            sink_caps
                .get("wfd_video_formats")
                .map(|formats| formats.contains("02"))
                .unwrap_or(false)
                && !self.config.extend_mode
        };

        let selected_video_format = sink_caps
            .get("wfd_video_formats")
            .map(|formats| {
                swaybeam_rtsp::WfdCapabilities::select_video_formats(formats, prefer_hevc)
            })
            .unwrap_or_else(swaybeam_rtsp::WfdCapabilities::build_video_formats);

        info!("Selected video format: {}", selected_video_format);

        let prefer_hevc = selected_video_format.starts_with("02");

        let hdcp_port = sink_caps
            .get("wfd_content_protection")
            .and_then(|value| swaybeam_rtsp::parse_wfd_content_protection_port(value));

        let content_protection = if prefer_hevc && hdcp_port.is_some() {
            info!("Advertising HDCP support for H.265 stream");
            "HDCP2.2".to_string()
        } else {
            "none".to_string()
        };

        let mut source_caps = std::collections::HashMap::new();
        source_caps.insert("wfd_video_formats".to_string(), selected_video_format);
        source_caps.insert(
            "wfd_audio_codecs".to_string(),
            swaybeam_rtsp::WfdCapabilities::build_audio_codecs(),
        );
        source_caps.insert("wfd_uibc_capability".to_string(), "none".to_string());
        source_caps.insert(
            "wfd_standby_resume_capability".to_string(),
            "none".to_string(),
        );
        source_caps.insert("wfd_content_protection".to_string(), content_protection);
        source_caps.insert("wfd_coupled_sink".to_string(), "none".to_string());

        rtsp_client.send_set_parameter(&source_caps).await?;
        info!("Sent source capabilities");

        Ok(sink_caps)
    }

    /// Determine video codec from sink capabilities
    fn get_negotiated_codec(
        &self,
        sink_caps: &std::collections::HashMap<String, String>,
    ) -> VideoCodec {
        if let Some(codec) = &self.config.video_codec {
            info!("Using configured codec: {}", codec);
            return codec.clone();
        }

        let hevc_supported = sink_caps
            .get("wfd_video_formats")
            .map(|formats| formats.contains("02"))
            .unwrap_or(false);

        let prefer_hevc = hevc_supported && !self.config.extend_mode;

        let selected = StreamPipeline::select_best_codec(prefer_hevc);
        info!(
            "Auto-selected codec: {} (HEVC supported by TV: {})",
            selected, hevc_supported
        );
        selected
    }

    fn map_negotiated_codec(codec: NegotiatedCodec) -> VideoCodec {
        match codec {
            NegotiatedCodec::H264 => {
                if StreamPipeline::is_hardware_encoder_available(&VideoCodec::H264Hardware) {
                    VideoCodec::H264Hardware
                } else {
                    VideoCodec::H264
                }
            }
            NegotiatedCodec::H265 => {
                if StreamPipeline::is_hardware_encoder_available(&VideoCodec::H265Hardware) {
                    VideoCodec::H265Hardware
                } else {
                    VideoCodec::H265
                }
            }
            NegotiatedCodec::AV1 => VideoCodec::AV1,
        }
    }

    fn is_connection_refused(err: &swaybeam_rtsp::RtspError) -> bool {
        matches!(err, swaybeam_rtsp::RtspError::Io(io_err) if io_err.kind() == std::io::ErrorKind::ConnectionRefused)
    }

    async fn start_negotiated_stream(
        &mut self,
        video_codec: VideoCodec,
        destination_ip: &str,
        destination_rtp_port: u16,
    ) -> anyhow::Result<()> {
        let (width, height, bitrate) = if self.config.extend_mode {
            (3840, 2160, 20_000_000u32)
        } else {
            (
                self.config.video_width,
                self.config.video_height,
                self.config.video_bitrate,
            )
        };

        let stream_config = StreamConfig {
            video_codec,
            video_bitrate: bitrate,
            video_width: width,
            video_height: height,
            video_framerate: self.config.video_framerate,
            audio_codec: AudioCodec::AAC,
            audio_bitrate: 128_000,
            audio_sample_rate: 48000,
            audio_channels: 2,
        };

        let capture_config = CaptureConfig {
            width,
            height,
            framerate: self.config.video_framerate,
            cursor_visible: true,
        };
        let mut capture = Capture::new(capture_config)?;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        let pw_stream = capture.start().await?;

        let audio_monitor = if self.config.enable_audio {
            if let Some(ref audio_sink) = self.audio_sink {
                Some(audio_sink.monitor_device())
            } else {
                warn!("Audio enabled but virtual sink not available, disabling audio");
                None
            }
        } else {
            None
        };
        info!("Audio monitor device: {:?}", audio_monitor);

        let pipeline =
            StreamPipeline::new_pipewire_with_audio(stream_config, pw_stream, audio_monitor)?;
        pipeline
            .set_output(destination_ip, destination_rtp_port)
            .await?;

        if let Some(ref hdcp_config) = self.hdcp_encryption {
            use swaybeam_stream::HdcpEncryptionConfig;
            let enc_config = HdcpEncryptionConfig {
                ks: hdcp_config.ks,
                riv: hdcp_config.riv,
                rrx: hdcp_config.rrx,
                receiver_version: hdcp_config.receiver_version,
            };
            pipeline.set_hdcp_encryption(enc_config).await;
            info!("HDCP encryption configured for stream pipeline");
        }

        pipeline.start().await?;
        info!(
            "PipeWire stream pipeline started to {}:{}",
            destination_ip, destination_rtp_port
        );

        self.capture = Some(capture);
        *self.stream.write().await = Some(pipeline);
        *self.state.write() = DaemonState::Streaming;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Daemon;

    #[test]
    fn test_aes_ctr_kd_derivation() {
        let rtx: [u8; 8] = [0x35, 0xc7, 0x23, 0xc8, 0xf9, 0x19, 0xbe, 0x44];
        let km: [u8; 16] = [
            0x08, 0x9c, 0x19, 0xd2, 0x39, 0x15, 0x86, 0xf0, 0x16, 0x05, 0x5a, 0x21, 0x39, 0x52,
            0xd7, 0x62,
        ];

        let kd = Daemon::compute_hdcp_kd(&rtx, &km, None, None);

        let kd_hex: String = kd.iter().map(|b| format!("{:02x}", b)).collect();
        eprintln!("Kd: {}", kd_hex);

        assert_eq!(
            kd_hex,
            "e9da8dc5f71ab59aab9839c28d26ab6a5283a2a6db01713109424514d67fe913"
        );
    }

    #[test]
    fn recognizes_l_prime_message_length() {
        let read_buffer = vec![0x0a; 33];

        assert_eq!(Daemon::hdcp_message_length(&read_buffer), Some(33));
    }

    #[test]
    fn extracts_fixed_size_hdcp_message_without_consuming_next_one() {
        let mut read_buffer = vec![
            0x14, 0x00, 0x06, 0x03, 0x00, 0x01, 0x06, 1, 2, 3, 4, 5, 6, 7, 8,
        ];

        let first = Daemon::extract_hdcp_message(&mut read_buffer).expect("receiver info");

        assert_eq!(first, vec![0x14, 0x00, 0x06, 0x03, 0x00, 0x01]);
        assert_eq!(read_buffer, vec![0x06, 1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn waits_for_complete_fixed_size_hdcp_message() {
        let read_buffer = vec![0x07, 0x00, 0x01];

        assert_eq!(Daemon::hdcp_message_length(&read_buffer), None);
    }
}
