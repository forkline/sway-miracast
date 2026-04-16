use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub enum Status {
    Ok,
    Warn,
    Error,
}

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub status: Status,
    pub message: String,
}

impl CheckResult {
    pub fn ok(message: &str) -> Self {
        CheckResult {
            status: Status::Ok,
            message: message.to_string(),
        }
    }

    pub fn warn(message: &str) -> Self {
        CheckResult {
            status: Status::Warn,
            message: message.to_string(),
        }
    }

    pub fn error(message: &str) -> Self {
        CheckResult {
            status: Status::Error,
            message: message.to_string(),
        }
    }
}

#[derive(Debug)]
pub struct Report {
    pub sway_result: CheckResult,
    pub pipewire_result: CheckResult,
    pub gstreamer_result: CheckResult,
    pub network_manager_result: CheckResult,
    pub wpa_supplicant_result: CheckResult,
    pub xdg_desktop_portal_result: CheckResult,
}

impl Report {
    pub fn all_ok(&self) -> bool {
        matches!(self.sway_result.status, Status::Ok)
            && matches!(self.pipewire_result.status, Status::Ok)
            && matches!(self.gstreamer_result.status, Status::Ok)
            && matches!(self.network_manager_result.status, Status::Ok)
            && matches!(self.wpa_supplicant_result.status, Status::Ok)
            && matches!(self.xdg_desktop_portal_result.status, Status::Ok)
    }

    pub fn print(&self) {
        println!("Miracast Doctor - Environment Check Report");
        println!("=========================================");

        self.print_check_result("Sway Compositor", &self.sway_result);
        self.print_check_result("PipeWire", &self.pipewire_result);
        self.print_check_result("GStreamer", &self.gstreamer_result);
        self.print_check_result("NetworkManager", &self.network_manager_result);
        self.print_check_result("WPA Supplicant", &self.wpa_supplicant_result);
        self.print_check_result("XDG Desktop Portal", &self.xdg_desktop_portal_result);

        if self.all_ok() {
            println!("\n✓ All checks passed! Your system is ready for Miracast.");
        } else {
            println!("\n✗ Some checks failed. Please review the issues above.");
            println!("\nRequirements for Miracast:");
            println!("  - WiFi adapter with P2P (Wi-Fi Direct) support");
            println!("  - Sway or wlroots-based compositor");
            println!("  - PipeWire for audio/video");
            println!("  - GStreamer with H.264/H.265 plugins");
            println!("  - NetworkManager for P2P connection management");
            println!("  - xdg-desktop-portal for screen capture");
        }
    }

    fn print_check_result(&self, name: &str, result: &CheckResult) {
        let status_str = match result.status {
            Status::Ok => "✓",
            Status::Warn => "⚠",
            Status::Error => "✗",
        };
        println!("{} {}: {}", status_str, name, result.message);
    }
}

pub fn check_all() -> anyhow::Result<Report> {
    Ok(Report {
        sway_result: check_sway()?,
        pipewire_result: check_pipewire()?,
        gstreamer_result: check_gstreamer()?,
        network_manager_result: check_network_manager()?,
        wpa_supplicant_result: check_wpa_supplicant()?,
        xdg_desktop_portal_result: check_xdg_desktop_portal()?,
    })
}

pub fn check_sway() -> anyhow::Result<CheckResult> {
    // Check for Wayland session
    let wayland_display = std::env::var("WAYLAND_DISPLAY").ok();
    let xdg_session_type = std::env::var("XDG_SESSION_TYPE").ok();

    if std::env::var("SWAYSOCK").is_ok() {
        // Check if swaymsg command works to confirm actual sway session
        match Command::new("swaymsg")
            .arg("-t")
            .arg("get_version")
            .output()
        {
            Ok(output) if output.status.success() => {
                Ok(CheckResult::ok("Running under Sway compositor"))
            }
            _ => Ok(CheckResult::warn("SWAYSOCK set but swaymsg failed")),
        }
    } else if wayland_display.is_some() || xdg_session_type.as_deref() == Some("wayland") {
        // Running Wayland but not Sway - check for other wlroots compositors
        let compositors = ["sway", "river", "labwc", "hyprland", "wayfire"];
        for compositor in compositors {
            let output = Command::new("pgrep").arg(compositor).output();
            if let Ok(o) = output
                && o.status.success()
                && !o.stdout.is_empty()
            {
                return Ok(CheckResult::ok(&format!(
                    "Running under {} (wlroots-compatible)",
                    compositor
                )));
            }
        }

        // Wayland session but unknown compositor
        Ok(CheckResult::warn(
            "Wayland session detected but compositor unknown - may or may not work with xdg-desktop-portal-wlr",
        ))
    } else {
        // Not Wayland
        Ok(CheckResult::error(
            "Not running a Wayland compositor - Miracast requires Sway or another wlroots-based compositor",
        ))
    }
}

