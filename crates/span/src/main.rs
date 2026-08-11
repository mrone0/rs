mod autostart;
mod clipboard;
mod config;
mod crypto;
mod daemon_control;
mod discovery;
mod transport;
mod trust_store;

use std::io;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use span_core::{DeviceId, TrustState, broadcast_targets};

use crate::clipboard::system_clipboard;
use crate::config::{
    LocalDevice, load_or_create_local_device, parse_platform, platform_name, trust_store_path,
};
use crate::crypto::decode_hex;
use crate::daemon_control::{start_daemon, stop_daemon};
use crate::discovery::{broadcast_once, listen_forever, listen_once};
use crate::transport::{
    TEXT_PORT, decrypt_text, encrypt_text, receive_text_forever, receive_text_once, send_text,
};
use crate::trust_store::TrustStore;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "start".to_string());

    match command.as_str() {
        "start" => start_daemon(),
        "stop" => stop_daemon(),
        "run" => run_daemon(),
        "foreground" => run_daemon(),
        "status" => print_status(),
        "devices" => print_devices(),
        "trust" => trust_device(args.collect()),
        "revoke" => revoke_device(args.next()),
        "reset" => reset_devices(),
        "discover" => discover_devices(),
        "announce" => announce_device(),
        "clip-read" => read_clipboard(),
        "clip-write" => write_clipboard(args.collect()),
        "send-text" => send_text_command(args.collect()),
        "receive-once" => receive_once(),
        "install" => install_autostart(),
        "uninstall" => uninstall_autostart(),
        "ui" => render_dashboard(),
        _ => {
            print_help();
            Ok(())
        }
    }
}

fn run_daemon() -> io::Result<()> {
    let local = load_or_create_local_device()?;
    let store_path = trust_store_path()?;
    let store = TrustStore::load(&store_path)?;
    let suppressed_text = Arc::new(Mutex::new(None::<String>));

    println!("span daemon");
    println!("device : {} ({})", local.name, local.id);
    println!("mode   : background-only, no main window");
    println!("policy : broadcast text to trusted devices only");
    println!("targets: {} trusted", store.trusted_devices().len());
    println!("listen : 0.0.0.0:{TEXT_PORT}");

    spawn_receiver(local.clone(), suppressed_text.clone());
    spawn_discovery_listener(store_path.clone(), local.id.clone());

    let mut clipboard = system_clipboard();
    let mut last_text = clipboard.read_text().unwrap_or(None);
    let mut last_change_count = clipboard.change_count().unwrap_or(None);
    let mut next_announce = std::time::Instant::now();

    loop {
        if std::time::Instant::now() >= next_announce {
            let _ = broadcast_once(&local);
            next_announce = std::time::Instant::now() + Duration::from_secs(15);
        }

        let change_was_reported = match clipboard.wait_for_change(Duration::from_millis(500)) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("clipboard wait error: {error}");
                false
            }
        };

        match clipboard.change_count() {
            Ok(Some(change_count)) if last_change_count == Some(change_count) => {
                if !change_was_reported {
                    continue;
                }
            }
            Ok(Some(change_count)) => {
                last_change_count = Some(change_count);
            }
            Ok(None) if !change_was_reported => continue,
            Ok(None) => {}
            Err(error) => eprintln!("clipboard change count error: {error}"),
        }

        match clipboard.read_text() {
            Ok(Some(text)) if last_text.as_deref() != Some(text.as_str()) => {
                last_text = Some(text.clone());
                if should_suppress(&suppressed_text, &text) {
                    thread::sleep(Duration::from_millis(350));
                    continue;
                }
                broadcast_clipboard_text(&store_path, &local, text)?;
            }
            Ok(_) => {}
            Err(error) => eprintln!("clipboard read error: {error}"),
        }
    }
}

