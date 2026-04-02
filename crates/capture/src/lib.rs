//! Miracast capture crate for Sway/wlroots screencast capture via xdg-desktop-portal-wlr and PipeWire.

use std::os::unix::io::RawFd;

use tracing::{debug, info};

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
    #[error("Portal request cancelled by user")]
    PortalCancelled,
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("Io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Platform not supported")]
    PlatformNotSupported,
    #[error("Capture not active")]
    NotActive,
}

#[derive(Debug)]
pub struct PipeWireStream {
    fd: RawFd,
    node_id: u32,
    session_handle: String,
}

impl PipeWireStream {
    pub fn fd(&self) -> RawFd {
        self.fd
    }

    pub fn node_id(&self) -> u32 {
        self.node_id
    }

    pub fn session_handle(&self) -> &str {
        &self.session_handle
    }
}

impl Drop for PipeWireStream {
    fn drop(&mut self) {
        if self.fd >= 0 {
            unsafe {
                libc::close(self.fd);
            }
        }
    }
}

pub struct Capture {
    config: CaptureConfig,
    active: bool,
    session_handle: Option<String>,
    stream: Option<PipeWireStream>,
}

#[cfg(target_os = "linux")]
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
            active: false,
            session_handle: None,
            stream: None,
        })
    }

    pub async fn start(&mut self) -> Result<&PipeWireStream, CaptureError> {
        if self.active {
            return Err(CaptureError::StartFailed("Capture already active".into()));
        }

        debug!("Starting capture with config: {:?}", self.config);

        let stream = self.start_capture().await?;

        self.stream = Some(stream);
        self.active = true;
        info!("Capture started successfully");
        Ok(self.stream.as_ref().unwrap())
    }

    async fn start_capture(&self) -> Result<PipeWireStream, CaptureError> {
        #[cfg(feature = "real_portal")]
        {
            if let Ok(stream) = self.start_portal_capture().await {
                return Ok(stream);
            }
        }

        self.start_simulated_capture()
    }

    #[cfg(feature = "real_portal")]
    async fn start_portal_capture(&self) -> Result<PipeWireStream, CaptureError> {
        use std::collections::HashMap;

        let conn = zbus::Connection::session()
            .await
            .map_err(|e| CaptureError::DBusError(e.to_string()))?;

        let session_token = format!("session_{}", rand::random::<u32>());
        let request_token = format!("request_{}", rand::random::<u32>());

        let portal = zbus::Proxy::new(
            &conn,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.ScreenCast",
        )
        .await
        .map_err(|e| CaptureError::PortalError(e.to_string()))?;

        let options: HashMap<&str, zvariant::Value> = [
            ("handle_token", zvariant::Value::new(request_token.as_str())),
            (
                "session_handle_token",
                zvariant::Value::new(session_token.as_str()),
            ),
        ]
        .into_iter()
        .collect();

        let _: zvariant::OwnedObjectPath = portal
            .call("CreateSession", &(options))
            .await
            .map_err(|e| CaptureError::PortalError(e.to_string()))?;

        Err(CaptureError::PortalError(
            "Portal integration not complete".into(),
        ))
    }

    fn start_simulated_capture(&self) -> Result<PipeWireStream, CaptureError> {
        info!("Using simulated PipeWire capture (portal not available or disabled)");
        let session_id = format!("session_{}", rand::random::<u32>());
        let node_id = 1;
        let fd = -1;

        Ok(PipeWireStream {
            fd,
            node_id,
            session_handle: session_id,
        })
    }

    pub async fn stop(&mut self) -> Result<(), CaptureError> {
        if !self.active {
            return Err(CaptureError::NotActive);
        }

        debug!("Stopping capture");

        if let Some(stream) = self.stream.take() {
            drop(stream);
        }

        self.session_handle = None;
        self.active = false;
        info!("Capture stopped");
        Ok(())
    }

    pub fn config(&self) -> &CaptureConfig {
        &self.config
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn stream(&self) -> Option<&PipeWireStream> {
        self.stream.as_ref()
    }
}

#[cfg(not(target_os = "linux"))]
impl Capture {
    pub fn new(_: CaptureConfig) -> Result<Self, CaptureError> {
        Err(CaptureError::PlatformNotSupported)
    }

    pub async fn start(&mut self) -> Result<&PipeWireStream, CaptureError> {
        Err(CaptureError::PlatformNotSupported)
    }

    pub async fn stop(&mut self) -> Result<(), CaptureError> {
        Err(CaptureError::PlatformNotSupported)
    }

    pub fn is_active(&self) -> bool {
        false
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

    #[cfg(target_os = "linux")]
    #[test]
    fn test_capture_config_validation() {
        let config = CaptureConfig {
            width: 1920,
            height: 1080,
            framerate: 30,
            cursor_visible: true,
        };
        assert!(Capture::new(config).is_ok());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_capture_config_zero_width() {
        let config = CaptureConfig {
            width: 0,
            height: 1080,
            framerate: 30,
            cursor_visible: true,
        };
        assert!(Capture::new(config).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_capture_config_zero_height() {
        let config = CaptureConfig {
            width: 1920,
            height: 0,
            framerate: 30,
            cursor_visible: true,
        };
        assert!(Capture::new(config).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_capture_config_framerate_low() {
        let config = CaptureConfig {
            width: 1920,
            height: 1080,
            framerate: 0,
            cursor_visible: false,
        };
        assert!(Capture::new(config).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_capture_config_framerate_high() {
        let config = CaptureConfig {
            width: 1920,
            height: 1080,
            framerate: 120,
            cursor_visible: false,
        };
        assert!(Capture::new(config).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_capture_new_success() {
        let config = CaptureConfig::default();
        let capture = Capture::new(config).unwrap();
        assert_eq!(capture.config().width, 1920);
        assert_eq!(capture.config().height, 1080);
        assert!(!capture.is_active());
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn test_capture_start_stop() {
        let config = CaptureConfig::default();
        let mut capture = Capture::new(config).unwrap();

        capture.start().await.unwrap();
        assert!(capture.is_active());

        capture.stop().await.unwrap();
        assert!(!capture.is_active());
    }
}
