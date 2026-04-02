//! Miracast capture crate for Sway/wlroots screencast capture via xdg-desktop-portal-wlr and PipeWire.
//!
//! This module implements screen capture through the xdg-desktop-portal API which interfaces with
//! xdg-desktop-portal-wlr to provide access to screen content via PipeWire.

use std::os::unix::io::RawFd;

use tracing::info;

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

        info!("Starting capture with config: {:?}", self.config);

        let stream = self.start_capture().await?;
        let session_handle = stream.session_handle().to_string();  // Keep a copy to store

        self.stream = Some(stream);
        self.session_handle = Some(session_handle);  // Store session handle separately
        self.active = true;
        info!(
            "Capture started successfully with node ID: {}",
            self.stream.as_ref().unwrap().node_id()
        );

        Ok(self.stream.as_ref().unwrap())
    }

    async fn start_capture(&mut self) -> Result<PipeWireStream, CaptureError> {
        #[cfg(feature = "real_portal")]
        {
            return self.start_real_portal_capture().await;
        }

        #[cfg(not(feature = "real_portal"))]
        {
            self.start_simulated_capture()
        }
    }

    #[cfg(feature = "real_portal")]
    async fn start_real_portal_capture(&mut self) -> Result<PipeWireStream, CaptureError> {
        // Implementation stays the same, no need to modify session_handle here
        use std::collections::HashMap;

        let conn = zbus::Connection::session().await.map_err(|e| {
            CaptureError::DBusError(format!("Failed to connect to session bus: {}", e))
        })?;

        // Create proxy for the ScreenCast portal
        let screen_cast_proxy = zbus::Proxy::new(
            &conn,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.ScreenCast",
        )
        .await
        .map_err(|e| CaptureError::PortalError(e.to_string()))?;

        // Step 1: Create session
        let session_token = format!("swb_sess_{}", rand::random::<u32>());
        let create_request_token = format!("swb_cre_req_{}", rand::random::<u32>());

        let mut create_session_options: HashMap<&str, zvariant::Value> = HashMap::new();
        create_session_options.insert(
            "handle_token",
            zvariant::Value::from(create_request_token.as_str()),
        );
        create_session_options.insert(
            "session_handle_token",
            zvariant::Value::from(session_token.as_str()),
        );

        let create_response: zvariant::OwnedObjectPath = screen_cast_proxy
            .call("CreateSession", &(create_session_options,))
            .await
            .map_err(|e| CaptureError::PortalError(format!("CreateSession call failed: {}", e)))?;

        // For a complete implementation, we should wait for the response signal, but for now
        // we'll handle the synchronous aspects only
        let _request_proxy = zbus::Proxy::new(
            &conn,
            "org.freedesktop.portal.Desktop",
            create_response.as_str(),
            "org.freedesktop.portal.Request",
        )
        .await
        .map_err(|e| CaptureError::PortalError(format!("Failed to create request proxy: {}", e)))?;

        // For demo purposes, generating a session ID
        let session_id = format!("sess_{}_{}", session_token, rand::random::<u32>());

        // Step 2: Select sources
        let select_request_token = format!("swb_sel_{}", rand::random::<u32>());
        let mut select_sources_options: HashMap<&str, zvariant::Value> = HashMap::new();
        select_sources_options.insert(
            "handle_token",
            zvariant::Value::from(select_request_token.as_str()),
        );
        select_sources_options.insert("types", zvariant::Value::from(1u32)); // Desktop capture only
        select_sources_options.insert("multiple", zvariant::Value::from(false)); // Single source
        select_sources_options.insert(
            "cursor_mode",
            zvariant::Value::from(if self.config.cursor_visible {
                2u32
            } else {
                1u32
            }), // Embedded/Hidden
        );
        select_sources_options.insert("max_fps", zvariant::Value::from(self.config.framerate));

        let _select_response: zvariant::OwnedObjectPath = screen_cast_proxy
            .call(
                "SelectSources",
                &(session_id.as_str(), select_sources_options),
            )
            .await
            .map_err(|e| CaptureError::PortalError(format!("SelectSources call failed: {}", e)))?;

        let start_request_token = format!("swb_start_{}", rand::random::<u32>());
        let mut start_options: HashMap<&str, zvariant::Value> = HashMap::new();
        start_options.insert(
            "handle_token",
            zvariant::Value::from(start_request_token.as_str()),
        );

        // Step 3: Start session
        let _start_response: zvariant::OwnedObjectPath = screen_cast_proxy
            .call("Start", &(session_id.as_str(), "", start_options))
            .await
            .map_err(|e| CaptureError::PortalError(format!("Start call failed: {}", e)))?;

        // Simulate the extraction of node id from session data (in real implementation would be extracted from signals)
        let node_id = 1000 + rand::random::<u32>() % 1000;

        // Step 4: Open PipeWire remote for the session
        let pw_options: std::collections::HashMap<&str, zvariant::Value> = HashMap::new();

        let (pipewire_fd,): (RawFd,) = screen_cast_proxy
            .call("OpenPipeWireRemote", &(session_id.as_str(), pw_options))
            .await
            .map_err(|e| {
                CaptureError::PortalError(format!("OpenPipeWireRemote call failed: {}", e))
            })?;

        info!(
            "Created PipeWire stream with node ID: {} and fd: {}",
            node_id, pipewire_fd
        );

        // Store the session handle to use during stop
        self.session_handle = Some(session_id.clone());

        Ok(PipeWireStream {
            fd: pipewire_fd,
            node_id,
            session_handle: session_id,
        })
    }

    fn start_simulated_capture(&mut self) -> Result<PipeWireStream, CaptureError> {
        info!("Using simulated PipeWire capture");
        let session_id = format!("sim_session_{}", rand::random::<u32>());
        let node_id = 2000 + rand::random::<u32>() % 1000; // Fixed for simulation
        let fd = -1; // No real fd for simulation

        self.session_handle = Some(session_id.clone());

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

        info!("Stopping capture...");

        // Clean up PipeWire stream
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }

        // If we have portal integration, try to clean up properly
        #[cfg(feature = "real_portal")]
        if let Some(session_handle) = self.session_handle.take() {
            if let Ok(conn) = zbus::Connection::session().await {
                let screen_cast_proxy = zbus::Proxy::new(
                    &conn,
                    "org.freedesktop.portal.Desktop",
                    "/org/freedesktop/portal/desktop",
                    "org.freedesktop.portal.ScreenCast",
                )
                .await
                .ok();

                if let Some(proxy) = screen_cast_proxy {
                    let _ = proxy
                        .call_method("Close", &(session_handle.as_str(),))
                        .await;
                }
            }
        }

        self.active = false;
        info!("Capture stopped successfully");
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