fn spawn_discovery_listener(store_path: std::path::PathBuf, local_id: DeviceId) {
    thread::spawn(move || {
        if let Err(error) = listen_forever(|packet, addr| {
            if packet.id == local_id {
                return Ok(());
            }

            let endpoint = addr.ip().to_string();
            let mut store = TrustStore::load(&store_path)?;
            if store.update_endpoint_and_key(
                &packet.id,
                endpoint.clone(),
                packet.public_key.clone(),
            )? {
                println!("updated endpoint for {}: {endpoint}", packet.name);
            }
            Ok(())
        }) {
            eprintln!("discovery listener stopped: {error}");
        }
    });
}

fn spawn_receiver(local: LocalDevice, suppressed_text: Arc<Mutex<Option<String>>>) {
    thread::spawn(move || {
        let trust_path = match trust_store_path() {
            Ok(path) => path,
            Err(error) => {
                eprintln!("receiver trust path error: {error}");
                return;
            }
        };

        if let Err(error) = receive_text_forever(|packet| {
            let store = TrustStore::load(&trust_path)?;
            let Some(trusted) = store.trusted_device(&packet.from) else {
                eprintln!("rejected text from untrusted device: {}", packet.from);
                return Ok(());
            };

            let Some(sender_key) = trusted.public_key.as_deref() else {
                eprintln!("rejected text from device without key: {}", packet.from);
                return Ok(());
            };

            let text = decrypt_text(&packet, &local.private_key, sender_key)?;

            let mut clipboard = system_clipboard();
            clipboard.write_text(&text)?;
            if let Ok(mut suppressed) = suppressed_text.lock() {
                *suppressed = Some(text);
            }
            println!("received text from {}", packet.from);
            Ok(())
        }) {
            eprintln!("receiver stopped: {error}");
        }
    });
}

fn should_suppress(suppressed_text: &Arc<Mutex<Option<String>>>, text: &str) -> bool {
    let Ok(mut suppressed) = suppressed_text.lock() else {
        return false;
    };

    if suppressed.as_deref() == Some(text) {
        *suppressed = None;
        true
    } else {
        false
    }
}

fn broadcast_clipboard_text(
    store_path: &std::path::Path,
    local: &LocalDevice,
    text: String,
) -> io::Result<()> {
    let store = TrustStore::load(store_path)?;

    for device in store.trusted_devices() {
        let Some(endpoint) = device.endpoint.as_deref() else {
            continue;
        };

        let Some(public_key) = device.public_key.as_deref() else {
            continue;
        };

        let packet = encrypt_text(&local.id, &local.private_key, public_key, &text)?;

        match send_text((endpoint, TEXT_PORT), &packet) {
            Ok(()) => println!("sent text to {}", device.name),
            Err(error) => eprintln!("send to {} failed: {error}", device.name),
        }
    }

    Ok(())
}

fn print_status() -> io::Result<()> {
    let local = load_or_create_local_device()?;
    let store = TrustStore::load(trust_store_path()?)?;

    println!("span: ready");
    println!("device: {} ({})", local.name, local.id);
    println!("platform: {}", platform_name(local.platform));
    println!("public key: {}", local.public_key_hex());
    println!("trusted targets: {}", store.trusted_devices().len());
    Ok(())
}

fn print_devices() -> io::Result<()> {
    let store = TrustStore::load(trust_store_path()?)?;
    let devices = store.devices();
    let trusted = broadcast_targets(devices);

    println!("trusted broadcast targets: {}", trusted.len());
    if devices.is_empty() {
        println!("no devices yet");
        return Ok(());
    }

    for device in devices {
        println!(
            "- {}\t{}\t{}\t{:?}\t{}",
            device.id,
            device.name,
            platform_name(device.platform),
            device.trust_state,
            device.endpoint.as_deref().unwrap_or("-")
        );
    }
    Ok(())
}

