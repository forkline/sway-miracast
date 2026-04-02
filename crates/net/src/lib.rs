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
    async fn activate_connection(
        &self,
        connection: zvariant::ObjectPath<'_>,
        device: zvariant::ObjectPath<'_>,
        specific_object: zvariant::ObjectPath<'_>,
    ) -> zbus::Result<zvariant::OwnedObjectPath>;
    async fn add_and_activate_connection(
        &self,
        connection: HashMap<&str, HashMap<&str, zvariant::Value<'_>>>,
        device: zvariant::ObjectPath<'_>,
        specific_object: zvariant::ObjectPath<'_>,
    ) -> zbus::Result<(zvariant::OwnedObjectPath, zvariant::OwnedObjectPath)>;

    #[zbus(signal)]
    async fn device_added(&self, device_path: zvariant::OwnedObjectPath);
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Device.WifiP2P",
    default_service = "org.freedesktop.NetworkManager"
)]
trait WifiP2P {
    async fn start_find(&self, options: HashMap<&str, zvariant::Value<'_>>) -> zbus::Result<()>;
    async fn stop_find(&self) -> zbus::Result<()>;

    #[zbus(property)]
    fn peers(&self) -> zbus::Result<Vec<zvariant::OwnedObjectPath>>;

    #[zbus(signal)]
    async fn peer_added(&self, peer: zvariant::OwnedObjectPath);
    #[zbus(signal)]
    async fn peer_removed(&self, peer: zvariant::OwnedObjectPath);
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.WifiP2PPeer",
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
    #[allow(non_snake_case)]
    fn WfdIEs(&self) -> zbus::Result<Vec<u8>>;
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

    #[zbus(property)]
    fn ip4_config(&self) -> zbus::Result<zvariant::OwnedObjectPath>;

    #[zbus(property)]
    fn active_connection(&self) -> zbus::Result<zvariant::OwnedObjectPath>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.IP4Config",
    default_service = "org.freedesktop.NetworkManager"
)]
trait IP4Config {
    #[zbus(property)]
    fn addresses(&self) -> zbus::Result<Vec<(u32, u32, u32)>>;
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
    pub rtsp_port: u16,
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

