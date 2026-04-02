use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const WFD_SOURCE_CAPS: &str =
    "wfd_video_formats: 00 01 02 04 0001FEFF 3FFFFFFF 00000FFF 00 0000 0000 00 00000000 00 00000000 00\r\n\
     wfd_audio_codecs: AAC 00000001 00\r\n\
     wfd_client_rtp_ports: RTP/AVP/UDP;unicast 5004 5005 mode=play\r\n\
     wfd_uibc_capability: none\r\n";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("0.0.0.0:7236").await?;
    eprintln!("=== MIRACAST RTSP DEBUG SERVER ===");
    eprintln!("Listening on 0.0.0.0:7236");
    eprintln!("Waiting for TV to connect...\n");

    loop {
        let (mut socket, addr) = listener.accept().await?;
        eprintln!("\n=== CLIENT CONNECTED: {} ===\n", addr);

        tokio::spawn(async move {
            let mut buffer = vec![0u8; 8192];
            let mut session_id = format!(
                "{:08x}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            );

            loop {
                match socket.read(&mut buffer).await {
                    Ok(0) => {
                        eprintln!("\n=== CLIENT DISCONNECTED ===\n");
                        break;
                    }
                    Ok(n) => {
                        let data = &buffer[..n];
                        eprintln!("\n--- RECEIVED {} bytes ---", n);
                        print_hex_dump(data);

                        let request = String::from_utf8_lossy(data);
                        eprintln!("\n{}", request);

                        let response = handle_rtsp(&request, &mut session_id);

                        eprintln!("\n--- SENDING RESPONSE ---");
                        print_hex_dump(response.as_bytes());
                        eprintln!("\n{}", response);

                        if let Err(e) = socket.write_all(response.as_bytes()).await {
                            eprintln!("Send error: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("Read error: {}", e);
                        break;
                    }
                }
            }
        });
    }
}

fn print_hex_dump(data: &[u8]) {
    for (i, chunk) in data.chunks(16).enumerate() {
        let hex: String = chunk.iter().map(|b| format!("{:02x} ", b)).collect();
        let ascii: String = chunk
            .iter()
            .map(|&b| if b >= 32 && b <= 126 { b as char } else { '.' })
            .collect();
        eprintln!("{:04x}: {:48} |{}|", i * 16, hex, ascii);
    }
}

fn handle_rtsp(request: &str, session_id: &mut String) -> String {
    let lines: Vec<&str> = request.split("\r\n").collect();
    if lines.is_empty() {
        return "RTSP/1.0 400 Bad Request\r\n\r\n".to_string();
    }

    let mut cseq = 1u32;
    for line in &lines {
        if line.to_lowercase().starts_with("cseq:") {
            if let Some(num) = line.split(':').nth(1) {
                if let Ok(n) = num.trim().parse::<u32>() {
                    cseq = n;
                }
            }
        }
    }

    let first_line = lines[0];
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    let method = parts.get(0).unwrap_or(&"");

    eprintln!("\n>>> METHOD: {}", method);

    match *method {
        "OPTIONS" => {
            format!(
                "RTSP/1.0 200 OK\r\n\
                 CSeq: {}\r\n\
                 Public: org.wfa.wfd1.0, OPTIONS, DESCRIBE, GET_PARAMETER, PAUSE, PLAY, SETUP, SET_PARAMETER, TEARDOWN\r\n\
                 \r\n",
                cseq
            )
        }
        "GET_PARAMETER" => {
            let body_start = request.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
            let body = &request[body_start..];

            if body.contains("wfd_") {
                format!(
                    "RTSP/1.0 200 OK\r\n\
                     CSeq: {}\r\n\
                     Content-Type: text/parameters\r\n\
                     Content-Length: {}\r\n\
                     \r\n\
                     {}",
                    cseq,
                    WFD_SOURCE_CAPS.len(),
                    WFD_SOURCE_CAPS
                )
            } else {
                format!(
                    "RTSP/1.0 200 OK\r\n\
                     CSeq: {}\r\n\
                     \r\n",
                    cseq
                )
            }
        }
        "SET_PARAMETER" => {
            let body_start = request.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
            let body = &request[body_start..];
            eprintln!("\n>>> TV PARAMETERS:\n{}", body);

            format!(
                "RTSP/1.0 200 OK\r\n\
                 CSeq: {}\r\n\
                 \r\n",
                cseq
            )
        }
        "SETUP" => {
            let mut client_port = "5004-5005";
            for line in &lines {
                if line.to_lowercase().starts_with("transport:") {
                    if let Some(cp) = line.split("client_port=").nth(1) {
                        client_port = cp.split(';').next().unwrap_or(client_port);
                    }
                }
            }

            eprintln!("\n>>> SETUP: client_port={}", client_port);

            format!(
                "RTSP/1.0 200 OK\r\n\
                 CSeq: {}\r\n\
                 Session: {}\r\n\
                 Transport: RTP/AVP/UDP;unicast;client_port={};server_port=5004-5005\r\n\
                 \r\n",
                cseq, session_id, client_port
            )
        }
        "PLAY" => {
            eprintln!("\n>>> PLAY - START STREAMING NOW!");

            format!(
                "RTSP/1.0 200 OK\r\n\
                 CSeq: {}\r\n\
                 Session: {}\r\n\
                 Range: npt=0.000-\r\n\
                 \r\n",
                cseq, session_id
            )
        }
        "TEARDOWN" => {
            eprintln!("\n>>> TEARDOWN");
            format!("RTSP/1.0 200 OK\r\nCSeq: {}\r\n\r\n", cseq)
        }
        _ => {
            eprintln!("\n>>> UNKNOWN METHOD: {}", method);
            format!("RTSP/1.0 200 OK\r\nCSeq: {}\r\n\r\n", cseq)
        }
    }
}
