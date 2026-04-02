//! Integration tests that require real system services
//! Run with: cargo test --test integration_real -- --ignored

mod common {
    pub mod system_utils;
}

use common::system_utils;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    /// Test that we can connect to PipeWire
    #[tokio::test]
    #[ignore = "Requires PipeWire session"]
    async fn test_pipewire_connection() {
        use pipewire as pw;

        pw::init();

        let mainloop = pw::MainLoop::new(None).expect("Failed to create main loop");
        let context = pw::Context::new(&mainloop).expect("Failed to create context");

        // Connect to default PipeWire instance
        let core = context.connect(None).expect("Failed to connect to PipeWire");

        println!("Connected to PipeWire");

        // Clean up
        drop(core);
        drop(context);
        drop(mainloop);
    }

    /// Test that we can initialize GStreamer
    #[test]
    #[ignore = "Requires GStreamer"]
    fn test_gstreamer_init() {
        use gstreamer as gst;

        gst::init().expect("Failed to initialize GStreamer");
        println!("GStreamer initialized successfully");
    }

    /// Test that we can create a GStreamer pipeline
    #[test]
    #[ignore = "Requires GStreamer"]
    fn test_gstreamer_pipeline() {
        use gstreamer as gst;
        use gst::prelude::*;

        gst::init().expect("Failed to initialize GStreamer");

        let pipeline = gst::parse_launch(
            "videotestsrc ! videoconvert ! x264enc ! fakesink"
        ).expect("Failed to create pipeline");

        pipeline.set_state(gst::State::Playing).expect("Failed to start pipeline");

        std::thread::sleep(Duration::from_millis(100));

        pipeline.set_state(gst::State::Null).expect("Failed to stop pipeline");
    }

    /// Test NetworkManager D-Bus connection
    #[tokio::test]
    #[ignore = "Requires NetworkManager"]
    async fn test_networkmanager_dbus() {
        use zbus::Connection;

        let conn = Connection::system().await.expect("Failed to connect to system bus");

        let proxy = zbus::ProxyBuilder::new(&conn)
            .destination("org.freedesktop.NetworkManager")
            .path("/org/freedesktop/NetworkManager")
            .interface("org.freedesktop.NetworkManager")
            .build()
            .await
            .expect("Failed to create NM proxy");

        let version: String = proxy.call("Get", &("org.freedesktop.NetworkManager", "Version"))
            .await
            .expect("Failed to get version");

        println!("NetworkManager version: {}", version);
        assert!(!version.is_empty());
    }

    /// Test xdg-desktop-portal D-Bus connection
    #[tokio::test]
    #[ignore = "Requires xdg-desktop-portal"]
    async fn test_portal_dbus() {
        use zbus::Connection;

        let conn = Connection::session().await.expect("Failed to connect to session bus");

        let proxy = zbus::ProxyBuilder::new(&conn)
            .destination("org.freedesktop.portal.Desktop")
            .path("/org/freedesktop/portal/desktop")
            .interface("org.freedesktop.portal.ScreenCast")
            .build()
            .await
            .expect("Failed to create portal proxy");

        println!("Connected to xdg-desktop-portal ScreenCast interface");
    }

    /// Test full capture to stream pipeline (requires all services)
    #[tokio::test]
    #[ignore = "Requires all services and user interaction"]
    async fn test_full_pipeline() {
        // This test would:
        // 1. Connect to xdg-desktop-portal and request screen capture
        // 2. Set up PipeWire stream
        // 3. Create GStreamer pipeline
        // 4. Stream for a few seconds
        // 5. Clean up

        // User would need to authorize the screen capture request
        println!("This test requires user interaction to authorize screen capture");
    }
}
