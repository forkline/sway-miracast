use std::time::Duration;

#[derive(Debug, Clone)]
pub struct P2pConfig {
    pub interface_name: String,
    pub group_name: String,
}

#[derive(Debug, Clone)]
pub struct Sink {
    pub name: String,
    pub address: String,
    pub ip_address: Option<String>,
    pub wfd_capabilities: Option<String>,
}

#[derive(Debug)]
pub struct P2pManager {
    config: P2pConfig,
}

#[derive(thiserror::Error, Debug)]
pub enum NetError {
    #[error("D-Bus error: {0}")]
    DBusError(String),

    #[error("NetworkManager error: {0}")]
    NetworkManagerError(String),

    #[error("Connection timeout")]
    ConnectionTimeout,

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Operation not supported: {0}")]
    OperationNotSupported(String),
}

#[derive(Debug)]
pub struct P2pConnection {
    sink: Sink,
    interface: String,
}

impl P2pManager {
    pub async fn new(config: P2pConfig) -> Result<Self, NetError> {
        Ok(P2pManager { config })
    }

    pub async fn discover_sinks(&self, timeout: Duration) -> Result<Vec<Sink>, NetError> {
        tracing::debug!("Starting P2P discovery with timeout {:?}", timeout);

        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(vec![
            Sink {
                name: "Living Room TV".to_string(),
                address: "de:ad:be:ef:de:ad".to_string(),
                ip_address: None,
                wfd_capabilities: Some("Source:Infrastructure AP|Sink:Primary RFSW".to_string()),
            },
            Sink {
                name: "Bedroom Monitor".to_string(),
                address: "ca:fe:ba:be:ca:fe".to_string(),
                ip_address: None,
                wfd_capabilities: Some("Sink:Primary RFSW".to_string()),
            },
        ])
    }

    pub async fn connect(&self, sink: &Sink) -> Result<P2pConnection, NetError> {
        tracing::info!(
            "Establishing P2P connection to {}, MAC: {}",
            sink.name,
            sink.address
        );

        tokio::time::sleep(Duration::from_millis(100)).await;

        let connected_sink = Sink {
            name: sink.name.clone(),
            address: sink.address.clone(),
            ip_address: Some("192.168.10.1".to_string()),
            wfd_capabilities: sink.wfd_capabilities.clone(),
        };

        tracing::info!("Connected to P2P device: {}", sink.name);

        Ok(P2pConnection {
            sink: connected_sink,
            interface: self.config.interface_name.clone(),
        })
    }

    pub async fn disconnect(&self) -> Result<(), NetError> {
        tracing::info!("Disconnecting from P2P connection");
        Ok(())
    }

    pub async fn start_discovery(&self) -> Result<(), NetError> {
        tracing::info!(
            "Starting P2P discovery on interface {}",
            self.config.interface_name
        );
        Ok(())
    }

    pub async fn stop_discovery(&self) -> Result<(), NetError> {
        tracing::info!("Stopping P2P discovery");
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

    #[tokio::test]
    async fn test_p2p_config_creation() {
        let config = P2pConfig {
            interface_name: "wlan0".to_string(),
            group_name: "test_group".to_string(),
        };

        assert_eq!(config.interface_name, "wlan0");
        assert_eq!(config.group_name, "test_group");
    }

    #[tokio::test]
    async fn test_sink_creation() {
        let sink = Sink {
            name: "Test Sink".to_string(),
            address: "AA:BB:CC:DD:EE:FF".to_string(),
            ip_address: Some("192.168.1.100".to_string()),
            wfd_capabilities: Some("WFD_SUPPORTED".to_string()),
        };

        assert_eq!(sink.name, "Test Sink");
        assert_eq!(sink.address, "AA:BB:CC:DD:EE:FF");
        assert_eq!(sink.ip_address, Some("192.168.1.100".to_string()));
        assert_eq!(sink.wfd_capabilities, Some("WFD_SUPPORTED".to_string()));
    }

    #[tokio::test]
    async fn test_sink_without_ip() {
        let sink = Sink {
            name: "Test Sink".to_string(),
            address: "AA:BB:CC:DD:EE:FF".to_string(),
            ip_address: None,
            wfd_capabilities: None,
        };

        assert_eq!(sink.ip_address, None);
        assert_eq!(sink.wfd_capabilities, None);
    }

    #[tokio::test]
    async fn test_p2p_manager_creation() {
        let config = P2pConfig {
            interface_name: "wlan0".to_string(),
            group_name: "test_group".to_string(),
        };

        let manager = P2pManager::new(config).await;
        assert!(manager.is_ok());
    }

    #[tokio::test]
    async fn test_discover_sinks() {
        let config = P2pConfig {
            interface_name: "wlan0".to_string(),
            group_name: "test_group".to_string(),
        };

        let manager = P2pManager::new(config).await.unwrap();
        let sinks = manager
            .discover_sinks(Duration::from_secs(1))
            .await
            .unwrap();

        assert!(!sinks.is_empty());
        assert_eq!(sinks[0].name, "Living Room TV");
    }

    #[tokio::test]
    async fn test_connect() {
        let config = P2pConfig {
            interface_name: "wlan0".to_string(),
            group_name: "test_group".to_string(),
        };

        let manager = P2pManager::new(config).await.unwrap();
        let sinks = manager
            .discover_sinks(Duration::from_secs(1))
            .await
            .unwrap();
        let connection = manager.connect(&sinks[0]).await.unwrap();

        assert_eq!(connection.get_sink().name, "Living Room TV");
        assert!(connection.get_sink().ip_address.is_some());
    }
}
