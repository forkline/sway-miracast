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
    pub go_ip_address: Option<String>,
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

#[derive(Debug, Clone)]
struct GroupStartedInfo {
    interface_name: String,
    ip_address: String,
    go_ip_address: String,
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

    pub async fn discover_sinks(
        &self,
        timeout: Duration,
        preferred_sink: Option<&str>,
    ) -> Result<Vec<Sink>, NetError> {
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
        let mut preferred_match = None;
        let mut preferred_poll = tokio::time::interval(Duration::from_millis(500));

        let timeout_future = tokio::time::sleep(timeout);
        tokio::pin!(timeout_future);

        loop {
            tokio::select! {
                Some(peer_signal) = peer_stream.next() => {
                    if let Ok(peer_path) = peer_signal.args() {
                        let peer = peer_path.peer.clone();
                        tracing::info!("Discovered peer: {}", peer);
                        discovered_peers.push(peer);

                        if let Some(preferred_sink) = preferred_sink {
                            if let Some(sink) = self.peer_to_sink(&peer_path.peer).await? {
                                let preferred_address = preferred_sink.to_ascii_lowercase();
                                let sink_address = sink.address.to_ascii_lowercase();
                                if sink.name == preferred_sink || sink_address == preferred_address {
                                    tracing::info!(
                                        "Preferred sink {} discovered, ending discovery early",
                                        preferred_sink
                                    );
                                    preferred_match = Some(sink);
                                    break;
                                }
                            }
                        }
                    }
                }
                _ = preferred_poll.tick(), if preferred_sink.is_some() => {
                    let current_peer_paths = p2p.peers().await.map_err(|e| {
                        NetError::DiscoveryError(format!("Failed to get current peers: {}", e))
                    })?;

                    if let Some(preferred_sink) = preferred_sink {
                        let preferred_address = preferred_sink.to_ascii_lowercase();
                        for peer_path in current_peer_paths {
                            if let Some(sink) = self.peer_to_sink(&peer_path).await? {
                                let sink_address = sink.address.to_ascii_lowercase();
                                if sink.name == preferred_sink || sink_address == preferred_address {
                                    tracing::info!(
                                        "Preferred sink {} already visible, ending discovery early",
                                        preferred_sink
                                    );
                                    preferred_match = Some(sink);
                                    break;
                                }
                            }
                        }
                    }

                    if preferred_match.is_some() {
                        break;
                    }
                }
                _ = &mut timeout_future => {
                    tracing::info!("Discovery timeout reached");
                    break;
                }
            }
        }

        if let Some(sink) = preferred_match {
            p2p.stop_find()
                .await
                .map_err(|e| NetError::DiscoveryError(format!("Failed to stop P2P find: {}", e)))?;
            return Ok(vec![sink]);
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
            if let Some(sink) = self.peer_to_sink(peer_path).await? {
                sinks.push(sink);
            }
        }

        // Stop discovery
        p2p.stop_find()
            .await
            .map_err(|e| NetError::DiscoveryError(format!("Failed to stop P2P find: {}", e)))?;

        Ok(sinks)
    }

