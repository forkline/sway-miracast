use std::process::Command;
use std::time::Duration;

#[derive(Debug)]
pub struct P2pManager {
    config: P2pConfig,
}

#[derive(Debug, Clone)]
pub struct Sink {
    pub name: String,
    pub address: String,
    pub ip_address: Option<String>,
}

#[derive(Debug)]
pub struct P2pConfig {
    pub interface_name: String,
    pub group_name: String,
}

#[derive(thiserror::Error, Debug)]
pub enum NetError {
    #[error("Command execution failed: {0}")]
    CommandFailed(String),

    #[error("NetworkManager/dbus error: {0}")]
    NetworkManagerError(String),

    #[error("Connection timeout")]
    ConnectionTimeout,

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    ParseError(String),
}

pub struct P2pConnection {
    sink: Sink,
    interface: String,
}

impl P2pManager {
    pub fn new(config: P2pConfig) -> Result<Self, NetError> {
        // Validate that the interface exists
        Self::validate_interface(&config.interface_name)?;

        Ok(P2pManager { config })
    }

    fn validate_interface(interface: &str) -> Result<(), NetError> {
        let output = Command::new("nmcli").args(["device", "status"]).output()?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        if !output_str.contains(interface) {
            return Err(NetError::DeviceNotFound(format!(
                "Interface {} not found",
                interface
            )));
        }

        Ok(())
    }

