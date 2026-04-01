use std::fmt;
use std::os::unix::io::RawFd;

/// Possible video codecs supported by the stream
#[derive(Debug, Clone, PartialEq)]
pub enum VideoCodec {
    /// H.264 codec, primary for Miracast
    H264,
}

impl fmt::Display for VideoCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VideoCodec::H264 => write!(f, "H264"),
        }
    }
}

/// Possible audio codecs supported by the stream
#[derive(Debug, Clone, PartialEq)]
pub enum AudioCodec {
    /// Advanced Audio Coding
    AAC,
    /// Linear Pulse Code Modulation
    LPCM,
}

impl fmt::Display for AudioCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioCodec::AAC => write!(f, "AAC"),
            AudioCodec::LPCM => write!(f, "LPCM"),
        }
    }
}

/// Configuration for the stream pipeline
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// The video codec to use
    pub video_codec: VideoCodec,
    /// Video bitrate in bits per second
    pub video_bitrate: u32,
    /// Video resolution width
    pub video_width: u32,
    /// Video resolution height  
    pub video_height: u32,
    /// Video framerate
    pub video_framerate: u32,
    /// The audio codec to use
    pub audio_codec: AudioCodec,
    /// Audio bitrate in bits per second
    pub audio_bitrate: u32,
    /// Audio sample rate
    pub audio_sample_rate: u32,
    /// Audio channels (1 for mono, 2 for stereo)
    pub audio_channels: u8,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            video_codec: VideoCodec::H264,
            video_bitrate: 8_000_000, // 8 Mbps
            video_width: 1920,
            video_height: 1080,
            video_framerate: 30,
            audio_codec: AudioCodec::AAC,
            audio_bitrate: 128_000, // 128 kbps
            audio_sample_rate: 48000,
            audio_channels: 2,
        }
    }
}

/// Errors that can occur during streaming operations
#[derive(Debug)]
pub enum StreamError {
    /// GStreamer initialization error
    GstInit(String),
    /// Pipeline construction error
    PipelineConstruction(String),
    /// Pipeline state transition error
    StateTransition(String),
    /// Invalid configuration
    InvalidConfiguration(String),
    /// Input setup error
    InputSetup(String),
    /// Output setup error
    OutputSetup(String),
    /// IO error
    Io(std::io::Error),
    /// Internal error
    Internal(String),
}

impl fmt::Display for StreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamError::GstInit(msg) => write!(f, "GStreamer initialization error: {}", msg),
            StreamError::PipelineConstruction(msg) => {
                write!(f, "Pipeline construction error: {}", msg)
            }
            StreamError::StateTransition(msg) => {
                write!(f, "Pipeline state transition error: {}", msg)
            }
            StreamError::InvalidConfiguration(msg) => write!(f, "Invalid configuration: {}", msg),
            StreamError::InputSetup(msg) => write!(f, "Input setup error: {}", msg),
            StreamError::OutputSetup(msg) => write!(f, "Output setup error: {}", msg),
            StreamError::Io(err) => write!(f, "IO error: {}", err),
            StreamError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for StreamError {}

impl From<std::io::Error> for StreamError {
    fn from(error: std::io::Error) -> Self {
        StreamError::Io(error)
    }
}

/// GStreamer pipeline wrapper for Miracast streaming
pub struct StreamPipeline {
    _state: PipelineState,
    _config: StreamConfig,
    _pipewire_fd: Option<RawFd>,
    _output_host: Option<String>,
    _output_port: Option<u16>,
}

/// Internal pipeline state representation
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
enum PipelineState {
    Null,
    Ready,
    Paused,
    Playing,
}

impl StreamPipeline {
    /// Creates a new StreamPipeline with the given configuration
    pub fn new(config: StreamConfig) -> Result<Self, StreamError> {
        Ok(StreamPipeline {
            _state: PipelineState::Null,
            _config: config,
            _pipewire_fd: None,
            _output_host: None,
            _output_port: None,
        })
    }

    /// Connects PipeWire source as input to the pipeline
    pub fn set_input(&mut self, pipewire_fd: RawFd) -> Result<(), StreamError> {
        if pipewire_fd < 0 {
            return Err(StreamError::InputSetup("Invalid PipeWire FD".into()));
        }
        self._pipewire_fd = Some(pipewire_fd);
        Ok(())
    }

    /// Configures the output destination for the stream
    pub fn set_output(&mut self, host: &str, port: u16) -> Result<(), StreamError> {
        if host.is_empty() {
            return Err(StreamError::InvalidConfiguration(
                "Host cannot be empty".into(),
            ));
        }
        if port == 0 {
            return Err(StreamError::InvalidConfiguration(
                "Port cannot be zero".into(),
            ));
        }
        self._output_host = Some(host.into());
        self._output_port = Some(port);
        Ok(())
    }

    /// Starts the streaming pipeline
    pub fn start(&self) -> Result<(), StreamError> {
        if self._pipewire_fd.is_none() {
            return Err(StreamError::InvalidConfiguration(
                "Input PipeWire FD not set".into(),
            ));
        }
        if self._output_host.is_none() || self._output_port.is_none() {
            return Err(StreamError::InvalidConfiguration(
                "Output destination not set".into(),
            ));
        }
        todo!("Start GStreamer pipeline");
    }

    /// Stops the streaming pipeline
    pub fn stop(&self) -> Result<(), StreamError> {
        todo!("Stop GStreamer pipeline");
    }
}
