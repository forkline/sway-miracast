use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

// WFD Capabilities representing Wi-Fi Display capabilities
#[derive(Debug, Clone)]
pub struct WfdCapabilities {
    pub client_rtp_ports: Option<String>,
    pub video_formats: Option<String>,
    pub audio_codecs: Option<String>,
    pub display_edid: Option<String>,
    pub coupled_sink: Option<String>,
    pub uibc_capability: Option<String>,
    pub standby_resume_capability: Option<String>,
    pub content_protection: Option<String>,
}

impl WfdCapabilities {
    pub fn new() -> Self {
        WfdCapabilities {
            client_rtp_ports: None,
            video_formats: None,
            audio_codecs: None,
            display_edid: None,
            coupled_sink: None,
            uibc_capability: None,
            standby_resume_capability: None,
            content_protection: None,
        }
    }
}

impl Default for WfdCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

pub fn set_parameter(&mut self, param_name: &str, value: &str) -> Result<(), RtspError> {
    match param_name {
        "wfd_client_rtp_ports" => self.client_rtp_ports = Some(value.to_string()),
        "wfd_video_formats" => self.video_formats = Some(value.to_string()),
        "wfd_audio_codecs" => self.audio_codecs = Some(value.to_string()),
        "wfd_display_edid" => self.display_edid = Some(value.to_string()),
        "wfd_coupled_sink" => self.coupled_sink = Some(value.to_string()),
        "wfd_uibc_capability" => self.uibc_capability = Some(value.to_string()),
        "wfd_standby_resume_capability" => self.standby_resume_capability = Some(value.to_string()),
        "wfd_content_protection" => self.content_protection = Some(value.to_string()),
        _ => return Err(RtspError::InvalidParameter(param_name.to_string())),
    }
    Ok(())
}