pub fn check_pipewire() -> anyhow::Result<CheckResult> {
    // Check if PipeWire daemon is running
    let output = Command::new("pgrep").arg("pipewire").output()?;

    if !output.status.success() || output.stdout.is_empty() {
        // Also check for pulseaudio socket which might indicate PipeWire's presence
        if Path::new(&format!(
            "/run/user/{}/pulse/native",
            std::env::var("UID").unwrap_or_else(|_| "0".to_string())
        ))
        .exists()
            || Path::new("/tmp/pulse-socket").exists()
            || Command::new("pulseaudio")
                .arg("--check")
                .output()
                .is_ok_and(|o| o.status.success())
        {
            // PulseAudio or compatible is running, which might be PipeWire in compatibility mode
            return Ok(CheckResult::warn(
                "PipeWire not running directly, but PulseAudio compatibility might be available",
            ));
        }
        return Ok(CheckResult::error("PipeWire daemon not running"));
    }

    // Check for PipeWire session manager too
    let sm_output = Command::new("pgrep")
        .arg("pipewire-media-session")
        .output()?;
    if !sm_output.status.success() || sm_output.stdout.is_empty() {
        // Try pipewire-pulse instead for older setups
        let pp_output = Command::new("pgrep").arg("pipewire-pulse").output()?;
        if !pp_output.status.success() || pp_output.stdout.is_empty() {
            return Ok(CheckResult::warn(
                "PipeWire core is running but missing media session manager (may cause streaming issues)",
            ));
        }
    }

    Ok(CheckResult::ok(
        "PipeWire daemon and session manager running",
    ))
}

pub fn check_gstreamer() -> anyhow::Result<CheckResult> {
    // Check if GStreamer command-line tools are available
    let output = Command::new("gst-inspect-1.0").arg("--version").output();

    match output {
        Ok(o) if o.status.success() => {
            // Required: H.264 (universally supported by Miracast TVs)
            let required_plugins = ["x264", "h264parse", "rtph264pay"];
            let mut missing_required = Vec::new();

            for plugin in required_plugins.iter() {
                let result = Command::new("gst-inspect-1.0").arg(plugin).output();
                if !matches!(result, Ok(o) if o.status.success()) {
                    missing_required.push(*plugin);
                }
            }

            if !missing_required.is_empty() {
                return Ok(CheckResult::error(&format!(
                    "Missing required H.264 plugins: {} (install gstreamer-plugins-ugly)",
                    missing_required.join(", ")
                )));
            }

            // Optional: H.265 (better for 4K)
            let h265_plugins = ["x265", "h265parse", "rtph265pay"];
            let mut missing_h265 = Vec::new();
            for plugin in h265_plugins.iter() {
                let result = Command::new("gst-inspect-1.0").arg(plugin).output();
                if !matches!(result, Ok(o) if o.status.success()) {
                    missing_h265.push(*plugin);
                }
            }

            // Optional: AV1 (future-proof)
            let av1_available = Command::new("gst-inspect-1.0")
                .arg("svtav1enc")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            // Build result message
            let mut message = String::from("H.264 ready");

            if missing_h265.is_empty() {
                message.push_str(", H.265/4K ready");
            } else {
                message.push_str(&format!(
                    ", H.265 missing: {} (optional, for 4K)",
                    missing_h265.join(", ")
                ));
            }

            if av1_available {
                message.push_str(", AV1 ready");
            }

            Ok(CheckResult::ok(&message))
        }
        _ => Ok(CheckResult::error(
            "GStreamer not installed or gst-inspect-1.0 command not found",
        )),
    }
}

