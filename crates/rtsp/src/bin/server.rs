use miracast_rtsp::RtspServer;  // Import from the crate with proper name

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let server = RtspServer::new("127.0.0.1:8554".to_string());
    println!("Starting RTSP server on 127.0.0.1:8554...");
    server.start().await?;

    Ok(())
}
