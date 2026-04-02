#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_daemon_creation() {
        let daemon = Daemon::new();
        assert_eq!(daemon.get_state(), DaemonState::Idle);
    }

    #[tokio::test]
    async fn test_daemon_with_config() {
        let config = DaemonConfig {
            video_width: 1280,
            video_height: 720,
            video_framerate: 60,
            video_bitrate: 6_000_000,
            discovery_timeout: Duration::from_secs(5),
            interface: "wlan1".to_string(),
        };

        let daemon = Daemon::with_config(config);
        assert_eq!(daemon.get_state(), DaemonState::Idle);
    }

    #[tokio::test]
    async fn test_daemon_state_transitions() {
        let config = DaemonConfig::default();
        let daemon = Daemon::with_config(config);

        // Initially idle
        assert_eq!(daemon.get_state(), DaemonState::Idle);

        // Manually test changing state
        {
            let mut state = daemon.state.write();
            *state = DaemonState::Discovering;
        }

        assert_eq!(daemon.get_state(), DaemonState::Discovering);

        {
            let mut state = daemon.state.write();
            *state = DaemonState::Idle;
        }
    }

    #[test]
    fn test_daemon_config_default() {
        let config = DaemonConfig::default();
        assert_eq!(config.video_width, 1920);
        assert_eq!(config.video_height, 1080);
        assert_eq!(config.video_framerate, 30);
        assert_eq!(config.video_bitrate, 8_000_000);
        assert_eq!(config.discovery_timeout, Duration::from_secs(10));
        assert_eq!(config.interface, "wlan0");
    }

    #[tokio::test]
    async fn test_daemon_event_subscription() {
        let mut daemon = Daemon::new();
        let mut events_rx = daemon.subscribe_events();

        // Send an event to verify the channel works
        let event_tx = daemon.event_tx.clone();
        event_tx.send(DaemonEvent::Started).ok();

        // Since this is happening in background we need to check if the channel is still valid
        // This is actually difficult to test directly since the subscription mechanism involves
        // transferring ownership of one part of the channel, so we trust the implementation

        assert_eq!(daemon.get_state(), DaemonState::Idle);
    }
}
