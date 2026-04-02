use std::time::Duration;
use swaybeam_net::{P2pConfig, P2pManager};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure P2P manager
    let config = P2pConfig {
        interface_name: "wlan0".to_string(), // Assuming wlan0 is available
        group_name: "swaybeam_group".to_string(),
    };

    // Create P2P manager
    let mut p2p_manager = P2pManager::new(config).await?;

    println!("Starting WiFiP2P discovery...");
    p2p_manager.start_discovery().await?; // Updated to match method name in new implementation

    println!("Discovering Miracast sinks...");
    let sinks = p2p_manager.discover_sinks(Duration::from_secs(10)).await?;

    // Stop discovery after we're done
    p2p_manager.stop_discovery().await?; // Updated to match method name in new implementation

    if sinks.is_empty() {
        println!("No Miracast sinks found.");
        return Ok(());
    }

    println!("Found {} potential sink(s):", sinks.len());
    for (i, sink) in sinks.iter().enumerate() {
        println!("{}: {} ({})", i + 1, sink.name, sink.address);
        if let Some(wfd_caps) = &sink.wfd_capabilities {
            println!("    WFD Capabilities: {}", wfd_caps);
        }
    }

    // Connect to the first available sink as an example
    if let Some(first_sink) = sinks.first() {
        println!("\nAttempting to connect to: {}", first_sink.name);

        match p2p_manager.connect(first_sink).await {
            Ok(connection) => {
                println!("Successfully connected!");
                println!(
                    "Connected sink: {} ({})",
                    connection.get_sink().name,
                    connection.get_sink().address
                );

                if let Some(ip) = &connection.get_sink().ip_address {
                    println!("Assigned IP: {}", ip);
                } else {
                    println!("No IP assigned yet");
                }

                if let Some(wfd_caps) = &connection.get_sink().wfd_capabilities {
                    println!("WFD Capabilities: {}", wfd_caps);
                }

                // Disconnect after connection
                println!("Disconnecting from sink...");
                p2p_manager.disconnect().await?;
                println!("Disconnected successfully.");
            }
            Err(e) => {
                eprintln!("Failed to connect: {}", e);
            }
        }
    }

    Ok(())
}
