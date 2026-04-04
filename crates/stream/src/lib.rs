use std::fmt;
use swaybeam_capture::PipeWireStream;
mod test_pattern;
use gst_base::prelude::*;
use gstreamer_app::gst_base;
pub use test_pattern::{Frame, TestPatternConfig, TestPatternGenerator};

/// Possible video codecs supported by the stream
#[derive(Debug, Clone, PartialEq)]
pub enum VideoCodec {
    /// H.264 codec, primary for Miracast
    H264,
    /// H.265/HEVC codec, better for 4K streaming
    H265,
    /// AV1 codec, future-proof with best compression
    AV1,
}

impl fmt::Display for VideoCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VideoCodec::H264 => write!(f, "H264"),
            VideoCodec::H265 => write!(f, "H265"),
            VideoCodec::AV1 => write!(f, "AV1"),
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

impl VideoCodec {
    pub fn gstreamer_encoder(&self) -> &'static str {
        match self {
            VideoCodec::H264 => "x264enc",
            VideoCodec::H265 => "x265enc",
            VideoCodec::AV1 => "svtav1enc", // or "av1enc"
        }
    }

    pub fn rtp_payloader(&self) -> &'static str {
        match self {
            VideoCodec::H264 => "rtpmp2tpay",
            VideoCodec::H265 => "rtpmp2tpay",
            VideoCodec::AV1 => "rtpav1pay",
        }
    }

    pub fn parser(&self) -> &'static str {
        match self {
            VideoCodec::H264 => "h264parse",
            VideoCodec::H265 => "h265parse",
            VideoCodec::AV1 => "av1parse",
        }
    }

    pub fn caps_name(&self) -> &'static str {
        match self {
            VideoCodec::H264 => "video/x-h264",
            VideoCodec::H265 => "video/x-h265",
            VideoCodec::AV1 => "video/x-av1",
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

impl StreamConfig {
    pub fn hd_1080p() -> Self {
        Self::default()
    }

    pub fn uhd_4k() -> Self {
        Self {
            video_codec: VideoCodec::H265, // H.265 is better for 4K
            video_bitrate: 20_000_000,     // 20 Mbps for 4K
            video_width: 3840,
            video_height: 2160,
            video_framerate: 30,
            ..Default::default()
        }
    }

    pub fn uhd_4k_60fps() -> Self {
        Self {
            video_codec: VideoCodec::H265,
            video_bitrate: 40_000_000, // 40 Mbps for 4K@60fps
            video_width: 3840,
            video_height: 2160,
            video_framerate: 60,
            ..Default::default()
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
    /// Buffer push error
    BufferPush(String),
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
            StreamError::BufferPush(msg) => write!(f, "Buffer push error: {}", msg),
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

// Add the new GStreamer imports here:
use gstreamer as gst;
use gstreamer::glib;
use gstreamer::prelude::*;
use gstreamer_app::AppSrc;
use std::sync::Arc;
use tokio::sync::Mutex;

impl From<glib::BoolError> for StreamError {
    fn from(error: glib::BoolError) -> Self {
        StreamError::Internal(format!("GStreamer error: {}", error))
    }
}

impl From<gst::glib::error::Error> for StreamError {
    fn from(error: gst::glib::error::Error) -> Self {
        StreamError::Internal(format!("GStreamer error: {}", error))
    }
}

/// GStreamer pipeline wrapper for Miracast streaming
struct StreamPipelineInner {
    pipeline: gst::Pipeline,
    appsrc: Option<AppSrc>,
    #[allow(dead_code)]
    config: StreamConfig,
    state: PipelineState,
    output_host: Option<String>,
    output_port: Option<u16>,
    _audio_appsrc: Option<AppSrc>,
    bus_watch_guard: Option<gst::bus::BusWatchGuard>,
    _pw_stream: Option<PipeWireStream>,
}

impl StreamPipelineInner {
    /// Creates a new StreamPipeline with the given configuration
    pub fn new(config: StreamConfig) -> Result<Self, StreamError> {
        gst::init()?;

        // Build pipeline
        let pipeline = gst::Pipeline::builder().name("miracast-stream").build();

        // Create video elements
        let appsrc_element = gst::ElementFactory::make("appsrc")
            .name("video-source")
            .build()?;

        let appsrc = appsrc_element.dynamic_cast::<AppSrc>().map_err(|_| {
            StreamError::PipelineConstruction("Failed to cast appsrc element to AppSrc".to_string())
        })?;

        let videoconvert = gst::ElementFactory::make("videoconvert").build()?;

        let encoder = gst::ElementFactory::make(config.video_codec.gstreamer_encoder()).build()?;

        let parser = gst::ElementFactory::make(config.video_codec.parser()).build()?;

        let codec_caps = gst::Caps::builder("video/x-h264")
            .field("stream-format", "byte-stream")
            .field("profile", "constrained-baseline")
            .build();
        let codecfilter = gst::ElementFactory::make("capsfilter")
            .name("codecfilter")
            .build()?;
        codecfilter.set_property("caps", &codec_caps);

        let queue_mux = gst::ElementFactory::make("queue")
            .name("queue-mux-video")
            .build()?;
        queue_mux.set_property("max-size-buffers", 1000u32);
        queue_mux.set_property("max-size-time", 500_000_000u64);

        let mpegtsmux = gst::ElementFactory::make("mpegtsmux")
            .name("mpegtsmux")
            .build()?;
        mpegtsmux.set_property("alignment", 7i32);

        let queue_pay = gst::ElementFactory::make("queue")
            .name("queue-pre-payloader")
            .build()?;
        queue_pay.set_property("max-size-buffers", 1u32);

        let rtp_pay = gst::ElementFactory::make(config.video_codec.rtp_payloader())
            .name("pay0")
            .build()?;
        rtp_pay.set_property("ssrc", 1u32);
        rtp_pay.set_property("perfect-rtptime", false);
        rtp_pay.set_property("timestamp-offset", 0u32);
        rtp_pay.set_property("seqnum-offset", 0i32);

        let udpsink = gst::ElementFactory::make("udpsink")
            .name("udpsink")
            .build()?;

        appsrc.set_property("is-live", true);
        appsrc.set_property("format", gst::Format::Time);

        fn set_prop_safe(element: &gst::Element, name: &str, value: &str) {
            element.set_property_from_str(name, value);
        }

        fn set_prop_int_safe(element: &gst::Element, name: &str, value: i32) {
            element.set_property(name, value);
        }

        match config.video_codec {
            VideoCodec::H264 => {
                set_prop_safe(&encoder, "tune", "zerolatency");
                set_prop_safe(&encoder, "speed-preset", "veryfast");
            }
            VideoCodec::H265 => {
                set_prop_safe(&encoder, "tune", "zerolatency");
                set_prop_safe(&encoder, "speed-preset", "fast");
            }
            VideoCodec::AV1 => {
                set_prop_int_safe(&encoder, "preset", 8);
                set_prop_int_safe(
                    &encoder,
                    "target-bitrate",
                    config.video_bitrate as i32 / 1000,
                );
            }
        }

        if !matches!(config.video_codec, VideoCodec::AV1) {
            encoder.set_property_from_str("bitrate", &(config.video_bitrate / 1000).to_string());
        }
        encoder.set_property_from_str("key-int-max", &(config.video_framerate * 2).to_string());

        parser.set_property("config-interval", -1i32);

        udpsink.set_property_from_str("host", "127.0.0.1");
        udpsink.set_property_from_str("port", "5004");
        udpsink.set_property("sync", false);
        udpsink.set_property("async", false);

        pipeline.add_many([
            appsrc.upcast_ref::<gst::Element>(),
            &videoconvert,
            &encoder,
            &parser,
            &codecfilter,
            &queue_mux,
            &mpegtsmux,
            &queue_pay,
            &rtp_pay,
            &udpsink,
        ])?;

        if appsrc.link(&videoconvert).is_err() {
            return Err(StreamError::PipelineConstruction(
                "Failed to link appsrc to videoconvert".to_string(),
            ));
        }
        if videoconvert.link(&encoder).is_err() {
            return Err(StreamError::PipelineConstruction(
                "Failed to link videoconvert to encoder".to_string(),
            ));
        }
        if encoder.link(&parser).is_err() {
            return Err(StreamError::PipelineConstruction(
                "Failed to link encoder to parser".to_string(),
            ));
        }
        if parser.link(&codecfilter).is_err() {
            return Err(StreamError::PipelineConstruction(
                "Failed to link parser to codecfilter".to_string(),
            ));
        }
        if codecfilter.link(&queue_mux).is_err() {
            return Err(StreamError::PipelineConstruction(
                "Failed to link codecfilter to queue_mux".to_string(),
            ));
        }

        let queue_mux_src = queue_mux.static_pad("src").ok_or_else(|| {
            StreamError::PipelineConstruction("Failed to get queue_mux src pad".to_string())
        })?;
        let mpegtsmux_sink = mpegtsmux.request_pad_simple("sink_4113").ok_or_else(|| {
            StreamError::PipelineConstruction(
                "Failed to request sink_4113 pad from mpegtsmux".to_string(),
            )
        })?;
        if queue_mux_src.link(&mpegtsmux_sink).is_err() {
            return Err(StreamError::PipelineConstruction(
                "Failed to link queue_mux to mpegtsmux".to_string(),
            ));
        }

        if mpegtsmux.link(&queue_pay).is_err() {
            return Err(StreamError::PipelineConstruction(
                "Failed to link mpegtsmux to queue_pay".to_string(),
            ));
        }
        if queue_pay.link(&rtp_pay).is_err() {
            return Err(StreamError::PipelineConstruction(
                "Failed to link queue_pay to rtp_pay".to_string(),
            ));
        }
        if rtp_pay.link(&udpsink).is_err() {
            return Err(StreamError::PipelineConstruction(
                "Failed to link rtp_pay to udpsink".to_string(),
            ));
        }

        // Set up bus watch to handle messages from the pipeline
        let bus_watch_guard = {
            let bus = pipeline
                .bus()
                .ok_or_else(|| StreamError::Internal("Pipeline has no bus".to_string()))?;

            bus.add_watch_local(move |_, msg| {
                match msg.view() {
                    gst::MessageView::Error(err) => {
                        tracing::error!(
                            "GStreamer error: {}, details: {:?}",
                            err.error(),
                            err.debug()
                        );
                    }
                    gst::MessageView::Warning(warn) => {
                        tracing::warn!(
                            "GStreamer warning: {}, details: {:?}",
                            warn.error(),
                            warn.debug()
                        );
                    }
                    gst::MessageView::Info(info) => {
                        tracing::info!("GStreamer info: {:?}", info.error());
                    }
                    gst::MessageView::StateChanged(state_changed) => {
                        let src = state_changed
                            .src()
                            .map(|s| s.path_string())
                            .unwrap_or_else(|| "unknown".into());
                        tracing::debug!(
                            "State changed: {} - {:?} -> {:?}",
                            src,
                            state_changed.old(),
                            state_changed.current()
                        );
                    }
                    gst::MessageView::Eos(_) => {
                        tracing::info!("Stream end-of-stream reached");
                    }
                    _ => {}
                }
                gstreamer::glib::ControlFlow::Continue
            })
            .map_err(|e| StreamError::Internal(format!("Failed to add bus watch: {}", e)))?
        };

        Ok(StreamPipelineInner {
            pipeline,
            appsrc: Some(appsrc),
            config,
            state: PipelineState::Null,
            output_host: None,
            output_port: None,
            _audio_appsrc: None,
            bus_watch_guard: Some(bus_watch_guard),
            _pw_stream: None,
        })
    }

    pub fn new_pipewire(
        config: StreamConfig,
        pw_stream: PipeWireStream,
    ) -> Result<Self, StreamError> {
        gst::init()?;

        let fd = pw_stream.fd();
        let node_id = pw_stream.node_id();

        tracing::info!(
            "Creating pipewire pipeline with fd={}, node_id={}",
            fd,
            node_id
        );

        // Validate fd is still valid
        let fd_valid = unsafe { libc::fcntl(fd, libc::F_GETFD) } != -1;
        tracing::info!("FD {} validity check: {}", fd, fd_valid);
        if !fd_valid {
            return Err(StreamError::PipelineConstruction(format!(
                "FD {} is invalid or closed before pipeline creation",
                fd
            )));
        }

        let pipeline = gst::Pipeline::builder().name("miracast-stream").build();

        let pipewiresrc: gst::Element = gst::ElementFactory::make("pipewiresrc")
            .name("src")
            .build()?;

        pipewiresrc.set_property("fd", fd);
        pipewiresrc.set_property("path", node_id.to_string());
        pipewiresrc.set_property("do-timestamp", true);

        let pipewiresrc_base: &gstreamer_app::gst_base::BaseSrc =
            pipewiresrc.dynamic_cast_ref().ok_or_else(|| {
                StreamError::PipelineConstruction("Failed to cast pipewiresrc to BaseSrc".into())
            })?;
        pipewiresrc_base.set_live(true);

        tracing::info!(
            "pipewiresrc configured: fd={}, path={}, do-timestamp=true, live=true",
            fd,
            node_id
        );

        let videoconvert = gst::ElementFactory::make("videoconvert").build()?;
        let encoder = gst::ElementFactory::make(config.video_codec.gstreamer_encoder())
            .name("enc")
            .build()?;
        let parser = gst::ElementFactory::make(config.video_codec.parser())
            .name("parser")
            .build()?;
        let queue_mux = gst::ElementFactory::make("queue")
            .name("queue-mux-video")
            .build()?;
        let mpegtsmux = gst::ElementFactory::make("mpegtsmux").build()?;
        let queue_pay = gst::ElementFactory::make("queue")
            .name("queue-pre-payloader")
            .build()?;
        let rtp_pay = gst::ElementFactory::make(config.video_codec.rtp_payloader())
            .name("pay0")
            .build()?;
        let udpsink = gst::ElementFactory::make("udpsink")
            .name("udpsink")
            .build()?;

        let key_int_max = config.video_framerate * 2;
        match config.video_codec {
            VideoCodec::H264 => {
                encoder.set_property_from_str("tune", "zerolatency");
                encoder.set_property_from_str("speed-preset", "veryfast");
                encoder.set_property("bitrate", config.video_bitrate / 1000);
                encoder.set_property("key-int-max", key_int_max);
            }
            VideoCodec::H265 => {
                encoder.set_property_from_str("tune", "zerolatency");
                encoder.set_property_from_str("speed-preset", "fast");
                encoder.set_property("bitrate", config.video_bitrate / 1000);
                encoder.set_property("key-int-max", key_int_max);
            }
            VideoCodec::AV1 => {
                encoder.set_property("preset", 8i32);
                encoder.set_property("target-bitrate", config.video_bitrate / 1000);
            }
        }

        parser.set_property("config-interval", -1i32);

        queue_mux.set_property("max-size-buffers", 1000u32);
        queue_mux.set_property("max-size-time", 500_000_000u64);
        queue_pay.set_property("max-size-buffers", 1u32);

        mpegtsmux.set_property("alignment", 7i32);

        rtp_pay.set_property("ssrc", 1u32);
        rtp_pay.set_property("perfect-rtptime", false);
        rtp_pay.set_property("timestamp-offset", 0u32);
        rtp_pay.set_property("seqnum-offset", 0i32);

        udpsink.set_property("host", "127.0.0.1");
        udpsink.set_property("port", 5004i32);
        udpsink.set_property("sync", false);
        udpsink.set_property("async", false);

        let capsfilter = gst::ElementFactory::make("capsfilter")
            .name("caps-filter")
            .build()?;
        let caps = gst::Caps::builder("video/x-h264")
            .field("stream-format", "byte-stream")
            .field("profile", "constrained-baseline")
            .build();
        capsfilter.set_property("caps", &caps);

        pipeline.add_many([
            &pipewiresrc,
            &videoconvert,
            &encoder,
            &parser,
            &capsfilter,
            &queue_mux,
            &mpegtsmux,
            &queue_pay,
            &rtp_pay,
            &udpsink,
        ])?;

        gst::Element::link_many([
            &pipewiresrc,
            &videoconvert,
            &encoder,
            &parser,
            &capsfilter,
            &queue_mux,
        ])?;

        let queue_mux_src = queue_mux.static_pad("src").ok_or_else(|| {
            StreamError::PipelineConstruction("Failed to get queue_mux src pad".into())
        })?;
        let mpegtsmux_sink = mpegtsmux.request_pad_simple("sink_4113").ok_or_else(|| {
            StreamError::PipelineConstruction(
                "Failed to request sink_4113 pad from mpegtsmux".into(),
            )
        })?;
        queue_mux_src.link(&mpegtsmux_sink).map_err(|e| {
            StreamError::PipelineConstruction(format!(
                "Failed to link queue_mux to mpegtsmux: {:?}",
                e
            ))
        })?;

        gst::Element::link_many([&mpegtsmux, &queue_pay, &rtp_pay, &udpsink])?;

        let bus_watch_guard = {
            let bus = pipeline
                .bus()
                .ok_or_else(|| StreamError::Internal("Pipeline has no bus".into()))?;

            bus.add_watch_local(move |_, msg| {
                match msg.view() {
                    gst::MessageView::Error(err) => {
                        tracing::error!(
                            "GStreamer ERROR from {}: {} (code: {}), debug: {:?}",
                            err.src()
                                .map(|s| s.path_string())
                                .unwrap_or_else(|| "unknown".into()),
                            err.error(),
                            err.error().code(),
                            err.debug()
                        );
                    }
                    gst::MessageView::Warning(warn) => {
                        tracing::warn!(
                            "GStreamer WARNING from {}: {} (code: {}), debug: {:?}",
                            warn.src()
                                .map(|s| s.path_string())
                                .unwrap_or_else(|| "unknown".into()),
                            warn.error(),
                            warn.error().code(),
                            warn.debug()
                        );
                    }
                    gst::MessageView::Info(info) => {
                        tracing::info!(
                            "GStreamer INFO from {}: {:?}",
                            info.src()
                                .map(|s| s.path_string())
                                .unwrap_or_else(|| "unknown".into()),
                            info.error()
                        );
                    }
                    gst::MessageView::StateChanged(state_changed) => {
                        let src = state_changed
                            .src()
                            .map(|s| s.path_string())
                            .unwrap_or_else(|| "unknown".into());
                        tracing::debug!(
                            "State changed: {} - {:?} -> {:?}",
                            src,
                            state_changed.old(),
                            state_changed.current()
                        );
                    }
                    gst::MessageView::StreamStatus(status) => {
                        tracing::info!(
                            "Stream status: type={:?} from {}",
                            status.type_(),
                            status
                                .src()
                                .map(|s| s.path_string())
                                .unwrap_or_else(|| "unknown".into())
                        );
                    }
                    gst::MessageView::Buffering(buf) => {
                        tracing::info!("Buffering: {}%", buf.percent());
                    }
                    gst::MessageView::Eos(_) => {
                        tracing::warn!("Stream EOS reached");
                    }
                    gst::MessageView::Latency(_) => {
                        tracing::debug!("Latency message");
                    }
                    gst::MessageView::NewClock(clock) => {
                        tracing::debug!("New clock: {:?}", clock.clock().map(|c| c.name()));
                    }
                    gst::MessageView::Element(el) => {
                        tracing::debug!("Element message: {:?}", el.structure());
                    }
                    _ => {}
                }
                gstreamer::glib::ControlFlow::Continue
            })
            .map_err(|e| StreamError::Internal(format!("Failed to add bus watch: {}", e)))?
        };

        tracing::info!("GStreamer pipeline constructed successfully via parse::launch");

        Ok(StreamPipelineInner {
            pipeline,
            appsrc: None,
            config,
            state: PipelineState::Null,
            output_host: None,
            output_port: None,
            _audio_appsrc: None,
            bus_watch_guard: Some(bus_watch_guard),
            _pw_stream: Some(pw_stream),
        })
    }

    /// Sets the output destination for the stream
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

        let udpsink = self
            .pipeline
            .by_name("udpsink")
            .ok_or_else(|| StreamError::OutputSetup("Failed to get udpsink element".into()))?;

        udpsink.set_property("host", host);
        udpsink.set_property("port", port as i32);
        udpsink.set_property("sync", false);
        udpsink.set_property("async", false);

        self.output_host = Some(host.to_string());
        self.output_port = Some(port);

        Ok(())
    }

    /// Sets caps for the input
    pub fn set_caps(&self, caps: &gst::Caps) -> Result<(), StreamError> {
        if let Some(ref appsrc) = self.appsrc {
            appsrc.set_caps(Some(caps));
        }
        Ok(())
    }

    pub fn push_video_buffer(&self, buffer: &gst::Buffer) -> Result<(), StreamError> {
        if let Some(ref appsrc) = self.appsrc {
            let result = appsrc.push_buffer(buffer.clone());
            match result {
                Ok(_) => Ok(()),
                Err(e) => Err(StreamError::BufferPush(e.to_string())),
            }
        } else {
            Err(StreamError::BufferPush("No appsrc available".to_string()))
        }
    }

    pub fn push_video_data(&self, data: Vec<u8>) -> Result<(), StreamError> {
        let gst_buffer = gst::Buffer::from_slice(data);
        self.push_video_buffer(&gst_buffer)
    }

    /// Starts the streaming pipeline
    pub fn start(&mut self) -> Result<(), StreamError> {
        let result = self.pipeline.set_state(gst::State::Playing);
        match result {
            Ok(_) => {
                self.state = PipelineState::Playing;
                Ok(())
            }
            Err(_) => Err(StreamError::StateTransition(
                "Failed to set pipeline to Playing state".into(),
            )),
        }
    }

    /// Stops the streaming pipeline
    pub fn stop(&mut self) -> Result<(), StreamError> {
        // Send EOS event to trigger cleanup
        self.pipeline.send_event(gst::event::Eos::new());

        let result = self.pipeline.set_state(gst::State::Null);
        match result {
            Ok(_) => {
                self.state = PipelineState::Null;
                Ok(())
            }
            Err(_) => Err(StreamError::StateTransition(
                "Failed to set pipeline to Null state".into(),
            )),
        }
    }

    /// Gets the current pipeline state
    pub fn state(&self) -> PipelineState {
        self.state.clone()
    }
}

impl Drop for StreamPipelineInner {
    fn drop(&mut self) {
        // Stop the pipeline by setting it to Null state
        if self.state != PipelineState::Null {
            let _ = self.pipeline.set_state(gst::State::Null);
        }

        // The bus watch guard will be automatically dropped here,
        // which will cleanly remove the bus watch callback
        // Explicitly take() it to be clear we're dropping it
        self.bus_watch_guard.take();
    }
}

/// Inner type for encapsulating the GStreamer pipeline with a mutex
#[derive(Clone)]
pub struct StreamPipeline {
    inner: Arc<Mutex<StreamPipelineInner>>,
}

impl StreamPipeline {
    pub fn new(config: StreamConfig) -> Result<Self, StreamError> {
        let inner = StreamPipelineInner::new(config)?;
        Ok(StreamPipeline {
            inner: Arc::new(Mutex::new(inner)),
        })
    }

    pub fn new_pipewire(
        config: StreamConfig,
        pw_stream: PipeWireStream,
    ) -> Result<Self, StreamError> {
        let inner = StreamPipelineInner::new_pipewire(config, pw_stream)?;
        Ok(StreamPipeline {
            inner: Arc::new(Mutex::new(inner)),
        })
    }

    pub async fn set_output(&self, host: &str, port: u16) -> Result<(), StreamError> {
        let mut guard = self.inner.lock().await;
        guard.set_output(host, port)
    }

    pub async fn set_caps(&self, caps: &gst::Caps) -> Result<(), StreamError> {
        let guard = self.inner.lock().await;
        guard.set_caps(caps)
    }

    pub async fn push_video_data(&self, data: Vec<u8>) -> Result<(), StreamError> {
        let guard = self.inner.lock().await;
        guard.push_video_data(data)
    }

    pub async fn push_video_buffer(&self, buffer: &gst::Buffer) -> Result<(), StreamError> {
        let guard = self.inner.lock().await;
        guard.push_video_buffer(buffer)
    }

    pub async fn start(&self) -> Result<(), StreamError> {
        let mut guard = self.inner.lock().await;
        guard.start()
    }

    pub async fn stop(&self) -> Result<(), StreamError> {
        let mut guard = self.inner.lock().await;
        guard.stop()
    }

    pub async fn state(&self) -> PipelineState {
        let guard = self.inner.lock().await;
        guard.state()
    }

    pub async fn force_keyframe(&self) -> Result<(), StreamError> {
        let guard = self.inner.lock().await;
        let source = guard.pipeline.by_name("video-source").ok_or_else(|| {
            StreamError::Internal("Failed to find video-source element".to_string())
        })?;
        let event = gstreamer_video::UpstreamForceKeyUnitEvent::builder()
            .all_headers(true)
            .build();
        if source.send_event(event) {
            tracing::debug!("Sent force-key-unit event upstream");
            Ok(())
        } else {
            tracing::warn!("Failed to send force-key-unit event");
            Ok(())
        }
    }
}

/// Internal pipeline state representation
#[derive(Debug, Clone, PartialEq)]
pub enum PipelineState {
    Null,
    Ready,
    Paused,
    Playing,
}

impl fmt::Display for PipelineState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PipelineState::Null => write!(f, "Null"),
            PipelineState::Ready => write!(f, "Ready"),
            PipelineState::Paused => write!(f, "Paused"),
            PipelineState::Playing => write!(f, "Playing"),
        }
    }
}

