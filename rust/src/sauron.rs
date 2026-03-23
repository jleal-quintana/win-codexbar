//! Runtime integration for the sauron-sees desktop agent.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::settings::Settings;

const SAURON_EXE: &str = "sauron-sees.exe";
const SAURON_REPO_URL: &str = "https://github.com/jleal-quintana/sauron-sees";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SauronAgentState {
    Watching,
    Paused,
    Stopped,
    NotInstalled,
}

impl SauronAgentState {
    pub fn status_label(self) -> &'static str {
        match self {
            Self::Watching => "Watching 👁️",
            Self::Paused => "Paused ⏸",
            Self::Stopped => "Stopped 💤",
            Self::NotInstalled => "Not Installed",
        }
    }

    pub fn progress_percent(self) -> f64 {
        match self {
            Self::Watching => 100.0,
            Self::Paused => 50.0,
            Self::Stopped | Self::NotInstalled => 0.0,
        }
    }

    pub fn from_status_text(text: &str) -> Self {
        let normalized = text.to_ascii_lowercase();
        if normalized.contains("watching") || normalized.contains("running") {
            Self::Watching
        } else if normalized.contains("pause") {
            Self::Paused
        } else if normalized.contains("not installed") || normalized.contains("not found") {
            Self::NotInstalled
        } else {
            Self::Stopped
        }
    }
}

#[derive(Debug, Clone)]
pub struct SauronStatusReport {
    pub state: SauronAgentState,
    pub detail: String,
    pub screenshots_dir: Option<PathBuf>,
}

pub struct SauronManager {
    child: Option<Child>,
}

impl Default for SauronManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SauronManager {
    pub fn new() -> Self {
        Self { child: None }
    }

    pub fn repo_url() -> &'static str {
        SAURON_REPO_URL
    }

    pub fn status(settings: &Settings) -> Result<SauronStatusReport> {
        let exe_path = resolve_exe_path(settings).context("sauron-sees.exe was not found")?;
        let screenshots_dir = resolve_screenshots_dir(settings);
        let output = run_command(&exe_path, settings, ["status"])?;
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let combined = if stdout.is_empty() {
            stderr
        } else if stderr.is_empty() {
            stdout
        } else {
            format!("{} {}", stdout, stderr)
        };
        let detail = if combined.is_empty() {
            "Stopped".to_string()
        } else {
            combined
        };

        Ok(SauronStatusReport {
            state: SauronAgentState::from_status_text(&detail),
            detail,
            screenshots_dir,
        })
    }

    pub fn start(&mut self, settings: &Settings) -> Result<()> {
        self.reap_if_exited();
        if self.child.is_some() {
            return Ok(());
        }

        let exe_path = resolve_exe_path(settings).context("sauron-sees.exe was not found")?;
        let config_path = resolve_config_path(settings)
            .ok_or_else(|| anyhow::anyhow!("Could not determine the sauron-sees config path"))?;

        let mut command = Command::new(exe_path);
        command
            .arg("--config")
            .arg(config_path)
            .arg("agent")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child = command.spawn().context("Failed to start sauron-sees agent")?;
        self.child = Some(child);
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        let Some(mut child) = self.child.take() else {
            return Ok(());
        };

        terminate_child(&mut child).context("Failed to stop sauron-sees agent")?;
        Ok(())
    }

    pub fn pause_for_one_hour(&self, settings: &Settings) -> Result<()> {
        let exe_path = resolve_exe_path(settings).context("sauron-sees.exe was not found")?;
        let _ = run_command(&exe_path, settings, ["pause", "--duration", "1h"])?;
        Ok(())
    }

    pub fn resume(&self, settings: &Settings) -> Result<()> {
        let exe_path = resolve_exe_path(settings).context("sauron-sees.exe was not found")?;
        let _ = run_command(&exe_path, settings, ["resume"])?;
        Ok(())
    }

    pub fn open_screenshots_folder(&self, settings: &Settings) -> Result<()> {
        let screenshots_dir = resolve_screenshots_dir(settings)
            .ok_or_else(|| anyhow::anyhow!("Could not determine the Sauron screenshots folder"))?;
        open::that(&screenshots_dir).context("Failed to open screenshots folder")?;
        Ok(())
    }

    pub fn is_child_running(&mut self) -> bool {
        self.reap_if_exited();
        self.child.is_some()
    }

    fn reap_if_exited(&mut self) {
        let exited = self
            .child
            .as_mut()
            .and_then(|child| child.try_wait().ok())
            .flatten()
            .is_some();
        if exited {
            self.child = None;
        }
    }
}