    pub fn discover_sinks(&self, _timeout: Duration) -> Result<Vec<Sink>, NetError> {
        // In practice, we'd implement timeout logic with a subprocess that times out,
        // but for now, we'll just pass the timeout to potentially run nmcli with a timeout

        // Use nmcli to scan for available P2P devices
        let output = Command::new("nmcli")
            .args(["device", "wifi", "rescan"]) // Rescan for fresh results
            .output()
            .map_err(|e| {
                NetError::CommandFailed(format!("Failed to execute nmcli rescan: {}", e))
            })?;

        if !output.status.success() {
            return Err(NetError::CommandFailed(format!(
                "nmcli rescan failed with status {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        // Give some time for the rescan to happen
        std::thread::sleep(std::time::Duration::from_millis(500));

        let output = Command::new("nmcli")
            .args(["device", "wifi", "list", "--fields", "NAME,DEVICE,MAC,BARS"])
            .output()
            .map_err(|e| NetError::CommandFailed(format!("Failed to execute nmcli: {}", e)))?;

        if !output.status.success() {
            return Err(NetError::CommandFailed(format!(
                "nmcli failed with status {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8(output.stdout)
            .map_err(|e| NetError::ParseError(format!("Failed to parse nmcli output: {}", e)))?;

        self.parse_discovered_devices(&output_str)
    }

    fn parse_discovered_devices(&self, output: &str) -> Result<Vec<Sink>, NetError> {
        let mut sinks = Vec::new();

        // Skip the header line and process each device
        for line in output.lines().skip(1) {
            if line.trim().is_empty() {
                continue;
            }

            // Parse typical nmcli output format: NAME DEVICE MAC BARS
            let parts: Vec<&str> = line.split_whitespace().collect();

            if parts.len() >= 3 {
                // In typical nmcli, name is first field, mac is typically third field
                let name = parts[0].to_string();
                let mac = parts[2].to_string();

                if !mac.starts_with(|c: char| c.is_ascii_hexdigit() || c == ':')
                    || !mac.contains(':')
                    || mac.len() != 17
                {
                    continue; // Skip invalid MAC addresses
                }

                sinks.push(Sink {
                    name,
                    address: mac,
                    ip_address: None,
                });
            }
        }

        Ok(sinks)
    }

    pub fn connect(&self, sink: &Sink) -> Result<P2pConnection, NetError> {
        // First check if we're already connected to avoid duplicate attempts
        if self.is_device_connected(&sink.address)? {
            return Err(NetError::NetworkManagerError(
                "Device already connected".to_string(),
            ));
        }

        // Use nmcli to connect to P2P device using its name
        let output = Command::new("nmcli")
            .arg("device")
            .arg("wifi")
            .arg("connect")
            .arg(&sink.name)
            .output()
            .map_err(|e| {
                NetError::CommandFailed(format!(
                    "Failed to execute nmcli connecting to {}: {}",
                    sink.name, e
                ))
            })?;

        if !output.status.success() {
            return Err(NetError::CommandFailed(format!(
                "Connecting to {} failed: {}",
                sink.name,
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        // Get IP address assigned after connection
        let ip_address = self.get_ip_address()?;

        // Update the sink with the IP address (but clone since we take a ref)
        let connected_sink = Sink {
            name: sink.name.clone(),
            address: sink.address.clone(),
            ip_address,
        };

        Ok(P2pConnection {
            sink: connected_sink,
            interface: self.config.interface_name.clone(),
        })
    }

    fn is_device_connected(&self, address: &str) -> Result<bool, NetError> {
        let output = Command::new("nmcli")
            .arg("device")
            .arg("status")
            .output()
            .map_err(|e| {
                NetError::CommandFailed(format!("Failed to check device status: {}", e))
            })?;

        if !output.status.success() {
            return Err(NetError::CommandFailed(
                "Failed to get device status".to_string(),
            ));
        }

        let output_str = String::from_utf8(output.stdout)
            .map_err(|e| NetError::ParseError(format!("Failed to parse nmcli output: {}", e)))?;

        Ok(output_str.contains(address))
    }

    fn get_ip_address(&self) -> Result<Option<String>, NetError> {
        let output = Command::new("nmcli")
            .arg("device")
            .arg("show")
            .arg(&self.config.interface_name)
            .output()
            .map_err(|e| {
                NetError::CommandFailed(format!("Failed to get IP address for interface: {}", e))
            })?;

        if !output.status.success() {
            return Err(NetError::CommandFailed(
                "Failed to show device info for IP address retrieval".to_string(),
            ));
        }

        let output_str = String::from_utf8(output.stdout)
            .map_err(|e| NetError::ParseError(format!("Failed to parse nmcli output: {}", e)))?;

        // Look for IP address in output (format is usually 'IP4.ADDRESS[1]:...'
        for line in output_str.lines() {
            if line.trim().starts_with("IP4.ADDRESS") {
                // Extract IP address from format like "IP4.ADDRESS[1]:                         192.168.1.100/24"
                if let Some(after_colon) = line.find(':') {
                    let ip_part = &line[after_colon + 1..].trim();
                    let ip = ip_part.split('/').next().unwrap_or("").trim().to_string();

                    if !ip.is_empty() && ip != "-" {
                        return Ok(Some(ip));
                    }
                }
            }
        }

        Ok(None) // No assigned IP
    }

    pub fn disconnect(&self) -> Result<(), NetError> {
        let output = Command::new("nmcli")
            .arg("device")
            .arg("disconnect")
            .arg(&self.config.interface_name)
            .output()
            .map_err(|e| {
                NetError::CommandFailed(format!("Failed to disconnect interface: {}", e))
            })?;

        if !output.status.success() {
            return Err(NetError::CommandFailed(format!(
                "Disconnecting interface {} failed: {}",
                self.config.interface_name,
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }
}

impl P2pConnection {
    pub fn get_sink(&self) -> &Sink {
        &self.sink
    }

    pub fn get_interface(&self) -> &str {
        &self.interface
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_p2p_config_creation() {
        let config = P2pConfig {
            interface_name: "wlan0".to_string(),
            group_name: "test_group".to_string(),
        };

        assert_eq!(config.interface_name, "wlan0");
        assert_eq!(config.group_name, "test_group");
    }

    #[test]
    fn test_sink_creation() {
        let sink = Sink {
            name: "Test Sink".to_string(),
            address: "AA:BB:CC:DD:EE:FF".to_string(),
            ip_address: Some("192.168.1.100".to_string()),
        };

        assert_eq!(sink.name, "Test Sink");
        assert_eq!(sink.address, "AA:BB:CC:DD:EE:FF");
        assert_eq!(sink.ip_address, Some("192.168.1.100".to_string()));
    }

    #[test]
    fn test_sink_without_ip() {
        let sink = Sink {
            name: "Test Sink".to_string(),
            address: "AA:BB:CC:DD:EE:FF".to_string(),
            ip_address: None,
        };

        assert_eq!(sink.ip_address, None);
    }
}