/// Add bus handling function for monitoring pipeline events
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

    #[tokio::test]
    async fn test_stream_pipeline_new_success() {
        let config = StreamConfig::default();
        let result = StreamPipeline::new(config);
        // This may fail in CI if GStreamer plugins (x264enc, etc.) are not available
        // We just verify that the function returns a proper Result type
        match result {
            Ok(_) => {}
            Err(StreamError::GstInit(_)) => {}
            Err(StreamError::PipelineConstruction(_)) => {}
            Err(StreamError::Internal(_)) => {}
            Err(e) => panic!("Unexpected error type: {}", e),
        }
    }

    #[test]
    fn test_pipeline_state_variants() {
        assert_eq!(PipelineState::Null, PipelineState::Null);
        assert_eq!(PipelineState::Ready, PipelineState::Ready);
        assert_eq!(PipelineState::Paused, PipelineState::Paused);
        assert_eq!(PipelineState::Playing, PipelineState::Playing);
    }

    #[test]
    fn test_pipeline_state_display() {
        assert_eq!(PipelineState::Null.to_string(), "Null");
        assert_eq!(PipelineState::Ready.to_string(), "Ready");
        assert_eq!(PipelineState::Paused.to_string(), "Paused");
        assert_eq!(PipelineState::Playing.to_string(), "Playing");
    }

    #[test]
    fn test_stream_error_display() {
        let err = StreamError::GstInit("test".to_string());
        assert!(err.to_string().contains("GStreamer initialization error"));

        let err = StreamError::InputSetup("test".to_string());
        assert!(err.to_string().contains("Input setup error"));

        let err = StreamError::OutputSetup("test".to_string());
        assert!(err.to_string().contains("Output setup error"));

        let err = StreamError::BufferPush("test".to_string());
        assert!(err.to_string().contains("Buffer push error"));
    }

    #[test]
    fn test_stream_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
        let stream_err: StreamError = io_err.into();
        assert!(matches!(stream_err, StreamError::Io(_)));
    }

    #[test]
    fn test_video_codec_functions() {
        // Test H.264
        assert_eq!(VideoCodec::H264.gstreamer_encoder(), "x264enc");
        assert_eq!(VideoCodec::H264.rtp_payloader(), "rtpmp2tpay");
        assert_eq!(VideoCodec::H264.parser(), "h264parse");
        assert_eq!(VideoCodec::H264.caps_name(), "video/x-h264");

        // Test H.265
        assert_eq!(VideoCodec::H265.gstreamer_encoder(), "x265enc");
        assert_eq!(VideoCodec::H265.rtp_payloader(), "rtpmp2tpay");
        assert_eq!(VideoCodec::H265.parser(), "h265parse");
        assert_eq!(VideoCodec::H265.caps_name(), "video/x-h265");

        // Test AV1
        assert_eq!(VideoCodec::AV1.gstreamer_encoder(), "svtav1enc");
        assert_eq!(VideoCodec::AV1.rtp_payloader(), "rtpav1pay");
        assert_eq!(VideoCodec::AV1.parser(), "av1parse");
        assert_eq!(VideoCodec::AV1.caps_name(), "video/x-av1");
    }

    #[test]
    fn test_4k_presets() {
        // Test 4K resolution presets
        let config_4k = StreamConfig::uhd_4k();
        assert_eq!(config_4k.video_codec, VideoCodec::H265);
        assert_eq!(config_4k.video_width, 3840);
        assert_eq!(config_4k.video_height, 2160);
        assert_eq!(config_4k.video_bitrate, 20_000_000);
        assert_eq!(config_4k.video_framerate, 30);

        let config_4k_60fps = StreamConfig::uhd_4k_60fps();
        assert_eq!(config_4k_60fps.video_codec, VideoCodec::H265);
        assert_eq!(config_4k_60fps.video_width, 3840);
        assert_eq!(config_4k_60fps.video_height, 2160);
        assert_eq!(config_4k_60fps.video_bitrate, 40_000_000);
        assert_eq!(config_4k_60fps.video_framerate, 60);
    }
}