impl Drop for SauronManager {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn resolve_exe_path(settings: &Settings) -> Option<PathBuf> {
    if let Some(path) = settings
        .sauron_exe_path
        .as_deref()
        .map(expand_env_path)
        .filter(|path| path.is_file())
    {
        return Some(path);
    }

    which::which(SAURON_EXE)
        .ok()
        .or_else(|| which::which("sauron-sees").ok())
        .or_else(common_install_path)
}

fn resolve_config_path(settings: &Settings) -> Option<PathBuf> {
    if let Some(path) = settings
        .sauron_config_path
        .as_deref()
        .map(expand_env_path)
        .filter(|path| !path.as_os_str().is_empty())
    {
        return Some(path);
    }

    default_config_candidates()
        .into_iter()
        .find(|path| path.is_file())
}

fn default_config_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(dir) = dirs::config_dir() {
        candidates.push(dir.join("sauron-sees").join("config.toml"));
    }

    if let Some(dir) = dirs::data_local_dir() {
        candidates.push(dir.join("sauron-sees").join("config.toml"));
    }

    candidates
}

fn common_install_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from).map(|dir| {
        dir.join("Programs")
            .join("sauron-sees")
            .join(SAURON_EXE)
    })
    .filter(|path| path.is_file())
}

fn run_command<const N: usize>(
    exe_path: &Path,
    settings: &Settings,
    args: [&str; N],
) -> Result<std::process::Output> {
    let mut command = Command::new(exe_path);
    maybe_add_config_arg(&mut command, settings);
    command.args(args);
    let output = command.output().context("Failed to run sauron-sees command")?;
    if output.status.success() {
        Ok(output)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        Err(anyhow::anyhow!(
            "sauron-sees command failed{}",
            if detail.is_empty() {
                String::new()
            } else {
                format!(": {}", detail)
            }
        ))
    }
}

fn maybe_add_config_arg(command: &mut Command, settings: &Settings) {
    if let Some(config_path) = resolve_config_path(settings) {
        command.arg("--config").arg(config_path);
    }
}

fn resolve_screenshots_dir(settings: &Settings) -> Option<PathBuf> {
    let config_path = resolve_config_path(settings)?;
    let config_contents = std::fs::read_to_string(&config_path).ok()?;
    let config = toml::from_str::<SauronConfig>(&config_contents).ok()?;
    let temp_root = config.temp_root.map(|value| expand_env_path(&value))?;

    [
        temp_root.join("screenshots"),
        temp_root.join("captures"),
        temp_root.clone(),
    ]
    .into_iter()
    .find(|path| path.exists())
    .or(Some(temp_root))
}

fn expand_env_path(raw: &str) -> PathBuf {
    let mut expanded = raw.to_string();
    for (key, value) in std::env::vars() {
        let token = format!("%{}%", key);
        if expanded.contains(&token) {
            expanded = expanded.replace(&token, &value);
        }
    }
    PathBuf::from(expanded)
}

fn terminate_child(child: &mut Child) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::Threading::TerminateProcess;

        let handle = HANDLE(child.as_raw_handle() as isize);
        unsafe {
            let _ = TerminateProcess(handle, 0);
        }
    }

    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    child.kill()?;
    let _ = child.wait();
    Ok(())
}

#[derive(Debug, Deserialize)]
struct SauronConfig {
    temp_root: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::SauronAgentState;

    #[test]
    fn parse_status_text() {
        assert_eq!(
            SauronAgentState::from_status_text("Watching and recording"),
            SauronAgentState::Watching
        );
        assert_eq!(
            SauronAgentState::from_status_text("Paused for 1h"),
            SauronAgentState::Paused
        );
        assert_eq!(
            SauronAgentState::from_status_text("not installed"),
            SauronAgentState::NotInstalled
        );
        assert_eq!(
            SauronAgentState::from_status_text("idle"),
            SauronAgentState::Stopped
        );
    }
}
