//! Common test utilities for real system integration tests

use std::sync::Once;

static GST_INIT: Once = Once::new();
static PW_INIT: Once = Once::new();

pub fn ensure_gstreamer_init() {
    GST_INIT.call_once(|| {
        gstreamer::init().expect("Failed to initialize GStreamer");
    });
}

pub fn ensure_pipewire_init() {
    PW_INIT.call_once(|| {
        pipewire::init();
    });
}

/// Check if a service is available on D-Bus
pub async fn is_dbus_service_available(conn: &zbus::Connection, name: &str) -> bool {
    let proxy = zbus::ProxyBuilder::new(conn)
        .destination("org.freedesktop.DBus")
        .path("/org/freedesktop/DBus")
        .interface("org.freedesktop.DBus")
        .build()
        .await
        .ok();

    if let Some(proxy) = proxy {
        let names: Vec<String> = proxy.call("ListNames", &()).await.unwrap_or_default();
        names.iter().any(|n| n == name)
    } else {
        false
    }
}

/// Check if PipeWire is running
pub async fn is_pipewire_available() -> bool {
    let conn = zbus::Connection::session().await.ok();
    if let Some(conn) = conn {
        is_dbus_service_available(&conn, "org.pipewire.MediaSession").await
    } else {
        false
    }
}

/// Check if NetworkManager is running
pub async fn is_networkmanager_available() -> bool {
    let conn = zbus::Connection::system().await.ok();
    if let Some(conn) = conn {
        is_dbus_service_available(&conn, "org.freedesktop.NetworkManager").await
    } else {
        false
    }
}
