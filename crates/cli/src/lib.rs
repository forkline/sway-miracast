use std::fmt;
use std::io;

/// All possible commands for the CLI
#[derive(Debug, PartialEq)]
pub enum Command {
    Doctor,
    Discover {
        timeout: Option<u64>,
    },
    Connect {
        sink_name: String,
    },
    Disconnect {
        session_id: Option<String>,
    },
    Status,
    List,
    Stream {
        sink: Option<String>,
        config: Option<String>,
    },
    Stop {
        session: Option<String>,
    },
    Help,
    Version,
}

#[derive(Debug)]
pub enum CliError {
    InvalidArgs(String),
    NetworkError(String),
    ParseError(String),
    IoError(String),
    GeneralError(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::InvalidArgs(msg) => write!(f, "Invalid arguments: {}", msg),
            CliError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            CliError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            CliError::IoError(msg) => write!(f, "IO error: {}", msg),
            CliError::GeneralError(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for CliError {}

impl From<io::Error> for CliError {
    fn from(err: io::Error) -> Self {
        CliError::IoError(err.to_string())
    }
}

impl From<swaybeam_net::NetError> for CliError {
    fn from(err: swaybeam_net::NetError) -> Self {
        CliError::NetworkError(err.to_string())
    }
}

pub fn parse_args(args: &[String]) -> Result<Command, CliError> {
    if args.len() < 2 {
        return Ok(Command::Help);
    }

    match args[1].as_str() {
        "doctor" => Ok(Command::Doctor),
        "discover" => {
            let mut timeout = None;
            let mut i = 2;

            while i < args.len() {
                match args[i].as_str() {
                    "--timeout" | "-t" => {
                        if i + 1 >= args.len() {
                            return Err(CliError::InvalidArgs(
                                "Missing timeout value after --timeout".to_string(),
                            ));
                        }
                        timeout = match args[i + 1].parse::<u64>() {
                            Ok(val) => Some(val),
                            Err(_) => {
                                return Err(CliError::ParseError(
                                    "Invalid timeout value, must be a number".to_string(),
                                ));
                            }
                        };
                        i += 2;
                    }
                    _ => {
                        return Err(CliError::InvalidArgs(format!(
                            "Unknown option: {}",
                            args[i]
                        )));
                    }
                }
            }

            Ok(Command::Discover { timeout })
        }
        "connect" => {
            if args.len() < 3 {
                return Err(CliError::InvalidArgs(
                    "Missing sink name for connect command".to_string(),
                ));
            }
            Ok(Command::Connect {
                sink_name: args[2].clone(),
            })
        }
        "disconnect" => {
            let session_id = if args.len() > 2 {
                Some(args[2].clone())
            } else {
                None
            };
            Ok(Command::Disconnect { session_id })
        }
        "status" => Ok(Command::Status),
        "list" => Ok(Command::List),
        "stream" => {
            let mut sink = None;
            let mut config = None;
            let mut i = 2;

            while i < args.len() {
                match args[i].as_str() {
                    "--sink" | "-s" => {
                        if i + 1 >= args.len() {
                            return Err(CliError::InvalidArgs(
                                "Missing sink name after --sink".to_string(),
                            ));
                        }
                        sink = Some(args[i + 1].clone());
                        i += 2;
                    }
                    "--config" | "-c" => {
                        if i + 1 >= args.len() {
                            return Err(CliError::InvalidArgs(
                                "Missing config file after --config".to_string(),
                            ));
                        }
                        config = Some(args[i + 1].clone());
                        i += 2;
                    }
                    _ => {
                        return Err(CliError::InvalidArgs(format!(
                            "Unknown option for stream command: {}",
                            args[i]
                        )));
                    }
                }
            }

            Ok(Command::Stream { sink, config })
        }
        "stop" => {
            let session = if args.len() > 2 {
                Some(args[2].clone())
            } else {
                None
            };
            Ok(Command::Stop { session })
        }
        "--help" | "-h" => Ok(Command::Help),
        "--version" | "-v" => Ok(Command::Version),
        _ => Err(CliError::InvalidArgs(format!(
            "Unknown command: {}",
            args[1]
        ))),
    }
}

pub fn print_discovery_results(sinks: &[swaybeam_net::Sink]) {
    if sinks.is_empty() {
        println!("No Miracast sinks discovered.");
        return;
    }

    println!("Discovered {} Miracast sink(s):", sinks.len());
    for sink in sinks {
        println!("  Name: {}", sink.name);
        println!("  Address: {}", sink.address);
        if let Some(ip) = &sink.ip_address {
            println!("  IP Address: {}", ip);
        } else {
            println!("  IP Address: <not connected>");
        }
        println!();
    }
}

// Simplified session representation for use in the CLI
#[derive(Debug)]
pub struct SwaybeamSession {
    pub sink: Option<swaybeam_net::Sink>,
    pub status: SessionStatus,
    pub connection_time: String,
}

#[derive(Debug)]
pub enum SessionStatus {
    Connected,
    Connecting,
    Disconnected,
    Streaming,
    Error,
}

impl fmt::Display for SwaybeamSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Miracast Session Status: {:?}", self.status)?;

        if let Some(sink) = &self.sink {
            writeln!(f, "  Sink: {}", sink.name)?;
            writeln!(f, "  Address: {}", sink.address)?;
            if let Some(ip) = &sink.ip_address {
                writeln!(f, "  IP Address: {}", ip)?;
            }
        }

        writeln!(f, "  Connection Time: {}", self.connection_time)?;

        Ok(())
    }
}

impl fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionStatus::Connected => write!(f, "Connected"),
            SessionStatus::Connecting => write!(f, "Connecting"),
            SessionStatus::Disconnected => write!(f, "Disconnected"),
            SessionStatus::Streaming => write!(f, "Streaming"),
            SessionStatus::Error => write!(f, "Error"),
        }
    }
}

pub fn print_status(session: &SwaybeamSession) {
    println!("{}", session);
}

pub fn print_error(err: &CliError) {
    eprintln!("❌ Error: {}", err);
}