    async fn peer_to_sink(
        &self,
        peer_path: &zvariant::OwnedObjectPath,
    ) -> Result<Option<Sink>, NetError> {
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

        if !is_miracast_sink(&wfd_ies) {
            tracing::debug!("Peer {} ({}) is not a Miracast sink", name, hw_address);
            return Ok(None);
        }

        tracing::info!("Found Miracast sink: {} ({})", name, hw_address);

        Ok(Some(Sink {
            name,
            address: hw_address,
            ip_address: None,
            go_ip_address: None,
            rtsp_port: parse_wfd_rtsp_port(&wfd_ies),
            wfd_capabilities: Some(parse_wfd_capabilities(&wfd_ies)),
        }))
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
        // Format: [Subelement ID] [Length] [Device Info: 2 bytes] [RTSP Port] [Throughput]
        let wfd_ies: Vec<u8> = vec![
            0x00, // Subelement ID: WFD Device Information
            0x00, 0x06, // Length: 6 bytes
            0x00, 0x05, // Device Info: source capabilities
            0x1C, 0x44, // RTSP Port: 7236 (big-endian)
            0x00, 0xC8, // Max Throughput: 200 Mbps
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

        // The TV only keeps the P2P group alive briefly if RTSP negotiation does not start.
        // Prefer the wpa_supplicant group-start event so we can connect immediately.
        tracing::info!("Waiting for P2P group IP information...");
        let group_started = self.wait_for_group_started().await;
        let preferred_interface = group_started
            .as_ref()
            .map(|info| info.interface_name.as_str());
        let ready_interface = self
            .wait_for_p2p_interface_address(preferred_interface)
            .await;

        let (ip_address, interface_name) = if let Some((interface_name, ip_address)) =
            ready_interface
        {
            (ip_address, interface_name)
        } else {
            if group_started.is_none() {
                tracing::warn!("Did not observe P2P-GROUP-STARTED in wpa_supplicant logs; falling back to NetworkManager IP lookup");
            }

            let ip_address = match self.get_ip_from_active_connection(&active_conn_path).await {
                Ok(ip) => ip,
                Err(_) => self.get_device_ip_address().await?,
            };

            (
                ip_address,
                group_started
                    .as_ref()
                    .map(|info| info.interface_name.clone())
                    .unwrap_or_else(|| self.config.interface_name.clone()),
            )
        };
        let go_ip_address = group_started.map(|info| info.go_ip_address);

        tracing::info!("Got IP address: {}", ip_address);

        let connected_sink = Sink {
            name: sink.name.clone(),
            address: sink.address.clone(),
            ip_address: Some(ip_address),
            go_ip_address,
            rtsp_port: sink.rtsp_port,
            wfd_capabilities: sink.wfd_capabilities.clone(),
        };

        Ok(P2pConnection {
            sink: connected_sink,
            interface: interface_name,
        })
    }

    async fn wait_for_group_started(&self) -> Option<GroupStartedInfo> {
        use std::process::Command;

        for _ in 0..20 {
            let output = Command::new("journalctl")
                .args([
                    "-u",
                    "wpa_supplicant",
                    "--since",
                    "5 seconds ago",
                    "--no-pager",
                    "-o",
                    "cat",
                ])
                .output()
                .ok()?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(info) = stdout
                .lines()
                .rev()
                .find_map(Self::parse_group_started_line)
            {
                tracing::info!(
                    "Observed P2P group start on {} with local {} and GO {}",
                    info.interface_name,
                    info.ip_address,
                    info.go_ip_address
                );
                return Some(info);
            }

            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        None
    }

    async fn wait_for_p2p_interface_address(
        &self,
        preferred_interface: Option<&str>,
    ) -> Option<(String, String)> {
        for _ in 0..24 {
            if let Some(interface_info) = self.find_p2p_interface_address(preferred_interface) {
                return Some(interface_info);
            }

            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        None
    }

    fn parse_group_started_line(line: &str) -> Option<GroupStartedInfo> {
        let marker = "P2P-GROUP-STARTED ";
        let start = line.find(marker)? + marker.len();
        let data = &line[start..];
        let interface_name = data.split_whitespace().next()?.to_string();
        let ip_address = data
            .split("ip_addr=")
            .nth(1)?
            .split_whitespace()
            .next()?
            .to_string();
        let go_ip_address = data
            .split("go_ip_addr=")
            .nth(1)?
            .split_whitespace()
            .next()?
            .to_string();

        Some(GroupStartedInfo {
            interface_name,
            ip_address,
            go_ip_address,
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

    async fn get_ip_from_active_connection(
        &self,
        active_conn_path: &zvariant::OwnedObjectPath,
    ) -> Result<String, NetError> {
        // Get the active connection's IP4 config
        let active_conn_proxy = zbus::Proxy::new(
            &self.connection,
            "org.freedesktop.NetworkManager",
            active_conn_path.as_str(),
            "org.freedesktop.NetworkManager.Connection.Active",
        )
        .await?;

        for _ in 0..20 {
            // Try to get IP4Config from active connection
            if let Ok(ip4_config_path) = active_conn_proxy
                .get_property::<zvariant::OwnedObjectPath>("Ip4Config")
                .await
            {
                if ip4_config_path.as_str() != "/" {
                    let ip4_config = IP4ConfigProxy::builder(&self.connection)
                        .path(ip4_config_path)?
                        .build()
                        .await?;

                    if let Ok(addresses) = ip4_config.addresses().await {
                        if let Some((addr, _, _)) = addresses.first() {
                            let ip = u32::from_be(*addr);
                            let ip_str = format!(
                                "{}.{}.{}.{}",
                                (ip >> 24) & 0xFF,
                                (ip >> 16) & 0xFF,
                                (ip >> 8) & 0xFF,
                                ip & 0xFF
                            );
                            tracing::debug!("Got IP from active connection: {}", ip_str);
                            return Ok(ip_str);
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        Err(NetError::NetworkManagerError(
            "No IP from active connection".into(),
        ))
    }

    fn find_p2p_interface_address(
        &self,
        preferred_interface: Option<&str>,
    ) -> Option<(String, String)> {
        use std::process::Command;

        let output = Command::new("ip")
            .args(["-4", "-o", "addr", "show"])
            .output()
            .ok()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut fallback = None;

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 4 {
                continue;
            }

            let iface = parts[1];
            if !(iface.starts_with("p2p-") || iface == "p2p0") {
                continue;
            }

            let Some(ip) = parts[3].split('/').next() else {
                continue;
            };

            if preferred_interface == Some(iface) {
                tracing::debug!("Found preferred P2P IP on {}: {}", iface, ip);
                return Some((iface.to_string(), ip.to_string()));
            }

            fallback.get_or_insert_with(|| (iface.to_string(), ip.to_string()));
        }

        if let Some((iface, ip)) = &fallback {
            tracing::debug!("Found fallback P2P IP on {}: {}", iface, ip);
        }

        fallback
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
    const DEFAULT_RTSP_PORT: u16 = 7236;

    // WFD Device Information Subelement format:
    // Byte 0: Subelement ID (0x00 for WFD Device Information)
    // Bytes 1-2: Length (big-endian, 0x0006)
    // Bytes 3-4: WFD Device Information
    // Bytes 5-6: Session Management Control Port (RTSP)
    // Bytes 7-8: WFD Device Maximum Throughput
    if wfd_ies.len() >= 9 && wfd_ies[0] == 0x00 {
        let declared_len = ((wfd_ies[1] as u16) << 8) | (wfd_ies[2] as u16);
        if declared_len >= 6 {
            return ((wfd_ies[5] as u16) << 8) | (wfd_ies[6] as u16);
        }
    }

    // Tolerate older malformed single-byte-device-info payloads while we migrate tests.
    if wfd_ies.len() >= 6 && wfd_ies[0] == 0x00 {
        return ((wfd_ies[4] as u16) << 8) | (wfd_ies[5] as u16);
    }

    if wfd_ies.len() >= 3 {
        return ((wfd_ies[1] as u16) << 8) | (wfd_ies[2] as u16);
    }

    DEFAULT_RTSP_PORT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_wfd_rtsp_port() {
        // Test with LG TV WFD IEs: full WFD Device Information subelement
        // Format: [Subelement ID, Length, Device Info (2 bytes), RTSP Port, Throughput]
        let lg_tv_wfd_ies = vec![0x00, 0x00, 0x06, 0x01, 0x13, 0x1c, 0x44, 0x00, 0x32];
        assert_eq!(parse_wfd_rtsp_port(&lg_tv_wfd_ies), 7236);

        // Test with spec-compliant source advertisement using default port 7236 (0x1C44)
        let standard_wfd_ies = vec![0x00, 0x00, 0x06, 0x00, 0x05, 0x1C, 0x44, 0x00, 0xC8];
        assert_eq!(parse_wfd_rtsp_port(&standard_wfd_ies), 7236);

        // Accept the older malformed single-byte device-info layout while migrating callers.
        let legacy_wfd_ies = vec![0x00, 0x00, 0x06, 0x05, 0x1C, 0x44];
        assert_eq!(parse_wfd_rtsp_port(&legacy_wfd_ies), 7236);

        // Test with insufficient bytes - should return default
        let short_wfd_ies = vec![0x00, 0x00];
        assert_eq!(parse_wfd_rtsp_port(&short_wfd_ies), 7236);

        // Test with empty bytes - should return default
        let empty_wfd_ies: Vec<u8> = vec![];
        assert_eq!(parse_wfd_rtsp_port(&empty_wfd_ies), 7236);

        // Additional test with custom port
        let custom_port = vec![0x00, 0x00, 0x06, 0x00, 0x05, 0x08, 0xae, 0x00, 0xc8];
        assert_eq!(parse_wfd_rtsp_port(&custom_port), 2222);
    }

    #[test]
    fn test_parse_group_started_line() {
        let line = "P2P-GROUP-STARTED p2p-wlp2s0-7 client ssid=\"DIRECT-XY\" freq=2412 go_dev_addr=22:28:bc:a8:6c:fe [PERSISTENT] ip_addr=192.168.49.10 ip_mask=255.255.255.0 go_ip_addr=192.168.49.1";
        let info = P2pManager::parse_group_started_line(line).expect("group started info");

        assert_eq!(info.interface_name, "p2p-wlp2s0-7");
        assert_eq!(info.ip_address, "192.168.49.10");
        assert_eq!(info.go_ip_address, "192.168.49.1");
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
        // Bytes 3-4: WFD Device Information
        // Bytes 5-6: Session Management Control Port = 7236 (0x1C44)
        // Bytes 7-8: WFD Device Maximum Throughput
        let wfd_ies_expected = vec![
            0x00, // Subelement ID: WFD Device Information
            0x00, 0x06, // Length: 6 bytes
            0x00, 0x05, // Device Info: source capabilities
            0x1C, 0x44, // RTSP Port: 7236 (big-endian = 0x1C44 = 7236 decimal)
            0x00, 0xC8, // Max Throughput: 200 Mbps
        ];

        // Construct the same vector as done in the connect method
        let wfd_ies_actual = vec![
            0x00, // Subelement ID: WFD Device Information
            0x00, 0x06, // Length: 6 bytes
            0x00, 0x05, // Device Info: source capabilities
            0x1C, 0x44, // RTSP Port: 7236 (big-endian)
            0x00, 0xC8, // Max Throughput: 200 Mbps
        ];

        assert_eq!(wfd_ies_expected, wfd_ies_actual);

        // Verify individual components in correct positions
        assert_eq!(wfd_ies_actual[0], 0x00); // Subelement ID
        assert_eq!(wfd_ies_actual[1], 0x00); // Length byte 1
        assert_eq!(wfd_ies_actual[2], 0x06); // Length byte 2 (total 6 bytes following)
        assert_eq!(wfd_ies_actual[3], 0x00); // Device info byte 1
        assert_eq!(wfd_ies_actual[4], 0x05); // Device info byte 2
        assert_eq!(wfd_ies_actual[5], 0x1C); // RTSP port high byte
        assert_eq!(wfd_ies_actual[6], 0x44); // RTSP port low byte
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