fn trust_device(args: Vec<String>) -> io::Result<()> {
    if args.len() == 1 {
        let Some(id) = DeviceId::new(args[0].clone()) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "empty device id",
            ));
        };

        let mut store = TrustStore::load(trust_store_path()?)?;
        if store.trust_existing(&id)? {
            let label = store
                .device(&id)
                .map(|device| device.name.as_str())
                .unwrap_or(id.as_str());
            println!("trusted: {label} ({id})");
        } else {
            println!("not discovered: {id}");
            println!("usage: span trust <id> <name> <platform> [host] [public-key]");
        }
        return Ok(());
    }

    if args.len() < 3 || args.len() > 5 {
        println!("usage: span trust <id> <name> <platform> [host] [public-key]");
        return Ok(());
    }

    let id = &args[0];
    let name = &args[1];
    let platform = &args[2];
    let endpoint = args.get(3).cloned();
    let public_key = args.get(4).map(|value| value.to_lowercase());

    let Some(id) = DeviceId::new(id.clone()) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "empty device id",
        ));
    };

    if let Some(public_key) = public_key.as_deref() {
        let Some(bytes) = decode_hex(public_key) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "bad public key hex",
            ));
        };
        if bytes.len() != 32 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "bad public key length",
            ));
        }
    }

    let platform = parse_platform(platform);
    let mut store = TrustStore::load(trust_store_path()?)?;
    store.trust(id, name.clone(), platform, endpoint.clone(), public_key)?;
    println!(
        "trusted: {name} ({}){}",
        platform_name(platform),
        endpoint
            .as_deref()
            .map(|value| format!(" at {value}"))
            .unwrap_or_default()
    );
    Ok(())
}

fn revoke_device(id: Option<String>) -> io::Result<()> {
    let Some(id) = id.and_then(DeviceId::new) else {
        println!("usage: span revoke <id>");
        return Ok(());
    };

    let mut store = TrustStore::load(trust_store_path()?)?;
    if store.revoke(&id)? {
        println!("revoked: {id}");
    } else {
        println!("not found: {id}");
    }
    Ok(())
}

fn reset_devices() -> io::Result<()> {
    let mut store = TrustStore::load(trust_store_path()?)?;
    store.reset()?;
    println!("trusted devices reset");
    Ok(())
}

fn discover_devices() -> io::Result<()> {
    println!("listening for span devices for 2s...");
    let mut store = TrustStore::load(trust_store_path()?)?;
    for (packet, addr) in listen_once(Duration::from_secs(2))? {
        let mut info = packet.into_device_info();
        info.endpoint = Some(addr.ip().to_string());
        let public_key = info.public_key.clone().unwrap_or_default();
        let _ = store.record_discovered(info.clone())?;
        println!(
            "- {}\t{}\t{}\t{}\t{}",
            info.id,
            info.name,
            platform_name(info.platform),
            addr.ip(),
            public_key
        );
        println!("  trust: span trust {}", info.id);
    }
    Ok(())
}

fn announce_device() -> io::Result<()> {
    let local = load_or_create_local_device()?;
    broadcast_once(&local)?;
    println!("announced: {} ({})", local.name, local.id);
    Ok(())
}

fn print_help() {
    println!("usage: span <command>");
    println!("commands:");
    println!("  start                       launch background daemon and exit");
    println!("  stop                        stop background daemon");
    println!("  run                         start foreground daemon");
    println!("  status                      show local status");
    println!("  devices                     list trusted devices");
    println!("  trust <id>                  trust a discovered device");
    println!("  trust <id> <name> <platform> [host] [public-key]");
    println!("  revoke <id>                 remove a trusted device");
    println!("  reset                       remove all trusted devices");
    println!("  discover                    listen for LAN devices");
    println!("  announce                    broadcast this device once");
    println!("  clip-read                   print current clipboard text");
    println!("  clip-write <text>           write text to clipboard");
    println!("  send-text <host> <text>     send text to a PC");
    println!("  receive-once                receive one text and copy it");
    println!("  install                     install login autostart");
    println!("  uninstall                   remove login autostart");
    println!("  ui                          render minimal text dashboard");
}

