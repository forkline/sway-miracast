use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::json;
use std::time::Duration;
use tabled::{Table, Tabled};

use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "swaybeam")]
#[command(about = "Miracast source for wlroots-based compositors")]
struct Cli {
    #[arg(long)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Doctor,
    Discover {
        #[arg(short, long, default_value = "10")]
        timeout: u64,
    },
    Connect {
        #[arg(short, long)]
        sink: String,
    },
    Stream {
        #[arg(long, default_value = "1920")]
        width: u32,
        #[arg(long, default_value = "1080")]
        height: u32,
        #[arg(long, default_value = "30")]
        framerate: u32,
    },
    Disconnect,
    Daemon {
        #[arg(short, long)]
        sink: Option<String>,
        #[arg(short, long)]
        client: bool,
    },
    Status,
}

#[derive(Tabled)]
struct SinkRow {
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Address")]
    address: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match &cli.command {
        Command::Doctor => doctor_command(cli.json).await,
        Command::Discover { timeout } => discover_command(*timeout, cli.json).await,
        Command::Connect { sink } => connect_command(sink, cli.json).await,
        Command::Stream {
            width,
            height,
            framerate,
        } => stream_command(*width, *height, *framerate, cli.json).await,
        Command::Disconnect => disconnect_command(cli.json).await,
        Command::Daemon { sink, client } => daemon_command(sink.clone(), *client, cli.json).await,
        Command::Status => status_command(cli.json).await,
    }
}

async fn doctor_command(json_output: bool) -> Result<()> {
    use swaybeam_doctor::check_all;

    if json_output {
        let report = check_all()?;
        let output = json!({
            "all_ok": report.all_ok(),
            "checks": {
                "sway": report.sway_result.message,
                "pipewire": report.pipewire_result.message,
                "gstreamer": report.gstreamer_result.message,
                "network_manager": report.network_manager_result.message,
            }
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("Running system capability checks...\n");
        let report = check_all()?;
        report.print();
    }

    Ok(())
}

async fn discover_command(timeout: u64, json_output: bool) -> Result<()> {
    use swaybeam_net::{P2pConfig, P2pManager};

    let config = P2pConfig {
        interface_name: "wlan0".to_string(),
        group_name: "swaybeam".to_string(),
    };

    let manager = P2pManager::new(config).await?;
    let devices = manager.discover_sinks(Duration::from_secs(timeout)).await?;

    if json_output {
        let output = json!({
            "devices": devices.iter().map(|d| json!({
                "name": &d.name,
                "address": &d.address,
                "ip_address": &d.ip_address
            })).collect::<Vec<_>>(),
            "count": devices.len()
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("Discovering Miracast sinks for {}s...\n", timeout);

        if devices.is_empty() {
            println!("No Miracast devices found.");
            return Ok(());
        }

        let rows: Vec<SinkRow> = devices
            .iter()
            .map(|d| SinkRow {
                name: d.name.clone(),
                address: d.address.clone(),
            })
            .collect();

        println!("{}", Table::new(rows));
        println!("\nFound {} device(s)", devices.len());
    }

    Ok(())
}

async fn connect_command(sink_param: &str, json_output: bool) -> Result<()> {
    use swaybeam_net::{P2pConfig, P2pManager};

    let config = P2pConfig {
        interface_name: "wlan0".to_string(),
        group_name: "swaybeam".to_string(),
    };

    let manager = P2pManager::new(config).await?;
    let devices = manager.discover_sinks(Duration::from_secs(5)).await?;

    let target = devices
        .into_iter()
        .find(|d| d.name == sink_param || d.address == sink_param);

    match target {
        Some(device) => {
            let connection = manager.connect(&device).await?;

            if json_output {
                let output = json!({
                    "status": "connected",
                    "sink": {
                        "name": connection.get_sink().name,
                        "address": connection.get_sink().address,
                        "ip_address": connection.get_sink().ip_address,
                    }
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!("Connected to {}", sink_param);
                println!("   Address: {}", connection.get_sink().address);
                if let Some(ref ip) = connection.get_sink().ip_address {
                    println!("   IP: {}", ip);
                }
            }
        }
        None => {
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "status": "error",
                        "message": format!("Sink '{}' not found", sink_param)
                    }))?
                );
            } else {
                eprintln!("Sink '{}' not found", sink_param);
            }
        }
    }

    Ok(())
}

async fn stream_command(width: u32, height: u32, framerate: u32, json_output: bool) -> Result<()> {
    use swaybeam_stream::{StreamConfig, StreamPipeline};

    let config = StreamConfig {
        video_width: width,
        video_height: height,
        video_framerate: framerate,
        ..Default::default()
    };

    let pipeline = StreamPipeline::new(config)?;
    pipeline.set_output("127.0.0.1", 5004).await?;

    if json_output {
        let output = json!({
            "status": "ready",
            "video": {
                "resolution": format!("{}x{}", width, height),
                "framerate": framerate
            }
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("Stream pipeline configured:");
        println!("   Resolution: {}x{}", width, height);
        println!("   Framerate: {} fps", framerate);
    }

    Ok(())
}

async fn disconnect_command(json_output: bool) -> Result<()> {
    use swaybeam_net::{P2pConfig, P2pManager};

    let config = P2pConfig {
        interface_name: "wlan0".to_string(),
        group_name: "swaybeam".to_string(),
    };

    let manager = P2pManager::new(config).await?;
    manager.disconnect().await?;

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "status": "disconnected"
            }))?
        );
    } else {
        println!("Disconnected from Miracast sink");
    }

    Ok(())
}

async fn daemon_command(sink: Option<String>, client_mode: bool, _json_output: bool) -> Result<()> {
    use swaybeam_daemon::{Daemon, DaemonConfig};

    println!("Starting Miracast daemon...");
    if client_mode {
        println!("Running in RTSP client mode (TV is Group Owner)");
    }
    let config = DaemonConfig {
        preferred_sink: sink,
        force_client_mode: client_mode,
        ..Default::default()
    };
    let mut daemon = Daemon::with_config(config);

    if let Err(e) = daemon.run().await {
        eprintln!("Daemon error: {}", e);
    }

    Ok(())
}

async fn status_command(json_output: bool) -> Result<()> {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "connected": false,
                "streaming": false,
                "sink": null
            }))?
        );
    } else {
        println!("Status:");
        println!("   Connected: No");
        println!("   Streaming: No");
        println!("   Current sink: None");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        let cmd = Cli::try_parse_from(["swaybeam", "doctor"]);
        assert!(cmd.is_ok());

        let cmd = Cli::try_parse_from(["swaybeam", "discover"]);
        assert!(cmd.is_ok());

        let cmd = Cli::try_parse_from(["swaybeam", "connect", "-s", "TestSink"]);
        assert!(cmd.is_ok());

        let cmd = Cli::try_parse_from(["swaybeam", "stream"]);
        assert!(cmd.is_ok());

        let cmd = Cli::try_parse_from(["swaybeam", "disconnect"]);
        assert!(cmd.is_ok());

        let cmd = Cli::try_parse_from(["swaybeam", "daemon"]);
        assert!(cmd.is_ok());

        let cmd = Cli::try_parse_from(["swaybeam", "status"]);
        assert!(cmd.is_ok());
    }
}
