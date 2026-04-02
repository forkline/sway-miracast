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
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("Io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Platform not supported")]
    PlatformNotSupported,
}

#[derive(Debug)]
pub struct PipeWireStream {
    fd: RawFd,
    session_id: String,
    pipewire_node_id: u32,
    #[cfg(target_os = "linux")]
    #[allow(dead_code)]
    stream_handle: Option<pipewire_handle::StreamHandle>,
}

#[cfg(target_os = "linux")]
mod pipewire_handle {
    use std::os::unix::io::RawFd;

    // Mock placeholder for PipeWire stream
    #[derive(Debug)]
    pub struct StreamHandle {
        _fd: RawFd,
        _node_id: u32,
    }

    impl Drop for StreamHandle {
        fn drop(&mut self) {
            // Cleanup logic here
        }
    }

    pub fn create_stream(node_id: u32, fd: RawFd) -> Result<StreamHandle, crate::CaptureError> {
        Ok(StreamHandle {
            _fd: fd,
            _node_id: node_id,
        })
    }
}

#[cfg(not(target_os = "linux"))]
mod pipewire_handle {
    use std::os::unix::io::RawFd;
    #[derive(Debug)]
    pub struct StreamHandle {/* dummy */}

    pub fn create_stream(_node_id: u32, _fd: RawFd) -> Result<StreamHandle, crate::CaptureError> {
        Err(crate::CaptureError::PlatformNotSupported)
    }
}

impl PipeWireStream {
    pub fn fd(&self) -> RawFd {
        self.fd
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn pipewire_node_id(&self) -> u32 {
        self.pipewire_node_id
    }
}

pub struct Capture {
    config: CaptureConfig,
    active: bool,
    #[cfg(target_os = "linux")]
    session_handle: std::cell::Cell<Option<String>>,
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
            session_handle: std::cell::Cell::new(None),
        })
    }

    pub async fn start(&mut self) -> Result<PipeWireStream, CaptureError> {
        debug!("Starting capture with config: {:?}", self.config);

        #[cfg(target_os = "linux")]
        {
            // On Linux, we would use the actual portal implementation
            // For now, simulating
            let session_id = format!("session_{}", rand::random::<u32>());
            let node_id = 1; // Simulated PipeWire node ID
            let fake_fd = -1_i32 as RawFd; // Simulated RawFd (invalid fd for this example)

            let stream_handle = pipewire_handle::create_stream(node_id, fake_fd)?;

            self.active = true;
            info!(
                "Capture started successfully with simulated PipeWire node ID: {}",
                node_id
            );

            Ok(PipeWireStream {
                fd: fake_fd,
                session_id,
                pipewire_node_id: node_id,
                stream_handle: Some(stream_handle),
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(CaptureError::PlatformNotSupported)
        }
    }

    pub async fn stop(&mut self) -> Result<(), CaptureError> {
        debug!("Stopping capture");
        self.active = false;

        #[cfg(target_os = "linux")]
        self.session_handle.set(None);

        info!("Capture stopped");
        Ok(())
    }

    pub fn config(&self) -> &CaptureConfig {
        &self.config
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}

// Non-Linux implementation - just returns not supported
#[cfg(not(target_os = "linux"))]
impl Capture {
    pub fn new(_: CaptureConfig) -> Result<Self, CaptureError> {
        Err(CaptureError::PlatformNotSupported)
    }

    pub async fn start(&mut self) -> Result<PipeWireStream, CaptureError> {
        Err(CaptureError::PlatformNotSupported)
    }

    pub async fn stop(&mut self) -> Result<(), CaptureError> {
        Err(CaptureError::PlatformNotSupported)
    }

    pub fn config(&self) -> &CaptureConfig {
        panic!("Only available on supported platforms")
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

    #[test]
    fn test_capture_config_validation() {
        let config = CaptureConfig {
            width: 1920,
            height: 1080,
            framerate: 30,
            cursor_visible: true,
        };
        #[cfg(target_os = "linux")]
        {
            assert!(Capture::new(config).is_ok());
        }
        #[cfg(not(target_os = "linux"))]
        {
            // On non-linux platforms, it will return PlatformNotSupported
        }
    }

    #[test]
    fn test_capture_config_zero_width() {
        let config = CaptureConfig {
            width: 0,
            height: 1080,
            framerate: 30,
            cursor_visible: true,
        };
        #[cfg(target_os = "linux")]
        {
            assert!(Capture::new(config).is_err());
        }
    }

    #[test]
    fn test_capture_config_zero_height() {
        let config = CaptureConfig {
            width: 1920,
            height: 0,
            framerate: 30,
            cursor_visible: true,
        };
        #[cfg(target_os = "linux")]
        {
            assert!(Capture::new(config).is_err());
        }
    }

    #[test]
    fn test_capture_config_framerate_low() {
        let config = CaptureConfig {
            width: 1920,
            height: 1080,
            framerate: 0,
            cursor_visible: false,
        };
        #[cfg(target_os = "linux")]
        {
            assert!(Capture::new(config).is_err());
        }
    }

    #[test]
    fn test_capture_config_framerate_high() {
        let config = CaptureConfig {
            width: 1920,
            height: 1080,
            framerate: 120,
            cursor_visible: false,
        };
        #[cfg(target_os = "linux")]
        {
            assert!(Capture::new(config).is_err());
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_capture_new_success() {
        let config = CaptureConfig::default();
        let capture = Capture::new(config).unwrap();
        assert_eq!(capture.config().width, 1920);
        assert_eq!(capture.config().height, 1080);
    }

    #[cfg(target_os = "linux")]
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

    #[cfg(all(target_os = "linux", feature = "mock"))]
    #[tokio::test]
    async fn test_capture_start_stop() {
        let config = CaptureConfig::default();
        let mut capture = Capture::new(config).unwrap();

        let stream = capture.start().await.unwrap();
        assert_eq!(stream.pipewire_node_id(), 1); // mocked id
        assert!(!stream.session_id().is_empty());
        assert!(capture.is_active());

        capture.stop().await.unwrap();
        assert!(!capture.is_active());
    }
}