pub fn check_network_manager() -> anyhow::Result<CheckResult> {
    // Check if NetworkManager is running as a system process
    let output = Command::new("pgrep").arg("NetworkManager").output();

    match output {
        Ok(o) if o.status.success() && !o.stdout.is_empty() => {
            Ok(CheckResult::ok("NetworkManager daemon running"))
        }
        _ => {
            // Check via D-Bus as alternative
            let dbus_call = Command::new("busctl")
                .arg("call")
                .arg("org.freedesktop.NetworkManager")
                .arg("/org/freedesktop/NetworkManager")
                .arg("org.freedesktop.DBus.Properties")
                .arg("Get")
                .arg("org.freedesktop.NetworkManager")
                .arg("State")
                .output();

            if let Ok(dbc) = dbus_call
                && dbc.status.success()
            {
                return Ok(CheckResult::ok("NetworkManager accessible via D-Bus"));
            }

            // Try nmcli as final fallback
            let nmcli_call = Command::new("nmcli")
                .arg("-p")
                .arg("g")
                .arg("status")
                .output();
            match nmcli_call {
                Ok(nco) if nco.status.success() => {
                    Ok(CheckResult::ok("NetworkManager accessible via nmcli"))
                }
                _ => Ok(CheckResult::error(
                    "NetworkManager not running or inaccessible",
                )),
            }
        }
    }
}

pub fn check_wpa_supplicant() -> anyhow::Result<CheckResult> {
    // First check if we have any WiFi hardware
    let wifi_hardware = check_wifi_hardware()?;

    if !wifi_hardware {
        return Ok(CheckResult::error(
            "No WiFi hardware detected - Miracast requires WiFi with P2P support",
        ));
    }

    // Check if wpa_supplicant is running
    let output = Command::new("pgrep").arg("wpa_supplicant").output();

    match output {
        Ok(o) if o.status.success() && !o.stdout.is_empty() => {
            Ok(CheckResult::ok("wpa_supplicant daemon running"))
        }
        _ => {
            // Check if NetworkManager handles P2P internally (newer versions do)
            let nm_p2p = Command::new("nmcli")
                .args(["-f", "WIFI-P2P", "general", "status"])
                .output();

            if let Ok(output) = nm_p2p {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.contains("enabled") || stdout.contains("available") {
                    return Ok(CheckResult::ok("NetworkManager P2P support available"));
                }
            }

            // Check wpa_supplicant binary availability as fallback
            let cmd = Command::new("wpa_supplicant").arg("--version").output();
            match cmd {
                Ok(c) if c.status.success() => Ok(CheckResult::warn(
                    "wpa_supplicant installed but not running. Start with: sudo systemctl start wpa_supplicant",
                )),
                _ => Ok(CheckResult::error(
                    "wpa_supplicant not installed. Install: sudo pacman -S wpa_supplicant",
                )),
            }
        }
    }
}

