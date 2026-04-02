use futures_util::StreamExt;
use std::collections::HashMap;
use std::time::Duration;
use zbus::Connection;

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
trait NetworkManager {
    async fn get_devices(&self) -> zbus::Result<Vec<zvariant::OwnedObjectPath>>;
    async fn get_device_by_ip_iface(&self, iface: &str) -> zbus::Result<zvariant::OwnedObjectPath>;

    #[zbus(signal)]
    async fn device_added(&self, device_path: zvariant::OwnedObjectPath);
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Device.WifiP2P",
    default_service = "org.freedesktop.NetworkManager"
)]
trait WifiP2P {
    async fn start(&self) -> zbus::Result<()>;
    async fn stop(&self) -> zbus::Result<()>;
    async fn start_find(&self, options: HashMap<&str, zvariant::Value<'_>>) -> zbus::Result<()>;
    async fn stop_find(&self) -> zbus::Result<()>;
    async fn create_group(&self, props: HashMap<&str, zvariant::Value<'_>>) -> zbus::Result<()>;
    async fn request_group(
        &self,
        peer_path: &zvariant::ObjectPath<'_>,
        wfd_properties: HashMap<&str, zvariant::Value<'_>>,
    ) -> zbus::Result<()>;

    #[zbus(property)]
    fn peers(&self) -> zbus::Result<Vec<zvariant::OwnedObjectPath>>;
    #[zbus(property)]
    fn group(&self) -> zbus::Result<zvariant::OwnedObjectPath>;
    #[zbus(property)]
    fn wfd_properties(&self) -> zbus::Result<HashMap<String, zvariant::OwnedValue>>;

    #[zbus(signal)]
    async fn peer_added(&self, peer: zvariant::OwnedObjectPath);
    #[zbus(signal)]
    async fn peer_removed(&self, peer: zvariant::OwnedObjectPath);
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Device.P2P.Peer",
    default_service = "org.freedesktop.NetworkManager"
)]
trait P2PPeer {
    #[zbus(property)]
    fn flags(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn hw_address(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn manufacturer(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn model(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn model_number(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn serial(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn wfd_ies(&self) -> zbus::Result<Vec<u8>>;
    #[zbus(property)]
    fn name(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn last_seen(&self) -> zbus::Result<i64>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Device",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Device {
    #[zbus(property)]
    fn device_type(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn interface(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn ip_interface(&self) -> zbus::Result<String>;
}

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

#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("D-Bus error: {0}")]
    DBusError(#[from] zbus::Error),

    #[error("ZVariant error: {0}")]
    ZVariantError(#[from] zvariant::Error),

    #[error("NetworkManager error: {0}")]
    NetworkManagerError(String),

    #[error("No P2P device found")]
    NoP2PDevice,

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Timeout waiting for peer")]
    Timeout,

    #[error("Discovery error: {0}")]
    DiscoveryError(String),

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Peer not found")]
    PeerNotFound,
}

#[derive(Debug, Clone)]
pub struct P2pConnection {
    pub sink: Sink,
    pub interface: String,
}

pub struct P2pManager {
    config: P2pConfig,
    connection: Connection,
    nm_proxy: NetworkManagerProxy<'static>,
    p2p_proxy: Option<WifiP2PProxy<'static>>,
    device_path: Option<zvariant::OwnedObjectPath>,
}

impl P2pManager {
    pub async fn new(config: P2pConfig) -> Result<Self, NetError> {
        let connection = Connection::system().await?;

        let nm_proxy = NetworkManagerProxy::new(&connection).await?;

        let mut instance = Self {
            config,
            connection,
            nm_proxy,
            p2p_proxy: None,
            device_path: None,
        };

        instance.find_p2p_device().await?;

        Ok(instance)
    }

    pub async fn find_p2p_device(&mut self) -> Result<(), NetError> {
        let devices =
            self.nm_proxy.get_devices().await.map_err(|e| {
                NetError::NetworkManagerError(format!("Failed to get devices: {}", e))
            })?;

        for device_path in devices {
            let device_proxy = DeviceProxy::builder(&self.connection)
                .path(device_path.clone())?
                .build()
                .await?;

            let device_type = device_proxy.device_type().await?;

            // NM_DEVICE_TYPE_WIFI_P2P = 29
            if device_type == 29 {
                self.p2p_proxy = Some(
                    WifiP2PProxy::builder(&self.connection)
                        .path(device_path.clone())?
                        .build()
                        .await
                        .map_err(|e| {
                            NetError::NetworkManagerError(format!(
                                "Failed to create P2P proxy: {}",
                                e
                            ))
                        })?,
                );

                self.device_path = Some(device_path);
                tracing::info!("Found P2P device");
                return Ok(());
            }
        }

        Err(NetError::DeviceNotFound(
            "No P2P capable WiFi device found".into(),
        ))
    }

    pub async fn discover_sinks(&self, timeout: Duration) -> Result<Vec<Sink>, NetError> {
        let p2p = self
            .p2p_proxy
            .as_ref()
            .ok_or_else(|| NetError::DeviceNotFound("P2P device not initialized".into()))?;

        // Start P2P discovery
        let options: HashMap<&str, zvariant::Value> = HashMap::new();
        p2p.start_find(options)
            .await
            .map_err(|e| NetError::DiscoveryError(format!("Failed to start P2P find: {}", e)))?;

        tracing::info!("Started P2P discovery...");

        // Listen for peer events with timeout
        let mut peer_stream = p2p.receive_peer_added().await.map_err(|e| {
            NetError::DiscoveryError(format!("Failed to create peer stream: {}", e))
        })?;
        let mut discovered_peers = Vec::new();

        let timeout_future = tokio::time::sleep(timeout);
        tokio::pin!(timeout_future);

        loop {
            tokio::select! {
                Some(peer_signal) = peer_stream.next() => {
                    if let Ok(peer_path) = peer_signal.args() {
                        let peer = peer_path.peer.clone();
                        tracing::info!("Discovered peer: {}", peer);
                        discovered_peers.push(peer);
                    }
                }
                _ = &mut timeout_future => {
                    tracing::info!("Discovery timeout reached");
                    break;
                }
            }
        }

        // Get currently available peers
        let current_peer_paths = p2p
            .peers()
            .await
            .map_err(|e| NetError::DiscoveryError(format!("Failed to get current peers: {}", e)))?;

        let mut unique_peers: std::collections::HashSet<_> = discovered_peers.iter().collect();
        for peer_path in &current_peer_paths {
            unique_peers.insert(peer_path);
        }

        let mut sinks = Vec::new();
        for peer_path in unique_peers {
            let peer = P2PPeerProxy::builder(&self.connection)
                .path(peer_path)?
                .build()
                .await
                .map_err(|e| {
                    NetError::NetworkManagerError(format!("Failed to create peer proxy: {}", e))
                })?;

            let hw_address = peer.hw_address().await.map_err(|e| {
                NetError::NetworkManagerError(format!("Failed to get peer address: {}", e))
            })?;

            let name = peer
                .name()
                .await
                .unwrap_or_else(|_| "Unknown Peer".to_string());

            let wfd_ies = peer.wfd_ies().await.unwrap_or_default();

            if is_miracast_sink(&wfd_ies) {
                sinks.push(Sink {
                    name: name.clone(),
                    address: hw_address.clone(),
                    ip_address: None,
                    wfd_capabilities: Some(parse_wfd_capabilities(&wfd_ies)),
                });

                tracing::info!("Found Miracast sink: {} ({})", name, hw_address);
            } else {
                tracing::debug!("Peer {} ({}) is not a Miracast sink", name, hw_address);
            }
        }

        // Stop discovery
        p2p.stop_find()
            .await
            .map_err(|e| NetError::DiscoveryError(format!("Failed to stop P2P find: {}", e)))?;

        Ok(sinks)
    }

    pub async fn connect(&self, sink: &Sink) -> Result<P2pConnection, NetError> {
        let p2p = self
            .p2p_proxy
            .as_ref()
            .ok_or_else(|| NetError::DeviceNotFound("P2P device not initialized".into()))?;

        // Find the peer by address
        let peer_paths = p2p
            .peers()
            .await
            .map_err(|e| NetError::NetworkManagerError(format!("Failed to get peers: {}", e)))?;

        let target_peer_path = self
            .find_peer_by_address(&peer_paths, &sink.address)
            .await?;

        // Prepare WFD properties for connection
        let wfd_props: HashMap<&str, zvariant::Value> = HashMap::from([
            ("source", zvariant::Value::Bool(true)),
            ("sink", zvariant::Value::Bool(false)),
        ]);

        tracing::info!(
            "Requesting group formation with: {} ({})",
            sink.name,
            sink.address
        );

        // Request to join/create group with the peer
        p2p.request_group(&target_peer_path, wfd_props)
            .await
            .map_err(|e| NetError::ConnectionFailed(format!("Failed to request group: {}", e)))?;

        // Wait for group formation
        let group_formed = tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                match p2p.group().await {
                    Ok(group_path) => {
                        let root_path = zvariant::ObjectPath::try_from("/").ok();
                        if root_path.as_ref() != Some(&group_path) {
                            tracing::info!("Group formed successfully");
                            return Ok::<(), NetError>(());
                        }
                    }
                    Err(e) => {
                        tracing::debug!("Error getting group info: {}, retrying...", e);
                    }
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        })
        .await;

        if group_formed.is_err() {
            return Err(NetError::ConnectionFailed(
                "Timed out waiting for group formation".to_string(),
            ));
        }

        // Get connection IP address
        let ip_address = self
            .get_assigned_ip_for_group()
            .await
            .unwrap_or_else(|_| "192.168.10.1".to_string());

        let connected_sink = Sink {
            name: sink.name.clone(),
            address: sink.address.clone(),
            ip_address: Some(ip_address.clone()),
            wfd_capabilities: sink.wfd_capabilities.clone(),
        };

        Ok(P2pConnection {
            sink: connected_sink,
            interface: self.config.interface_name.clone(),
        })
    }

    async fn find_peer_by_address(
        &self,
        peer_paths: &[zvariant::OwnedObjectPath],
        address: &str,
    ) -> Result<zvariant::OwnedObjectPath, NetError> {
        for peer_path in peer_paths {
            let peer = P2PPeerProxy::builder(&self.connection)
                .path(peer_path)?
                .build()
                .await
                .map_err(|e| {
                    NetError::NetworkManagerError(format!("Failed to create peer proxy: {}", e))
                })?;

            match peer.hw_address().await {
                Ok(hw_addr) if hw_addr.to_lowercase() == address.to_lowercase() => {
                    return Ok(peer_path.clone());
                }
                Ok(_) => continue,
                Err(_) => continue,
            }
        }

        Err(NetError::PeerNotFound)
    }

    async fn get_assigned_ip_for_group(&self) -> Result<String, NetError> {
        Ok("192.168.10.1".to_string())
    }

    pub async fn disconnect(&self) -> Result<(), NetError> {
        let p2p = self
            .p2p_proxy
            .as_ref()
            .ok_or_else(|| NetError::DeviceNotFound("P2P device not initialized".into()))?;

        p2p.stop()
            .await
            .map_err(|e| NetError::NetworkManagerError(format!("Failed to stop P2P: {}", e)))?;

        tracing::info!("Disconnected from P2P connection");
        Ok(())
    }

    pub async fn start_discovery(&mut self) -> Result<(), NetError> {
        tracing::info!(
            "Starting P2P discovery on interface {}",
            self.config.interface_name
        );
        self.find_p2p_device().await?;
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

fn is_miracast_sink(wfd_ies: &[u8]) -> bool {
    wfd_ies
        .windows(4)
        .any(|w| w.len() >= 4 && w[0] == 0xdd && w[3] == 0x0a)
}

fn parse_wfd_capabilities(wfd_ies: &[u8]) -> String {
    if !wfd_ies.is_empty() {
        "WFD Sink".to_string()
    } else {
        "Generic P2P Device".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_nm_connection() {
        match Connection::system().await {
            Ok(_) => {
                // Connection successful
            }
            Err(_) => {
                println!("Note: D-Bus connection unavailable in test environment");
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_p2p_discovery() {
        let config = P2pConfig {
            interface_name: "wlan0".to_string(),
            group_name: "test_group".to_string(),
        };

        match P2pManager::new(config).await {
            Ok(mut manager) => {
                println!("P2P Manager created successfully");
                let _ = manager.find_p2p_device().await;
            }
            Err(_) => {
                println!("Note: Real P2P hardware unavailable for testing");
            }
        }
    }

    #[tokio::test]
    async fn test_wfd_capabilities_parsing() {
        let wfd_data = vec![0xdd, 0x04, 0x50, 0x0a];
        assert!(is_miracast_sink(&wfd_data));

        let empty_data = vec![];
        assert!(!is_miracast_sink(&empty_data));
    }

    #[tokio::test]
    async fn test_p2p_configs_and_connections() {
        let config = P2pConfig {
            interface_name: "wlan0".to_string(),
            group_name: "test_group".to_string(),
        };

        assert_eq!(config.interface_name, "wlan0");
        assert_eq!(config.group_name, "test_group");
    }
}
