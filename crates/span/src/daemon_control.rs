use std::fs::{self, OpenOptions};
use std::io;
use std::process::{Command, Stdio};

use crate::config::{cli_executable_path, daemon_log_path, daemon_pid_path};

pub fn start_daemon() -> io::Result<()> {
    #[cfg(target_os = "macos")]
    if launch_agent_loaded() {
        println!("span daemon already running (launch agent)");
        return Ok(());
    }

    let pid_path = daemon_pid_path()?;
    if let Some(pid) = read_pid(&pid_path)? {
        if process_running(pid) {
            println!("span daemon already running (pid {pid})");
            return Ok(());
        }

        let _ = fs::remove_file(&pid_path);
    }

    let exe = cli_executable_path()?;
    let log_path = daemon_log_path()?;
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let stderr = stdout.try_clone()?;

    let mut command = Command::new(exe);
    command
        .arg("run")
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    detach_command(&mut command)?;

    let child = command.spawn()?;
    let pid = child.id();
    fs::write(&pid_path, format!("{pid}\n"))?;

    println!("span daemon started in background (pid {pid})");
    println!("log    : {}", log_path.display());
    Ok(())
}

pub fn stop_daemon() -> io::Result<()> {
    #[cfg(target_os = "macos")]
    if stop_launch_agent()? {
        let pid_path = daemon_pid_path()?;
        let _ = fs::remove_file(pid_path);
        println!("stopped span daemon (launch agent)");
        return Ok(());
    }

    let pid_path = daemon_pid_path()?;
    let Some(pid) = read_pid(&pid_path)? else {
        println!("span daemon is not running");
        return Ok(());
    };

    if !process_running(pid) {
        let _ = fs::remove_file(&pid_path);
        println!("stale pid file removed ({pid})");
        return Ok(());
    }

    terminate_process(pid)?;
    let _ = fs::remove_file(&pid_path);
    println!("stopped span daemon (pid {pid})");
    Ok(())
}

#[cfg(target_os = "macos")]
fn launch_agent_service() -> String {
    format!("gui/{}/com.span.daemon", unsafe { libc::getuid() })
}

#[cfg(target_os = "macos")]
fn launch_agent_loaded() -> bool {
    std::process::Command::new("launchctl")
        .args(["print", &launch_agent_service()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn stop_launch_agent() -> io::Result<bool> {
    if !launch_agent_loaded() {
        return Ok(false);
    }

    let status = std::process::Command::new("launchctl")
        .args(["bootout", &launch_agent_service()])
        .status()?;
    if !status.success() && launch_agent_loaded() {
        return Err(io::Error::other(format!(
            "launchctl bootout failed with {status}"
        )));
    }
    Ok(true)
}

fn read_pid(path: &std::path::Path) -> io::Result<Option<u32>> {
    match fs::read_to_string(path) {
        Ok(value) => Ok(value.trim().parse::<u32>().ok()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn process_running(pid: u32) -> bool {
    unsafe {
        if libc::kill(pid as i32, 0) == 0 {
            true
        } else {
            matches!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(code) if code == libc::EPERM
            )
        }
    }
}

#[cfg(windows)]
fn process_running(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };

    unsafe {
        let handle = OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            pid,
        );
        if handle.is_null() {
            return false;
        }

        let running = WaitForSingleObject(handle, 0) == WAIT_TIMEOUT;
        CloseHandle(handle);
        running
    }
}

#[cfg(unix)]
fn terminate_process(pid: u32) -> io::Result<()> {
    unsafe {
        if libc::kill(pid as i32, libc::SIGTERM) == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

#[cfg(windows)]
fn terminate_process(pid: u32) -> io::Result<()> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};

    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }

        let result = TerminateProcess(handle, 0);
        CloseHandle(handle);
        if result == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

#[cfg(unix)]
fn detach_command(command: &mut Command) -> io::Result<()> {
    use std::os::unix::process::CommandExt;

    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }

    Ok(())
}

#[cfg(windows)]
fn detach_command(command: &mut Command) -> io::Result<()> {
    use std::os::windows::process::CommandExt;

    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;

    command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW | DETACHED_PROCESS);
    Ok(())
}
