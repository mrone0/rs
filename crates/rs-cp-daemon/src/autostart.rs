use std::fs;
use std::io;
use std::path::PathBuf;

pub fn install() -> io::Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        return install_macos_launch_agent();
    }

    #[cfg(target_os = "windows")]
    {
        return install_windows_startup_script();
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        install_linux_desktop_entry()
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", unix)))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "autostart unsupported on this platform",
        ))
    }
}

pub fn uninstall() -> io::Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        return remove_file_if_exists(macos_launch_agent_path()?);
    }

    #[cfg(target_os = "windows")]
    {
        return remove_file_if_exists(windows_startup_script_path()?);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        remove_file_if_exists(linux_desktop_entry_path()?)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", unix)))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "autostart unsupported on this platform",
        ))
    }
}

#[cfg(target_os = "macos")]
fn install_macos_launch_agent() -> io::Result<PathBuf> {
    let path = macos_launch_agent_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let exe = std::env::current_exe()?;
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.rs-cp.daemon</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
    <string>run</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>StandardOutPath</key>
  <string>/tmp/rs-cp.out.log</string>
  <key>StandardErrorPath</key>
  <string>/tmp/rs-cp.err.log</string>
</dict>
</plist>
"#,
        escape_xml(&exe.display().to_string())
    );

    fs::write(&path, plist)?;
    Ok(path)
}

#[cfg(target_os = "macos")]
fn macos_launch_agent_path() -> io::Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME not set"))?;
    Ok(home.join("Library/LaunchAgents/com.rs-cp.daemon.plist"))
}

#[cfg(target_os = "windows")]
fn install_windows_startup_script() -> io::Result<PathBuf> {
    let path = windows_startup_script_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let exe = std::env::current_exe()?;
    fs::write(
        &path,
        format!("@echo off\r\nstart \"\" /min \"{}\" run\r\n", exe.display()),
    )?;
    Ok(path)
}

#[cfg(target_os = "windows")]
fn windows_startup_script_path() -> io::Result<PathBuf> {
    let appdata = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "APPDATA not set"))?;
    Ok(appdata.join("Microsoft/Windows/Start Menu/Programs/Startup/rs-cp.cmd"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn install_linux_desktop_entry() -> io::Result<PathBuf> {
    let path = linux_desktop_entry_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let exe = std::env::current_exe()?;
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=rs-cp\nExec={} run\nX-GNOME-Autostart-enabled=true\n",
        exe.display()
    );
    fs::write(&path, entry)?;
    Ok(path)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn linux_desktop_entry_path() -> io::Result<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "config directory not found"))?;
    Ok(base.join("autostart/rs-cp.desktop"))
}

fn remove_file_if_exists(path: PathBuf) -> io::Result<PathBuf> {
    match fs::remove_file(&path) {
        Ok(()) => Ok(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(path),
        Err(error) => Err(error),
    }
}

#[cfg(target_os = "macos")]
fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