fn check_wifi_hardware() -> anyhow::Result<bool> {
    // Check for wireless interfaces
    let output = Command::new("ip").args(["link", "show"]).output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Look for wireless interfaces (wl*, wlan*)
    let has_wifi = stdout
        .lines()
        .any(|line| line.contains("wl") || line.contains("wlan"));

    if has_wifi {
        return Ok(true);
    }

    // Also check via /sys/class/net for wireless
    if let Ok(entries) = std::fs::read_dir("/sys/class/net") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("wl") || name_str.starts_with("wlan") {
                return Ok(true);
            }

            // Check if device has wireless extensions
            let phy80211 = entry.path().join("phy80211");
            if phy80211.exists() {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

pub fn check_xdg_desktop_portal() -> anyhow::Result<CheckResult> {
    // First try D-Bus which is more reliable
    let dbus_check = Command::new("busctl").arg("--user").arg("list").output();

    if let Ok(output) = dbus_check
        && output.status.success()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("org.freedesktop.portal.Desktop") {
            // Portal is running via D-Bus
            if stdout.contains("xdg-desktop-portal-wlr") {
                return Ok(CheckResult::ok(
                    "xdg-desktop-portal with WLR backend running",
                ));
            } else if stdout.contains("xdg-desktop-portal-gtk") {
                return Ok(CheckResult::warn(
                    "xdg-desktop-portal running with GTK backend (WLR backend preferred for Sway)",
                ));
            } else {
                return Ok(CheckResult::ok("xdg-desktop-portal running"));
            }
        }
    }

    // Fallback to pgrep
    let output = Command::new("pgrep")
        .arg("-f")
        .arg("xdg-desktop-portal")
        .output();

    match output {
        Ok(o) if o.status.success() && !o.stdout.is_empty() => {
            let wlr_output = Command::new("pgrep").arg("xdg-desktop-portal-wlr").output();

            match wlr_output {
                Ok(w) if w.status.success() && !w.stdout.is_empty() => Ok(CheckResult::ok(
                    "xdg-desktop-portal with WLR backend running",
                )),
                _ => Ok(CheckResult::warn(
                    "xdg-desktop-portal running but WLR backend not detected",
                )),
            }
        }
        _ => Ok(CheckResult::error(
            "No xdg-desktop-portal services running (required for screen sharing)",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_enum() {
        assert!(matches!(Status::Ok, Status::Ok));
        assert!(matches!(Status::Warn, Status::Warn));
        assert!(matches!(Status::Error, Status::Error));
    }

    #[test]
    fn test_checkresult_ok() {
        let result = CheckResult::ok("test message");
        assert!(matches!(result.status, Status::Ok));
        assert_eq!(result.message, "test message");
    }

    #[test]
    fn test_checkresult_warn() {
        let result = CheckResult::warn("warning message");
        assert!(matches!(result.status, Status::Warn));
        assert_eq!(result.message, "warning message");
    }

    #[test]
    fn test_checkresult_error() {
        let result = CheckResult::error("error message");
        assert!(matches!(result.status, Status::Error));
        assert_eq!(result.message, "error message");
    }

    #[test]
    fn test_report_all_ok() {
        let report = Report {
            sway_result: CheckResult::ok("ok"),
            pipewire_result: CheckResult::ok("ok"),
            gstreamer_result: CheckResult::ok("ok"),
            network_manager_result: CheckResult::ok("ok"),
            wpa_supplicant_result: CheckResult::ok("ok"),
            xdg_desktop_portal_result: CheckResult::ok("ok"),
        };
        assert!(report.all_ok());
    }

    #[test]
    fn test_report_not_all_ok_with_error() {
        let report = Report {
            sway_result: CheckResult::error("error"),
            pipewire_result: CheckResult::ok("ok"),
            gstreamer_result: CheckResult::ok("ok"),
            network_manager_result: CheckResult::ok("ok"),
            wpa_supplicant_result: CheckResult::ok("ok"),
            xdg_desktop_portal_result: CheckResult::ok("ok"),
        };
        assert!(!report.all_ok());
    }

    #[test]
    fn test_report_not_all_ok_with_warn() {
        let report = Report {
            sway_result: CheckResult::warn("warn"),
            pipewire_result: CheckResult::ok("ok"),
            gstreamer_result: CheckResult::ok("ok"),
            network_manager_result: CheckResult::ok("ok"),
            wpa_supplicant_result: CheckResult::ok("ok"),
            xdg_desktop_portal_result: CheckResult::ok("ok"),
        };
        assert!(!report.all_ok());
    }

    #[test]
    fn test_check_all_returns_report() {
        let result = check_all();
        assert!(result.is_ok());
        let report = result.unwrap();
        assert!(!report.sway_result.message.is_empty());
        assert!(!report.pipewire_result.message.is_empty());
        assert!(!report.gstreamer_result.message.is_empty());
        assert!(!report.network_manager_result.message.is_empty());
        assert!(!report.wpa_supplicant_result.message.is_empty());
        assert!(!report.xdg_desktop_portal_result.message.is_empty());
    }
}
