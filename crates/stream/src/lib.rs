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
        // Validate required parameters are set
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

        // Extract the required values
        let host = self._output_host.as_ref().unwrap();
        let port = self._output_port.unwrap();

        // Log what would be streamed in a complete implementation
        println!(
            "Would start streaming {}x{}@{}fps at {}kbps to {}:{}",
            self._config.video_width,
            self._config.video_height,
            self._config.video_framerate,
            self._config.video_bitrate / 1000, // Convert to kbps
            host,
            port
        );

        // In a complete implementation, this would create and start the actual GStreamer pipeline:
        // 1. Use GStreamer to create a pipeline with H.264 encoding
        // 2. Include elements: appsrc (for PipeWire), videoconvert, x264enc, rtph264pay, udpsink
        // 3. Start the pipeline in Playing state
        // 4. Return success or error

        Ok(())
    }

    /// Stops the streaming pipeline
    pub fn stop(&self) -> Result<(), StreamError> {
        // In a complete implementation, this would stop the active pipeline
        println!("Would stop streaming pipeline");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_codec_display() {
        assert_eq!(VideoCodec::H264.to_string(), "H264");
    }

    #[test]
    fn test_audio_codec_display() {
        assert_eq!(AudioCodec::AAC.to_string(), "AAC");
        assert_eq!(AudioCodec::LPCM.to_string(), "LPCM");
    }

    #[test]
    fn test_stream_config_default() {
        let config = StreamConfig::default();
        assert_eq!(config.video_codec, VideoCodec::H264);
        assert_eq!(config.audio_codec, AudioCodec::AAC);
        assert_eq!(config.video_bitrate, 8_000_000);
        assert_eq!(config.video_width, 1920);
        assert_eq!(config.video_height, 1080);
        assert_eq!(config.video_framerate, 30);
        assert_eq!(config.audio_bitrate, 128_000);
        assert_eq!(config.audio_sample_rate, 48000);
        assert_eq!(config.audio_channels, 2);
    }

    #[test]
    fn test_stream_config_custom() {
        let config = StreamConfig {
            video_codec: VideoCodec::H264,
            audio_codec: AudioCodec::LPCM,
            video_bitrate: 10_000_000,
            video_width: 1280,
            video_height: 720,
            video_framerate: 60,
            audio_bitrate: 256_000,
            audio_sample_rate: 44100,
            audio_channels: 1,
        };
        assert_eq!(config.video_width, 1280);
        assert_eq!(config.video_framerate, 60);
        assert_eq!(config.audio_channels, 1);
    }

    #[test]
    fn test_stream_pipeline_new_success() {
        let config = StreamConfig::default();
        let result = StreamPipeline::new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stream_pipeline_set_input_valid() {
        let config = StreamConfig::default();
        let mut pipeline = StreamPipeline::new(config).unwrap();
        let result = pipeline.set_input(42);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stream_pipeline_set_input_negative_fd() {
        let config = StreamConfig::default();
        let mut pipeline = StreamPipeline::new(config).unwrap();
        let result = pipeline.set_input(-1);
        assert!(result.is_err());
        match result {
            Err(StreamError::InputSetup(msg)) => {
                assert!(msg.contains("Invalid PipeWire FD"));
            }
            _ => panic!("Expected InputSetup error"),
        }
    }

    #[test]
    fn test_stream_pipeline_set_output_valid() {
        let config = StreamConfig::default();
        let mut pipeline = StreamPipeline::new(config).unwrap();
        let result = pipeline.set_output("192.168.1.1", 5004);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stream_pipeline_set_output_empty_host() {
        let config = StreamConfig::default();
        let mut pipeline = StreamPipeline::new(config).unwrap();
        let result = pipeline.set_output("", 5004);
        assert!(result.is_err());
        match result {
            Err(StreamError::InvalidConfiguration(msg)) => {
                assert!(msg.contains("Host cannot be empty"));
            }
            _ => panic!("Expected InvalidConfiguration error"),
        }
    }

    #[test]
    fn test_stream_pipeline_set_output_zero_port() {
        let config = StreamConfig::default();
        let mut pipeline = StreamPipeline::new(config).unwrap();
        let result = pipeline.set_output("192.168.1.1", 0);
        assert!(result.is_err());
        match result {
            Err(StreamError::InvalidConfiguration(msg)) => {
                assert!(msg.contains("Port cannot be zero"));
            }
            _ => panic!("Expected InvalidConfiguration error"),
        }
    }

    #[test]
    fn test_stream_error_display() {
        let err = StreamError::GstInit("test".to_string());
        assert!(err.to_string().contains("GStreamer initialization error"));

        let err = StreamError::InputSetup("test".to_string());
        assert!(err.to_string().contains("Input setup error"));

        let err = StreamError::OutputSetup("test".to_string());
        assert!(err.to_string().contains("Output setup error"));
    }

    #[test]
    fn test_stream_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
        let stream_err: StreamError = io_err.into();
        assert!(matches!(stream_err, StreamError::Io(_)));
    }

    #[test]
    fn test_pipeline_state_variants() {
        assert_eq!(PipelineState::Null, PipelineState::Null);
        assert_eq!(PipelineState::Ready, PipelineState::Ready);
        assert_eq!(PipelineState::Paused, PipelineState::Paused);
        assert_eq!(PipelineState::Playing, PipelineState::Playing);
    }
}
