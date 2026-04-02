use miracast_stream::{StreamConfig, StreamPipeline, VideoCodec};

#[test]
fn test_h264_pipeline_creation() {
    let config = StreamConfig::hd_1080p();
    assert_eq!(config.video_codec, VideoCodec::H264);
    let pipeline = StreamPipeline::new(config);
    assert!(pipeline.is_ok());
}

#[test]
#[ignore = "Requires GStreamer with x265"]
fn test_h265_pipeline_creation() {
    let config = StreamConfig {
        video_codec: VideoCodec::H265,
        video_width: 3840,
        video_height: 2160,
        video_bitrate: 20_000_000,
        ..Default::default()
    };
    assert_eq!(config.video_codec, VideoCodec::H265);
    let pipeline = StreamPipeline::new(config);
    assert!(pipeline.is_ok());
}

#[test]
#[ignore = "Requires GStreamer with SVT-AV1"]
fn test_av1_pipeline_creation() {
    let config = StreamConfig {
        video_codec: VideoCodec::AV1,
        video_width: 1920,
        video_height: 1080,
        video_bitrate: 5_000_000, // AV1 is more efficient
        ..Default::default()
    };
    assert_eq!(config.video_codec, VideoCodec::AV1);
    let pipeline = StreamPipeline::new(config);
    assert!(pipeline.is_ok());
}

#[test]
fn test_4k_30fps_preset() {
    let config = StreamConfig::uhd_4k();
    assert_eq!(config.video_codec, VideoCodec::H265);
    assert_eq!(config.video_width, 3840);
    assert_eq!(config.video_height, 2160);
    assert_eq!(config.video_framerate, 30);
    assert_eq!(config.video_bitrate, 20_000_000);
}

#[test]
fn test_4k_60fps_preset() {
    let config = StreamConfig::uhd_4k_60fps();
    assert_eq!(config.video_codec, VideoCodec::H265);
    assert_eq!(config.video_framerate, 60);
    assert_eq!(config.video_bitrate, 40_000_000);
}

#[tokio::test]
#[ignore = "Requires GStreamer with x265"]
async fn test_h265_pipeline_start_stop() {
    let config = StreamConfig::uhd_4k();
    let pipeline = StreamPipeline::new(config).unwrap();

    pipeline.set_output("127.0.0.1", 5004).await.unwrap();
    pipeline.start().await.unwrap();
    assert_eq!(
        pipeline.state().await,
        miracast_stream::PipelineState::Playing
    );

    pipeline.stop().await.unwrap();
    assert_eq!(pipeline.state().await, miracast_stream::PipelineState::Null);
}