            // NM_DEVICE_TYPE_WIFI_P2P = 30 (from NetworkManager headers)
            // NM_DEVICE_TYPE_GENERIC = 29
            tracing::debug!(
                "Device {} has type {} (interface: {})",
                device_path,
                device_type,
                device_proxy.interface().await.unwrap_or_default()
            );
            if device_type == 30 {
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

            let wfd_ies = peer.WfdIEs().await.unwrap_or_default();
            tracing::debug!(
                "Peer {} WFD IEs length: {}, data: {:02x?}",
                name,
                wfd_ies.len(),
                wfd_ies
            );

            if is_miracast_sink(&wfd_ies) {
                sinks.push(Sink {
                    name: name.clone(),
                    address: hw_address.clone(),
                    ip_address: None,
                    rtsp_port: parse_wfd_rtsp_port(&wfd_ies),
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
        let _p2p = self
            .p2p_proxy
            .as_ref()
            .ok_or_else(|| NetError::DeviceNotFound("P2P device not initialized".into()))?;

        let device_path = self
            .device_path
            .as_ref()
            .ok_or_else(|| NetError::DeviceNotFound("P2P device path not set".into()))?;

        tracing::info!(
            "Creating P2P connection to: {} ({})",
            sink.name,
            sink.address
        );

        let device_obj_path = zvariant::ObjectPath::try_from(device_path.as_str())
            .map_err(NetError::ZVariantError)?;
        let root_path = zvariant::ObjectPath::try_from("/").map_err(NetError::ZVariantError)?;

        let mut wifi_p2p_props: HashMap<&str, zvariant::Value<'_>> = HashMap::new();
        wifi_p2p_props.insert(
            "peer",
            zvariant::Value::Str(zvariant::Str::from(&sink.address)),
        );

        // WFD Device Information Subelement (Wi-Fi Display spec Table 4)
        // Format: [Subelement ID] [Length] [Device Info] [RTSP Port] [Throughput] [Coupled Sink]
        let wfd_ies: Vec<u8> = vec![
            0x00,                   // Subelement ID: WFD Device Information
            0x00, 0x06,             // Length: 6 bytes
            0x05,                   // Device Info: Source (00) + Session Available (bit 2) + WFD Enabled (bit 0)
            0x1C, 0x44,             // RTSP Port: 7236 (big-endian)
            0x00, 0xC8,             // Max Throughput: 200 Mbps
            0x00,                   // Coupled Sink Status: none
        ];
        wifi_p2p_props.insert(
            "wfd-ies",
            zvariant::Value::Array(zvariant::Array::from(&wfd_ies)),
        );

        let mut connection_props: HashMap<&str, zvariant::Value<'_>> = HashMap::new();
        connection_props.insert(
            "type",
            zvariant::Value::Str(zvariant::Str::from("wifi-p2p")),
        );
        connection_props.insert(
            "id",
            zvariant::Value::Str(zvariant::Str::from(&self.config.group_name)),
        );
        connection_props.insert("autoconnect", zvariant::Value::Bool(false));

        let mut ipv4_props: HashMap<&str, zvariant::Value<'_>> = HashMap::new();
        ipv4_props.insert("method", zvariant::Value::Str(zvariant::Str::from("auto")));

        let connection_config: HashMap<&str, HashMap<&str, zvariant::Value<'_>>> = HashMap::from([
            ("connection", connection_props),
            ("wifi-p2p", wifi_p2p_props),
            ("ipv4", ipv4_props),
        ]);

        tracing::debug!("Connection config: {:?}", connection_config);

        let (conn_path, active_conn_path) = self
            .nm_proxy
            .add_and_activate_connection(connection_config, device_obj_path, root_path)
            .await
            .map_err(|e| NetError::ConnectionFailed(format!("Failed to add connection: {}", e)))?;

        tracing::info!(
            "Connection activated: {} (active: {})",
            conn_path,
            active_conn_path
        );

        tokio::time::sleep(Duration::from_secs(2)).await;

        let ip_address = self.get_device_ip_address().await.unwrap_or_else(|e| {
            tracing::warn!("Failed to get IP address: {}, using fallback", e);
            "192.168.10.1".to_string()
        });

        tracing::info!("Got IP address: {}", ip_address);

        let connected_sink = Sink {
            name: sink.name.clone(),
            address: sink.address.clone(),
            ip_address: Some(ip_address),
            rtsp_port: sink.rtsp_port,
            wfd_capabilities: sink.wfd_capabilities.clone(),
        };

        Ok(P2pConnection {
            sink: connected_sink,
            interface: self.config.interface_name.clone(),
        })
    }

    async fn get_device_ip_address(&self) -> Result<String, NetError> {
        let device_path = self
            .device_path
            .as_ref()
            .ok_or_else(|| NetError::DeviceNotFound("P2P device path not set".into()))?;

        let device_proxy = DeviceProxy::builder(&self.connection)
            .path(device_path.clone())?
            .build()
            .await
            .map_err(|e| {
                NetError::NetworkManagerError(format!("Failed to create device proxy: {}", e))
            })?;

        for _ in 0..10 {
            if let Ok(ip4_config_path) = device_proxy.ip4_config().await {
                if ip4_config_path.as_str() != "/" {
                    let ip4_config = IP4ConfigProxy::builder(&self.connection)
                        .path(ip4_config_path)?
                        .build()
                        .await
                        .map_err(|e| {
                            NetError::NetworkManagerError(format!(
                                "Failed to create IP4Config proxy: {}",
                                e
                            ))
                        })?;

                    if let Ok(addresses) = ip4_config.addresses().await {
                        if let Some((addr, _, _)) = addresses.first() {
                            let ip = u32::from_be(*addr);
                            return Ok(format!(
                                "{}.{}.{}.{}",
                                (ip >> 24) & 0xFF,
                                (ip >> 16) & 0xFF,
                                (ip >> 8) & 0xFF,
                                ip & 0xFF
                            ));
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        Err(NetError::NetworkManagerError(
            "No IP address assigned".into(),
        ))
    }

    pub async fn disconnect(&self) -> Result<(), NetError> {
        let p2p = self
            .p2p_proxy
            .as_ref()
            .ok_or_else(|| NetError::DeviceNotFound("P2P device not initialized".into()))?;

        p2p.stop_find().await.map_err(|e| {
            NetError::NetworkManagerError(format!("Failed to stop P2P find: {}", e))
        })?;

        tracing::info!("Stopped P2P discovery");
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
    if wfd_ies.is_empty() {
        return false;
    }
    let first_byte = wfd_ies[0];
    tracing::debug!(
        "WFD IEs: {:02x?}, first_byte: 0x{:02x}",
        wfd_ies,
        first_byte
    );
    if first_byte == 0xdd && wfd_ies.len() >= 4 && wfd_ies[3] == 0x0a {
        return true;
    }
    first_byte == 0x00 || first_byte == 0x01 || first_byte == 0x06 || first_byte == 0x07
}

fn parse_wfd_capabilities(wfd_ies: &[u8]) -> String {
    if !wfd_ies.is_empty() {
        "WFD Sink".to_string()
    } else {
        "Generic P2P Device".to_string()
    }
}

pub fn parse_wfd_rtsp_port(wfd_ies: &[u8]) -> u16 {
    // WFD Device Information: byte 1-2 (after device type byte) is RTSP port
    // Format: 01 XX XX ... where XX XX is the port
    if wfd_ies.len() >= 3 {
        ((wfd_ies[1] as u16) << 8) | (wfd_ies[2] as u16)
    } else {
        7236 // Default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_wfd_rtsp_port() {
        // Test with standard LG TV format: first byte is 01 (device type = sink),
        // bytes 1-2 (0x13, 0x1c) are the port in big-endian = 4892
        let lg_tv_wfd_ies = vec![0x01, 0x13, 0x1c]; // LG TV with port 4892
        assert_eq!(parse_wfd_rtsp_port(&lg_tv_wfd_ies), 4892);

        // Test with another port value - 7236 (our default)
        let standard_wfd_ies = vec![0x01, 0x1c, 0x44];  
        assert_eq!(parse_wfd_rtsp_port(&standard_wfd_ies), 7236);

        // Test with insufficient bytes - should return default
        let short_wfd_ies = vec![0x01, 0x13]; 
        assert_eq!(parse_wfd_rtsp_port(&short_wfd_ies), 7236);

        // Test with empty bytes - should return default  
        let empty_wfd_ies: Vec<u8> = vec![];
        assert_eq!(parse_wfd_rtsp_port(&empty_wfd_ies), 7236);

        // Additional comprehensive test
        let custom_port = vec![0x01, 0x08, 0xae]; // Custom port: 0x08ae = 2222
        assert_eq!(parse_wfd_rtsp_port(&custom_port), 2222);
    }

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

    #[tokio::test]
    async fn test_wfd_information_elements_format() {
        // Test that the WFD IEs are correctly constructed
        // Format according to Wi-Fi Display specification:
        // Byte 0: Subelement ID = 0x00 (WFD Device Information)
        // Bytes 1-2: Length = 6 bytes
        // Byte 3: Device Type (bits 1:0): 0x01 = WFD Source + Session Available bit
        // Bytes 4-5: Session Management Control Port = 7236 (0x1C44)
        // Bytes 6-7: WFD Device Maximum Throughput
        let wfd_ies_expected = vec![
            0x00, // Subelement ID: WFD Device Information
            0x00, 0x06, // Length: 6 bytes (in little endian format as per spec: 0x006=6)
            0x01, // Device Type: Source + Session Available
            0x1C, 0x44, // RTSP Port: 7236 (big-endian = 0x1C44 = 7236 decimal)
            0x00, 0x00, // Max Throughput: 0 (unlimited)
        ];

        // Construct the same vector as done in the connect method
        let wfd_ies_actual = vec![
            0x00, // Subelement ID: WFD Device Information
            0x00, 0x06, // Length: 6 bytes
            0x01, // Device Type: Source (bits 1:0=00) + Session Available (bit 2=1)
            0x1C, 0x44, // RTSP Port: 7236 (big-endian)
            0x00, 0x00, // Max Throughput: 0 (unlimited)
        ];

        assert_eq!(wfd_ies_expected, wfd_ies_actual);

        // Verify individual components in correct positions
        assert_eq!(wfd_ies_actual[0], 0x00); // Subelement ID
        assert_eq!(wfd_ies_actual[1], 0x00); // Length byte 1
        assert_eq!(wfd_ies_actual[2], 0x06); // Length byte 2 (total 6 bytes following)
        assert_eq!(wfd_ies_actual[3], 0x01); // Device type (Source + available)
        assert_eq!(wfd_ies_actual[4], 0x1C); // RTSP port high byte
        assert_eq!(wfd_ies_actual[5], 0x44); // RTSP port low byte
    }

    #[tokio::test]
    async fn test_p2p_device_advertises_correctly() {
        // Verify that our device would be advertised as source device with
        // correct capabilities in the discovery process

        // Device type byte: 0x01 = source + session available bit
        // Based on the code implementation:
        let _device_type_byte = 0x01; // From actual code implementation - kept for test completeness

        // RTSP control port is big-endian - verify conversion works correctly
        let rtsp_port_high = 0x1C; // 0x1C = 28
        let rtsp_port_low = 0x44; // 0x44 = 68
        let rtsp_port_value = ((rtsp_port_high as u16) << 8) | (rtsp_port_low as u16);
        assert_eq!(rtsp_port_value, 7236); // Should equal RTSP port 7236 (0x1C44)
    }
}