fn install_autostart() -> io::Result<()> {
    let path = autostart::install()?;
    println!("installed autostart: {}", path.display());
    Ok(())
}

fn uninstall_autostart() -> io::Result<()> {
    let path = autostart::uninstall()?;
    println!("removed autostart: {}", path.display());
    Ok(())
}

fn read_clipboard() -> io::Result<()> {
    let mut clipboard = system_clipboard();
    match clipboard.read_text()? {
        Some(text) => println!("{text}"),
        None => println!("clipboard is empty or not text"),
    }
    Ok(())
}

fn write_clipboard(args: Vec<String>) -> io::Result<()> {
    if args.is_empty() {
        println!("usage: span clip-write <text>");
        return Ok(());
    }

    let text = args.join(" ");
    let mut clipboard = system_clipboard();
    clipboard.write_text(&text)?;
    println!("clipboard updated");
    Ok(())
}

fn send_text_command(args: Vec<String>) -> io::Result<()> {
    let [host, text @ ..] = args.as_slice() else {
        println!("usage: span send-text <host> <text>");
        return Ok(());
    };

    if text.is_empty() {
        println!("usage: span send-text <host> <text>");
        return Ok(());
    }

    let local = load_or_create_local_device()?;
    let store = TrustStore::load(trust_store_path()?)?;
    let Some(peer) = store.devices().iter().find(|device| {
        device.trust_state == TrustState::Trusted
            && device.endpoint.as_deref() == Some(host.as_str())
    }) else {
        println!("no trusted device found for host {host}");
        return Ok(());
    };
    let Some(public_key) = peer.public_key.as_deref() else {
        println!("trusted device missing public key: {}", peer.name);
        return Ok(());
    };

    let packet = encrypt_text(&local.id, &local.private_key, public_key, &text.join(" "))?;
    send_text((host.as_str(), TEXT_PORT), &packet)?;
    println!("sent text to {host}:{TEXT_PORT}");
    Ok(())
}

fn receive_once() -> io::Result<()> {
    let local = load_or_create_local_device()?;
    let trust_path = trust_store_path()?;

    println!("waiting for text on port {TEXT_PORT}...");
    match receive_text_once(Duration::from_secs(30))? {
        Some(packet) => {
            let store = TrustStore::load(&trust_path)?;
            let Some(trusted) = store.trusted_device(&packet.from) else {
                println!("untrusted sender: {}", packet.from);
                return Ok(());
            };
            let Some(sender_key) = trusted.public_key.as_deref() else {
                println!("missing sender key: {}", packet.from);
                return Ok(());
            };

            let text = decrypt_text(&packet, &local.private_key, sender_key)?;
            let mut clipboard = system_clipboard();
            clipboard.write_text(&text)?;
            println!("received text from {} and copied it", packet.from);
        }
        None => println!("no text received"),
    }
    Ok(())
}

fn render_dashboard() -> io::Result<()> {
    let local = load_or_create_local_device()?;
    let store = TrustStore::load(trust_store_path()?)?;
    let trusted = store.trusted_devices();

    println!("╭────────────────────────────────────────────╮");
    println!("│ span                                      │");
    println!("├────────────────────────────────────────────┤");
    println!("│ mode      : background-only               │");
    println!("│ footprint : minimal                       │");
    println!("│ payload   : text only                     │");
    println!("│ device    : {:<29}│", truncate(&local.name, 29));
    println!("│ targets   : {:<29}│", trusted.len());
    println!("├────────────────────────────────────────────┤");
    if trusted.is_empty() {
        println!("│ no trusted devices                         │");
    } else {
        for device in trusted {
            println!("│ ✓ {:<41}│", truncate(&device.name, 41));
        }
    }
    println!("╰────────────────────────────────────────────╯");
    Ok(())
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut result = value.chars().take(max_chars).collect::<String>();
    if value.chars().count() > max_chars {
        result.pop();
        result.push('…');
    }
    result
}
