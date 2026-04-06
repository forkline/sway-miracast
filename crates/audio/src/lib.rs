use std::process::Command;
use thiserror::Error;
use tracing::{debug, info, warn};
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("Failed to execute pactl: {0}")]
    CommandFailed(String),
    #[error("Failed to parse pactl output: {0}")]
    ParseError(String),
    #[error("No default sink found")]
    NoDefaultSink,
    #[error("Failed to load module: {0}")]
    ModuleLoadFailed(String),
}

pub type Result<T> = std::result::Result<T, AudioError>;

pub struct VirtualAudioSink {
    sink_name: String,
    module_index: u32,
    previous_default: Option<String>,
    cleaned_up: bool,
}

impl VirtualAudioSink {
    pub fn create() -> Result<Self> {
        let uuid = Uuid::new_v4();
        let sink_name = format!("swaybeam_sink_{:.8}", uuid);

        let previous_default = get_default_sink()?;
        info!("Previous default sink: {:?}", previous_default);

        let module_index = load_null_sink(&sink_name)?;
        info!(
            "Created virtual sink '{}' with module index {}",
            sink_name, module_index
        );

        let sink = VirtualAudioSink {
            sink_name,
            module_index,
            previous_default,
            cleaned_up: false,
        };

        sink.set_as_default()?;

        Ok(sink)
    }

    pub fn sink_name(&self) -> &str {
        &self.sink_name
    }

    pub fn monitor_device(&self) -> String {
        format!("{}.monitor", self.sink_name)
    }

    pub fn set_as_default(&self) -> Result<()> {
        let output = Command::new("pactl")
            .args(["set-default-sink", &self.sink_name])
            .output()
            .map_err(|e| AudioError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AudioError::CommandFailed(stderr.to_string()));
        }

        info!("Set '{}' as default sink", self.sink_name);
        Ok(())
    }

    pub fn cleanup(&mut self) -> Result<()> {
        if self.cleaned_up {
            debug!("Already cleaned up, skipping");
            return Ok(());
        }

        info!("Cleaning up virtual audio sink");

        if let Some(ref previous) = self.previous_default {
            let output = Command::new("pactl")
                .args(["set-default-sink", previous])
                .output()
                .map_err(|e| AudioError::CommandFailed(e.to_string()))?;

            if output.status.success() {
                info!("Restored default sink to '{}'", previous);
            } else {
                warn!("Failed to restore default sink to '{}'", previous);
            }
        }

        let output = Command::new("pactl")
            .args(["unload-module", &self.module_index.to_string()])
            .output()
            .map_err(|e| AudioError::CommandFailed(e.to_string()))?;

        if output.status.success() {
            info!("Unloaded module {}", self.module_index);
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to unload module {}: {}", self.module_index, stderr);
        }

        self.cleaned_up = true;
        Ok(())
    }
}

impl Drop for VirtualAudioSink {
    fn drop(&mut self) {
        if !self.cleaned_up {
            debug!("VirtualAudioSink dropped, performing cleanup");
            if let Err(e) = self.cleanup() {
                warn!("Cleanup failed during drop: {}", e);
            }
        }
    }
}

fn get_default_sink() -> Result<Option<String>> {
    let output = Command::new("pactl")
        .args(["get-default-sink"])
        .output()
        .map_err(|e| AudioError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        return Ok(None);
    }

    let sink = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if sink.is_empty() {
        Ok(None)
    } else {
        Ok(Some(sink))
    }
}

fn load_null_sink(sink_name: &str) -> Result<u32> {
    let description = format!("swaybeam Stream {}", &sink_name[..8]);
    let args = format!(
        "sink_name={} rate=48000 sink_properties=device.description=\"{}\" device.icon_name=\"video-display\"",
        sink_name, description
    );

    let output = Command::new("pactl")
        .args(["load-module", "module-null-sink", &args])
        .output()
        .map_err(|e| AudioError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AudioError::ModuleLoadFailed(stderr.to_string()));
    }

    let index_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let index: u32 = index_str.parse().map_err(|e| {
        AudioError::ParseError(format!(
            "Failed to parse module index '{}': {}",
            index_str, e
        ))
    })?;

    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_device_format() {
        let uuid = Uuid::new_v4();
        let sink_name = format!("swytheam_sink_{:.8}", uuid);
        let monitor = format!("{}.monitor", sink_name);
        assert!(monitor.ends_with(".monitor"));
    }
}
