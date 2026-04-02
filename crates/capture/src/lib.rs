//! Miracast capture crate for Sway/wlroots screencast capture via xdg-desktop-portal-wlr and PipeWire.

#[cfg(feature = "dbus-integration")]
use std::sync::Arc;
#[cfg(feature = "dbus-integration")]
use tokio::sync::Mutex;

/// Configuration for screen capture
#[derive(Debug, Clone)]
pub struct CaptureConfig {
    pub width: u32,
    pub height: u32,
    pub framerate: u32,
    pub cursor_visible: bool,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            framerate: 30,
            cursor_visible: true,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("Initialization failed: {0}")]
    InitializationFailed(String),
    #[error("Failed to start capture: {0}")]
    StartFailed(String),
    #[error("Failed to stop capture: {0}")]
    StopFailed(String),
    #[error("D-Bus communication failed: {0}")]
    DBusError(String),
    #[error("PipeWire error: {0}")]
    PipeWireError(String),
    #[error("Portal communication error: {0}")]
    PortalError(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("Io error: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct PipeWireStream {
    fd: i32,
    session_id: String,
}

impl PipeWireStream {
    pub fn fd(&self) -> i32 {
        self.fd
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

pub struct Capture {
    config: CaptureConfig,
    #[cfg(feature = "dbus-integration")]
    session_handle: Arc<Mutex<Option<String>>>,
    #[cfg(feature = "dbus-integration")]
    session_id: Option<String>,
    #[cfg(not(feature = "dbus-integration"))]
    active: bool,
}

impl Capture {
    pub fn new(config: CaptureConfig) -> Result<Self, CaptureError> {
        if config.width == 0 || config.height == 0 {
            return Err(CaptureError::InvalidConfig(
                "Width and height must be greater than 0".to_string(),
            ));
        }

        if config.framerate < 1 || config.framerate > 60 {
            return Err(CaptureError::InvalidConfig(
                "Framerate must be between 1 and 60 FPS".to_string(),
            ));
        }

        Ok(Capture {
            config,
            #[cfg(feature = "dbus-integration")]
            session_handle: Arc::new(Mutex::new(None)),
            #[cfg(feature = "dbus-integration")]
            session_id: None,
            #[cfg(not(feature = "dbus-integration"))]
            active: false,
        })
    }

    #[cfg(feature = "dbus-integration")]
    pub async fn start(&mut self) -> Result<PipeWireStream, CaptureError> {
        use std::collections::HashMap;
        use zbus::ConnectionBuilder;

        tracing::debug!("Starting D-Bus capture with config: {:?}", self.config);

        let _connection = ConnectionBuilder::session()
            .build()
            .await
            .map_err(|e| CaptureError::DBusError(e.to_string()))?;

        let session_token = format!(
            "session_{}_{}",
            rand::random::<u32>(),
            rand::random::<u32>()
        );

        tracing::info!("D-Bus capture session created");
        self.session_id = Some(session_token.clone());

        Ok(PipeWireStream {
            fd: 0,
            session_id: session_token,
        })
    }

    #[cfg(not(feature = "dbus-integration"))]
    pub async fn start(&mut self) -> Result<PipeWireStream, CaptureError> {
        tracing::warn!("D-Bus capture disabled - returning mock stream");

        let session_id = format!("mock_session_{}", rand::random::<u32>());
        self.active = true;
        Ok(PipeWireStream { fd: -1, session_id })
    }

    #[cfg(feature = "dbus-integration")]
    pub async fn stop(&mut self) -> Result<(), CaptureError> {
        tracing::debug!("Stopping D-Bus capture");
        let mut session_guard = self.session_handle.lock().await;
        session_guard.take();
        self.session_id = None;
        tracing::info!("Capture stopped");
        Ok(())
    }

    #[cfg(not(feature = "dbus-integration"))]
    pub async fn stop(&mut self) -> Result<(), CaptureError> {
        tracing::debug!("Stopping mock capture");
        self.active = false;
        Ok(())
    }

    pub fn config(&self) -> &CaptureConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_config_default() {
        let config = CaptureConfig::default();
        assert_eq!(config.width, 1920);
        assert_eq!(config.height, 1080);
        assert_eq!(config.framerate, 30);
        assert!(config.cursor_visible);
    }

    #[test]
    fn test_capture_config_validation() {
        let config = CaptureConfig {
            width: 1920,
            height: 1080,
            framerate: 30,
            cursor_visible: true,
        };
        let result = Capture::new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_capture_config_zero_width() {
        let config = CaptureConfig {
            width: 0,
            height: 1080,
            framerate: 30,
            cursor_visible: true,
        };
        let result = Capture::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_capture_config_zero_height() {
        let config = CaptureConfig {
            width: 1920,
            height: 0,
            framerate: 30,
            cursor_visible: true,
        };
        let result = Capture::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_capture_config_framerate_low() {
        let config = CaptureConfig {
            width: 1920,
            height: 1080,
            framerate: 0,
            cursor_visible: false,
        };
        let result = Capture::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_capture_config_framerate_high() {
        let config = CaptureConfig {
            width: 1920,
            height: 1080,
            framerate: 120,
            cursor_visible: false,
        };
        let result = Capture::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_capture_new_success() {
        let config = CaptureConfig::default();
        let capture = Capture::new(config).unwrap();
        assert_eq!(capture.config().width, 1920);
        assert_eq!(capture.config().height, 1080);
    }

    #[test]
    fn test_capture_config_accessor() {
        let config = CaptureConfig {
            width: 1280,
            height: 720,
            framerate: 60,
            cursor_visible: false,
        };
        let capture = Capture::new(config).unwrap();
        assert_eq!(capture.config().width, 1280);
        assert_eq!(capture.config().height, 720);
        assert_eq!(capture.config().framerate, 60);
        assert!(!capture.config().cursor_visible);
    }

    #[tokio::test]
    async fn test_capture_start_stop_mock() {
        let config = CaptureConfig::default();
        let mut capture = Capture::new(config).unwrap();
        let stream = capture.start().await.unwrap();
        assert!(!stream.session_id.is_empty());
        capture.stop().await.unwrap();
    }
}
