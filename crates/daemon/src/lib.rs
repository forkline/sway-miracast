use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use miracast_capture::{Capture, CaptureConfig};
use miracast_doctor::{check_all, Report as DoctorReport};
use miracast_net::{NetError, P2pConfig, P2pConnection, P2pManager, Sink};
use miracast_rtsp::RtspServer;
use miracast_stream::{AudioCodec, StreamConfig, StreamPipeline, VideoCodec};

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
            group_name: "miracast".to_string(),
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
            group_name: "miracast".to_string(),
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

        let rtsp_addr = "0.0.0.0:7236";
        self.rtsp_server = Some(RtspServer::new(rtsp_addr.to_string()));
        *self.state.write() = DaemonState::Streaming;

        info!("RTSP negotiation completed");
        self.event_tx.send(DaemonEvent::Negotiated).ok();

        Ok(())
    }

    pub async fn start_stream(&mut self) -> anyhow::Result<()> {
        if self.get_state() != DaemonState::Streaming {
            return Err(anyhow::anyhow!("Daemon must be in Streaming state"));
        }

        let capture_config = CaptureConfig {
            width: self.config.video_width,
            height: self.config.video_height,
            framerate: self.config.video_framerate,
            cursor_visible: true,
        };

        let mut capture = Capture::new(capture_config)?;
        capture.start().await?;
        self.capture = Some(capture);

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
            } else {
                return Err(anyhow::anyhow!("Sink has no IP address"));
            }
        } else {
            return Err(anyhow::anyhow!("No active connection"));
        }

        self.stream = Some(pipeline);
        info!("Stream pipeline configured");
        self.event_tx.send(DaemonEvent::StreamingStarted).ok();

        Ok(())
    }

    pub async fn stop_stream(&mut self) -> anyhow::Result<()> {
        if let Some(mut capture) = self.capture.take() {
            capture.stop().await?;
            info!("Capture stopped");
        }

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
                group_name: "miracast".to_string(),
            };

            let manager = P2pManager::new(config).await?;
            manager.disconnect().await?;
            info!("Disconnected from {}", conn.get_sink().name);
        }

        *self.state.write() = DaemonState::Idle;
        Ok(())
    }

    pub async fn run_session(&mut self) -> anyhow::Result<()> {
        self.run_doctor_checks().await?;
        debug!("Doctor checks completed");

        let sinks = self.discover().await?;
        debug!("Discovered {} sink(s)", sinks.len());
        self.event_tx
            .send(DaemonEvent::Discovered(sinks.clone()))
            .ok();

        if sinks.is_empty() {
            return Err(anyhow::anyhow!("No Miracast sinks discovered"));
        }

        let sink = sinks[0].clone();
        self.connect(sink).await?;

        self.negotiate().await?;
        debug!("Negotiation completed");

        self.start_stream().await?;
        debug!("Stream started");

        Ok(())
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        info!("Daemon starting...");
        self.event_tx.send(DaemonEvent::Started).ok();

        if let Err(e) = self.run_session().await {
            error!("Session failed: {}", e);
            *self.state.write() = DaemonState::Error;
            self.event_tx
                .send(DaemonEvent::ErrorOccurred(e.to_string()))
                .ok();
            return Err(e);
        }

        let _ = self.stop_stream().await;
        let _ = self.disconnect().await;

        info!("Daemon shutting down...");
        self.event_tx.send(DaemonEvent::Ended).ok();

        Ok(())
    }

    pub async fn graceful_shutdown(&mut self) -> anyhow::Result<()> {
        info!("Shutting down daemon gracefully...");

        let _ = self.stop_stream().await;
        if self.connection.is_some() {
            let _ = self.disconnect().await;
        }

        *self.state.write() = DaemonState::Idle;
        info!("Daemon shutdown completed");
        Ok(())
    }
}

impl Default for Daemon {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_daemon_creation() {
        let daemon = Daemon::new();
        assert_eq!(daemon.get_state(), DaemonState::Idle);
    }

    #[tokio::test]
    async fn test_daemon_with_config() {
        let config = DaemonConfig {
            video_width: 1280,
            video_height: 720,
            video_framerate: 60,
            video_bitrate: 6_000_000,
            discovery_timeout: Duration::from_secs(5),
            interface: "wlan1".to_string(),
        };

        let daemon = Daemon::with_config(config);
        assert_eq!(daemon.get_state(), DaemonState::Idle);
    }

    #[tokio::test]
    async fn test_daemon_state_transitions() {
        let daemon = Daemon::new();
        assert_eq!(daemon.get_state(), DaemonState::Idle);

        *daemon.state.write() = DaemonState::Discovering;
        assert_eq!(daemon.get_state(), DaemonState::Discovering);

        *daemon.state.write() = DaemonState::Idle;
        assert_eq!(daemon.get_state(), DaemonState::Idle);
    }

    #[test]
    fn test_daemon_config_default() {
        let config = DaemonConfig::default();
        assert_eq!(config.video_width, 1920);
        assert_eq!(config.video_height, 1080);
        assert_eq!(config.video_framerate, 30);
        assert_eq!(config.video_bitrate, 8_000_000);
        assert_eq!(config.discovery_timeout, Duration::from_secs(10));
        assert_eq!(config.interface, "wlan0");
    }

    #[tokio::test]
    async fn test_daemon_event_subscription() {
        let mut daemon = Daemon::new();
        let _events_rx = daemon.subscribe_events();
        assert_eq!(daemon.get_state(), DaemonState::Idle);
    }
}
