//! Example demonstrating RTSP server functionality

use std::collections::HashMap;
use swaybeam_rtsp::{RtspServer, RtspSession};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Miracast RTSP - WFD Negotiation Example");
    println!("=======================================");

    let _server = RtspServer::new("127.0.0.1:7236".to_string());
    println!("RTSP Server initialized at 127.0.0.1:7236");

    let session_id = "sample_session".to_string();
    let mut session = RtspSession::new(session_id.clone());
    println!("\nSession created, initial state: {:?}", session.state);

    // Simulate WFD negotiation
    session.process_options()?;
    println!("OPTIONS processed, state: {:?}", session.state);

    let mut params = HashMap::new();
    params.insert("wfd_video_formats".to_string(), "00 04".to_string());
    session.process_set_parameter(&params)?;
    println!("SET_PARAMETER done, state: {:?}", session.state);

    session.process_get_parameter(&["wfd_video_formats"])?;
    println!("GET_PARAMETER done, state: {:?}", session.state);

    session.process_play()?;
    println!("PLAY initiated, state: {:?}", session.state);

    session.process_teardown()?;
    println!("TEARDOWN complete, state: {:?}", session.state);

    println!("\nSession simulation completed!");
    Ok(())
}
