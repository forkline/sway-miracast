#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_cli_parsing() {
        // Test that CLI can parse basic commands
        let cmd = Cli::try_parse_from(&["swaybeam", "doctor"]);
        assert!(cmd.is_ok());
        match cmd.unwrap().command {
            Command::Doctor => (), // Expected
            _ => panic!("Expected Doctor command"),
        }

        let cmd = Cli::try_parse_from(&["swaybeam", "discover"]);
        assert!(cmd.is_ok());
        match cmd.unwrap().command {
            Command::Discover { timeout } => assert_eq!(timeout, 10), // Default value
            _ => panic!("Expected Discover command with default timeout"),
        }

        let cmd = Cli::try_parse_from(&["swaybeam", "discover", "-t", "20"]);
        assert!(cmd.is_ok());
        match cmd.unwrap().command {
            Command::Discover { timeout } => assert_eq!(timeout, 20),
            _ => panic!("Expected Discover command with custom timeout"),
        }

        let cmd = Cli::try_parse_from(&["swaybeam", "connect", "-s", "TestSink"]);
        assert!(cmd.is_ok());
        match cmd.unwrap().command {
            Command::Connect { sink } => assert_eq!(sink, "TestSink"),
            _ => panic!("Expected Connect command"),
        }

        let cmd = Cli::try_parse_from(&["swaybeam", "stream"]);
        assert!(cmd.is_ok());
        match cmd.unwrap().command {
            Command::Stream {
                width,
                height,
                framerate,
            } => {
                assert_eq!(width, 1920);
                assert_eq!(height, 1080);
                assert_eq!(framerate, 30); // Default values
            }
            _ => panic!("Expected Stream command with default parameters"),
        }

        let cmd = Cli::try_parse_from(&[
            "swaybeam",
            "stream",
            "--width",
            "1280",
            "--height",
            "720",
            "--framerate",
            "60",
        ]);
        assert!(cmd.is_ok());
        match cmd.unwrap().command {
            Command::Stream {
                width,
                height,
                framerate,
            } => {
                assert_eq!(width, 1280);
                assert_eq!(height, 720);
                assert_eq!(framerate, 60);
            }
            _ => panic!("Expected Stream command with custom parameters"),
        }

        let cmd = Cli::try_parse_from(&["swaybeam", "disconnect"]);
        assert!(cmd.is_ok());
        match cmd.unwrap().command {
            Command::Disconnect => (), // Expected
            _ => panic!("Expected Disconnect command"),
        }

        let cmd = Cli::try_parse_from(&["swaybeam", "daemon"]);
        assert!(cmd.is_ok());
        match cmd.unwrap().command {
            Command::Daemon => (), // Expected
            _ => panic!("Expected Daemon command"),
        }

        let cmd = Cli::try_parse_from(&["swaybeam", "status"]);
        assert!(cmd.is_ok());
        match cmd.unwrap().command {
            Command::Status => (), // Expected
            _ => panic!("Expected Status command"),
        }
    }

    #[test]
    fn test_json_flag() {
        let cmd = Cli::try_parse_from(&["swaybeam", "--json", "doctor"]);
        assert!(cmd.is_ok());
        let parsed = cmd.unwrap();
        assert!(parsed.json);
        match parsed.command {
            Command::Doctor => (), // Expected
            _ => panic!("Expected Doctor command"),
        }
    }

    #[test]
    fn test_invalid_command() {
        let cmd = Cli::try_parse_from(&["swaybeam", "invalid"]);
        assert!(cmd.is_err());
    }

    #[test]
    fn test_help_command() {
        let result = std::panic::catch_unwind(|| Cli::try_parse_from(&["swaybeam", "--help"]));
        // Should not panic because help triggers exit early
        assert!(result.is_err()); // Clap exits early on help
    }
}
