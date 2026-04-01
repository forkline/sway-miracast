use miracast_net::{P2pConfig, P2pManager, Sink};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure P2P manager
    let config = P2pConfig {
        interface_name: "wlan0".to_string(), // Assuming wlan0 is available
        group_name: "miracast_group".to_string(),
    };

    // Create P2P manager
    let p2p_manager = P2pManager::new(config)?;

    println!("Discovering Miracast sinks...");
    let sinks = p2p_manager.discover_sinks(Duration::from_secs(10))?;

    if sinks.is_empty() {
        println!("No Miracast sinks found.");
        return Ok(());
    }

    println!("Found {} potential sink(s):", sinks.len());
    for (i, sink) in sinks.iter().enumerate() {
        println!("{}: {} ({})", i + 1, sink.name, sink.address);
    }

    // Connect to the first available sink as an example
    if let Some(first_sink) = sinks.first() {
        println!("\nAttempting to connect to: {}", first_sink.name);

        match p2p_manager.connect(first_sink) {
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
                    println!("No IP assigned");
                }

                // Disconnect after connection
                println!("Disconnecting from sink...");
                p2p_manager.disconnect()?;
                println!("Disconnected successfully.");
            }
            Err(e) => {
                eprintln!("Failed to connect: {}", e);
            }
        }
    }

    Ok(())
}
