use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use span_core::{DeviceId, Platform};

use crate::crypto::{decode_hex, encode_hex, generate_keypair};

#[derive(Clone, Debug)]
pub struct LocalDevice {
    pub id: DeviceId,
    pub name: String,
    pub platform: Platform,
    pub private_key: [u8; 32],
    pub public_key: [u8; 32],
}

impl LocalDevice {
    pub fn public_key_hex(&self) -> String {
        encode_hex(&self.public_key)
    }
}

pub fn config_dir() -> io::Result<PathBuf> {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    }
    .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "config directory not found"))?;

    let dir = base.join("span");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn trust_store_path() -> io::Result<PathBuf> {
    Ok(config_dir()?.join("trusted-devices.tsv"))
}

/// Return the CLI/daemon executable next to the currently running binary.
///
/// The GUI is shipped as a separate binary so it can be launched without a
/// console. Its daemon and auto-start actions must still invoke `span`, not
/// `span-gui`.
pub fn cli_executable_path() -> io::Result<PathBuf> {
    let current = std::env::current_exe()?;
    let is_gui = current
        .file_stem()
        .and_then(|name| name.to_str())
        .map(|name| name == "span-gui")
        .unwrap_or(false);

    if is_gui {
        let cli_name = if cfg!(windows) { "span.exe" } else { "span" };
        Ok(current.with_file_name(cli_name))
    } else {
        Ok(current)
    }
}

/// Return the companion GUI executable for this installation.
///
/// Release bundles keep GUI as a separate executable so Windows can use a
/// no-console subsystem. macOS may ship the GUI inside `Span.app`, so fall back
/// to that app layout when no sibling `span-gui` exists.
pub fn gui_executable_path() -> io::Result<PathBuf> {
    let current = std::env::current_exe()?;
    let gui_name = if cfg!(windows) {
        "span-gui.exe"
    } else {
        "span-gui"
    };
    let sibling = current.with_file_name(gui_name);
    if sibling.exists() {
        return Ok(sibling);
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(dir) = current.parent() {
            let app_gui = dir.join("Span.app/Contents/MacOS/span-gui");
            if app_gui.exists() {
                return Ok(app_gui);
            }
        }
    }

    Ok(sibling)
}

pub fn daemon_pid_path() -> io::Result<PathBuf> {
    Ok(config_dir()?.join("daemon.pid"))
}

pub fn daemon_log_path() -> io::Result<PathBuf> {
    Ok(config_dir()?.join("daemon.log"))
}

pub fn load_or_create_local_device() -> io::Result<LocalDevice> {
    let path = config_dir()?.join("device.tsv");
    if let Ok(value) = fs::read_to_string(&path) {
        if let Some(device) = parse_local_device(&value) {
            return Ok(device);
        }
    }

    let (private_key, public_key) = generate_keypair();
    let device = LocalDevice {
        id: DeviceId::new(format!("{}-{}", device_name(), timestamp())).unwrap(),
        name: device_name(),
        platform: current_platform(),
        private_key,
        public_key,
    };
    fs::write(&path, serialize_local_device(&device))?;
    Ok(device)
}

fn parse_local_device(value: &str) -> Option<LocalDevice> {
    let mut fields = value.trim().split('\t');
    Some(LocalDevice {
        id: DeviceId::new(fields.next()?.to_string())?,
        name: fields.next()?.to_string(),
        platform: parse_platform(fields.next()?),
        private_key: decode_key(fields.next()?)?,
        public_key: decode_key(fields.next()?)?,
    })
}

fn serialize_local_device(device: &LocalDevice) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\n",
        device.id,
        sanitize(&device.name),
        platform_name(device.platform),
        encode_hex(&device.private_key),
        encode_hex(&device.public_key)
    )
}

fn decode_key(value: &str) -> Option<[u8; 32]> {
    let bytes = decode_hex(value)?;
    if bytes.len() != 32 {
        return None;
    }
    let mut key = [0_u8; 32];
    key.copy_from_slice(&bytes);
    Some(key)
}

pub fn current_platform() -> Platform {
    if cfg!(target_os = "windows") {
        Platform::Windows
    } else if cfg!(target_os = "macos") {
        Platform::MacOs
    } else if cfg!(target_os = "linux") {
        Platform::Linux
    } else {
        Platform::Unknown
    }
}

pub fn platform_name(platform: Platform) -> &'static str {
    match platform {
        Platform::Windows => "windows",
        Platform::MacOs => "macos",
        Platform::Linux => "linux",
        Platform::Android => "android",
        Platform::Ios => "ios",
        Platform::IpadOs => "ipados",
        Platform::Unknown => "unknown",
    }
}

pub fn parse_platform(value: &str) -> Platform {
    match value {
        "windows" => Platform::Windows,
        "macos" => Platform::MacOs,
        "linux" => Platform::Linux,
        "android" => Platform::Android,
        "ios" => Platform::Ios,
        "ipados" => Platform::IpadOs,
        _ => Platform::Unknown,
    }
}

pub fn sanitize(value: &str) -> String {
    value.replace(['\t', '\n', '\r'], " ")
}

fn device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "span-device".to_string())
}

fn timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}
