use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use span_core::{DeviceId, DeviceInfo, Platform, TrustState};

use crate::config::{parse_platform, platform_name, sanitize};
#[derive(Clone, Debug)]
pub struct TrustStore {
    path: PathBuf,
    devices: Vec<DeviceInfo>,
}

impl TrustStore {
    pub fn load(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        let mut devices = if path.exists() {
            parse_devices(&fs::read_to_string(&path)?)
        } else {
            Vec::new()
        };
        let raw_devices = devices.clone();
        normalize_devices(&mut devices);
        let store = Self { path, devices };
        if store.devices != raw_devices {
            store.save()?;
        }

        Ok(store)
    }

    pub fn devices(&self) -> &[DeviceInfo] {
        &self.devices
    }

    pub fn trusted_devices(&self) -> Vec<&DeviceInfo> {
        self.devices
            .iter()
            .filter(|device| device.trust_state == TrustState::Trusted)
            .collect()
    }

    pub fn trusted_device(&self, id: &DeviceId) -> Option<&DeviceInfo> {
        self.devices
            .iter()
            .find(|device| &device.id == id && device.trust_state == TrustState::Trusted)
    }

    pub fn trusted_device_mut(&mut self, id: &DeviceId) -> Option<&mut DeviceInfo> {
        self.devices
            .iter_mut()
            .find(|device| &device.id == id && device.trust_state == TrustState::Trusted)
    }

    pub fn save_now(&self) -> io::Result<()> {
        self.save()
    }

    pub fn device(&self, id: &DeviceId) -> Option<&DeviceInfo> {
        self.devices.iter().find(|device| &device.id == id)
    }

    pub fn trust_existing(&mut self, id: &DeviceId) -> io::Result<bool> {
        let Some(device) = self.devices.iter_mut().find(|device| &device.id == id) else {
            return Ok(false);
        };
        device.trust_state = TrustState::Trusted;
        normalize_devices(&mut self.devices);
        self.save()?;
        Ok(true)
    }

    pub fn trust(
        &mut self,
        id: DeviceId,
        name: String,
        platform: Platform,
        endpoint: Option<String>,
        public_key: Option<String>,
    ) -> io::Result<()> {
        if let Some(device) = self.devices.iter_mut().find(|device| device.id == id) {
            device.name = name;
            device.platform = platform;
            device.trust_state = TrustState::Trusted;
            if endpoint.is_some() {
                device.endpoint = endpoint;
            }
            if public_key.is_some() {
                device.public_key = public_key;
            }
        } else {
            self.devices.push(DeviceInfo {
                id,
                name,
                platform,
                trust_state: TrustState::Trusted,
                endpoint,
                public_key,
            });
        }
        normalize_devices(&mut self.devices);
        self.save()
    }

    pub fn record_discovered(&mut self, mut discovered: DeviceInfo) -> io::Result<bool> {
        let Some(index) = find_existing_device_index(&self.devices, &discovered) else {
            self.devices.push(discovered);
            normalize_devices(&mut self.devices);
            self.save()?;
            return Ok(true);
        };
        let existing = &mut self.devices[index];

        if matches!(
            existing.trust_state,
            TrustState::Blocked | TrustState::Revoked
        ) {
            return Ok(false);
        }

        if let (Some(existing_key), Some(discovered_key)) = (
            existing.public_key.as_deref(),
            discovered.public_key.as_deref(),
        ) {
            if existing_key != discovered_key {
                self.devices.push(discovered);
                normalize_devices(&mut self.devices);
                self.save()?;
                return Ok(false);
            }
        }

        discovered.trust_state = existing.trust_state;
        let changed = *existing != discovered;
        if changed {
            *existing = discovered;
            normalize_devices(&mut self.devices);
            self.save()?;
        }
        Ok(changed)
    }

    pub fn revoke(&mut self, id: &DeviceId) -> io::Result<bool> {
        let mut changed = false;
        for device in &mut self.devices {
            if &device.id == id {
                device.trust_state = TrustState::Revoked;
                changed = true;
            }
        }
        if changed {
            self.save()?;
        }
        Ok(changed)
    }

    pub fn update_endpoint_and_key(
        &mut self,
        id: &DeviceId,
        endpoint: String,
        public_key: String,
    ) -> io::Result<bool> {
        let Some(device) = self.devices.iter_mut().find(|device| {
            device.trust_state == TrustState::Trusted
                && (&device.id == id || device.public_key.as_deref() == Some(public_key.as_str()))
        }) else {
            return Ok(false);
        };

        let mut changed = false;

        if device.endpoint.as_deref() != Some(endpoint.as_str()) {
            device.endpoint = Some(endpoint);
            changed = true;
        }

        match device.public_key.as_deref() {
            None => {
                device.public_key = Some(public_key);
                changed = true;
            }
            Some(existing) if existing == public_key => {}
            Some(_) => {
                return Ok(false);
            }
        }

        if changed {
            self.save()?;
        }
        Ok(changed)
    }

    pub fn reset(&mut self) -> io::Result<()> {
        self.devices.clear();
        self.save()
    }

    pub fn compact(&mut self) -> io::Result<bool> {
        let before = self.devices.clone();
        normalize_devices(&mut self.devices);
        let changed = self.devices != before;
        if changed {
            self.save()?;
        }
        Ok(changed)
    }

    fn save(&self) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::File::create(&self.path)?;
        file.write_all(serialize_devices(&self.devices).as_bytes())?;
        file.sync_all()
    }
}

fn parse_devices(value: &str) -> Vec<DeviceInfo> {
    value.lines().filter_map(parse_device).collect()
}

fn find_existing_device_index(devices: &[DeviceInfo], discovered: &DeviceInfo) -> Option<usize> {
    devices
        .iter()
        .position(|device| device.id == discovered.id)
        .or_else(|| {
            discovered.public_key.as_deref().and_then(|key| {
                devices.iter().position(|device| {
                    device
                        .public_key
                        .as_deref()
                        .is_some_and(|existing| existing == key)
                })
            })
        })
        .or_else(|| {
            devices
                .iter()
                .position(|device| same_advertised_device(device, discovered))
        })
}

fn normalize_devices(devices: &mut Vec<DeviceInfo>) {
    let mut normalized: Vec<DeviceInfo> = Vec::new();
    for device in devices.drain(..) {
        if let Some(index) = find_merge_target(&normalized, &device) {
            merge_device(&mut normalized[index], device);
        } else {
            normalized.push(device);
        }
    }
    *devices = normalized;
}

fn find_merge_target(devices: &[DeviceInfo], incoming: &DeviceInfo) -> Option<usize> {
    devices
        .iter()
        .position(|device| device.id == incoming.id)
        .or_else(|| {
            incoming.public_key.as_deref().and_then(|key| {
                devices.iter().position(|device| {
                    device
                        .public_key
                        .as_deref()
                        .is_some_and(|existing| existing == key)
                })
            })
        })
        .or_else(|| {
            devices
                .iter()
                .position(|device| same_advertised_device(device, incoming))
        })
}

fn same_advertised_device(left: &DeviceInfo, right: &DeviceInfo) -> bool {
    if left
        .public_key
        .as_deref()
        .zip(right.public_key.as_deref())
        .is_some_and(|(left, right)| left != right)
    {
        return false;
    }

    left.name == right.name && left.platform == right.platform
}

fn merge_device(existing: &mut DeviceInfo, incoming: DeviceInfo) {
    let key_conflict = existing
        .public_key
        .as_deref()
        .zip(incoming.public_key.as_deref())
        .is_some_and(|(left, right)| left != right);

    if key_conflict && existing.trust_state == TrustState::Trusted {
        return;
    }

    existing.trust_state = stronger_trust_state(existing.trust_state, incoming.trust_state);
    // Same public key means this is the same cryptographic device. Keep the
    // newest advertised id so future encrypted packets pass sender lookup.
    existing.id = incoming.id;
    existing.name = incoming.name;
    existing.platform = incoming.platform;
    if incoming.endpoint.is_some() {
        existing.endpoint = incoming.endpoint;
    }
    if existing.public_key.is_none() || existing.trust_state != TrustState::Trusted {
        if incoming.public_key.is_some() {
            existing.public_key = incoming.public_key;
        }
    }
}

fn stronger_trust_state(left: TrustState, right: TrustState) -> TrustState {
    if trust_state_rank(right) > trust_state_rank(left) {
        right
    } else {
        left
    }
}

fn trust_state_rank(state: TrustState) -> u8 {
    match state {
        TrustState::Discovered => 0,
        TrustState::Pending => 1,
        TrustState::Trusted => 2,
        TrustState::Revoked => 3,
        TrustState::Blocked => 4,
    }
}

fn parse_device(line: &str) -> Option<DeviceInfo> {
    let mut fields = line.split('\t');
    Some(DeviceInfo {
        id: DeviceId::new(fields.next()?.to_string())?,
        name: fields.next()?.to_string(),
        platform: parse_platform(fields.next()?),
        trust_state: parse_trust_state(fields.next()?),
        endpoint: fields.next().and_then(non_empty_string),
        public_key: fields.next().and_then(non_empty_string),
    })
}

fn serialize_devices(devices: &[DeviceInfo]) -> String {
    devices
        .iter()
        .map(|device| {
            format!(
                "{}\t{}\t{}\t{}\t{}\t{}\n",
                device.id,
                sanitize(&device.name),
                platform_name(device.platform),
                trust_state_name(device.trust_state),
                device.endpoint.as_deref().unwrap_or_default(),
                device.public_key.as_deref().unwrap_or_default()
            )
        })
        .collect()
}

fn non_empty_string(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then_some(value.to_string())
}

fn parse_trust_state(value: &str) -> TrustState {
    match value {
        "discovered" => TrustState::Discovered,
        "pending" => TrustState::Pending,
        "trusted" => TrustState::Trusted,
        "blocked" => TrustState::Blocked,
        "revoked" => TrustState::Revoked,
        _ => TrustState::Discovered,
    }
}

fn trust_state_name(state: TrustState) -> &'static str {
    match state {
        TrustState::Discovered => "discovered",
        TrustState::Pending => "pending",
        TrustState::Trusted => "trusted",
        TrustState::Blocked => "blocked",
        TrustState::Revoked => "revoked",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_trusted_devices() {
        let path = temp_file("trusted-devices.tsv");
        let _ = fs::remove_file(&path);

        let mut store = TrustStore::load(&path).unwrap();
        store
            .trust(
                DeviceId::new("abc").unwrap(),
                "MacBook".to_string(),
                Platform::MacOs,
                Some("127.0.0.1".to_string()),
                Some("aa".repeat(32)),
            )
            .unwrap();

        let loaded = TrustStore::load(&path).unwrap();
        assert_eq!(loaded.trusted_devices().len(), 1);
        assert_eq!(loaded.trusted_devices()[0].name, "MacBook");
        assert_eq!(
            loaded.trusted_devices()[0].endpoint.as_deref(),
            Some("127.0.0.1")
        );
        assert_eq!(
            loaded.trusted_devices()[0].public_key.as_deref(),
            Some(&"aa".repeat(32)[..])
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn revoked_devices_stop_receiving() {
        let path = temp_file("revoked-devices.tsv");
        let _ = fs::remove_file(&path);
        let id = DeviceId::new("abc").unwrap();

        let mut store = TrustStore::load(&path).unwrap();
        store
            .trust(
                id.clone(),
                "MacBook".to_string(),
                Platform::MacOs,
                Some("127.0.0.1".to_string()),
                Some("aa".repeat(32)),
            )
            .unwrap();
        assert!(store.revoke(&id).unwrap());

        let loaded = TrustStore::load(&path).unwrap();
        assert!(loaded.trusted_devices().is_empty());

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn updates_only_trusted_device_endpoint() {
        let path = temp_file("endpoint-devices.tsv");
        let _ = fs::remove_file(&path);
        let trusted_id = DeviceId::new("trusted").unwrap();
        let unknown_id = DeviceId::new("unknown").unwrap();

        let mut store = TrustStore::load(&path).unwrap();
        store
            .trust(
                trusted_id.clone(),
                "MacBook".to_string(),
                Platform::MacOs,
                None,
                None,
            )
            .unwrap();

        assert!(
            store
                .update_endpoint_and_key(&trusted_id, "192.168.1.23".to_string(), "aa".repeat(32))
                .unwrap()
        );
        assert!(
            store
                .update_endpoint_and_key(&unknown_id, "192.168.1.24".to_string(), "aa".repeat(32))
                .unwrap()
        );
        assert!(
            !store
                .update_endpoint_and_key(&unknown_id, "192.168.1.25".to_string(), "bb".repeat(32))
                .unwrap()
        );

        let loaded = TrustStore::load(&path).unwrap();
        assert_eq!(
            loaded.trusted_devices()[0].endpoint.as_deref(),
            Some("192.168.1.24")
        );
        assert_eq!(
            loaded.trusted_devices()[0].public_key.as_deref(),
            Some(&"aa".repeat(32)[..])
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn trust_lookup_requires_trusted_state() {
        let path = temp_file("trusted-lookup.tsv");
        let _ = fs::remove_file(&path);
        let trusted_id = DeviceId::new("trusted").unwrap();
        let revoked_id = DeviceId::new("revoked").unwrap();

        let mut store = TrustStore::load(&path).unwrap();
        store
            .trust(
                trusted_id.clone(),
                "MacBook".to_string(),
                Platform::MacOs,
                None,
                None,
            )
            .unwrap();
        store
            .trust(
                revoked_id.clone(),
                "Old PC".to_string(),
                Platform::Windows,
                None,
                None,
            )
            .unwrap();
        store.revoke(&revoked_id).unwrap();

        assert!(store.trusted_device(&trusted_id).is_some());
        assert!(store.trusted_device(&revoked_id).is_none());
        assert!(
            store
                .trusted_device(&DeviceId::new("unknown").unwrap())
                .is_none()
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn discovery_keeps_existing_trust_state() {
        let path = temp_file("discovery-trust.tsv");
        let _ = fs::remove_file(&path);
        let id = DeviceId::new("trusted").unwrap();

        let mut store = TrustStore::load(&path).unwrap();
        store
            .trust(
                id.clone(),
                "Phone".to_string(),
                Platform::Android,
                Some("192.168.1.10".to_string()),
                Some("aa".repeat(32)),
            )
            .unwrap();

        store
            .record_discovered(DeviceInfo {
                id: id.clone(),
                name: "Phone".to_string(),
                platform: Platform::Android,
                trust_state: TrustState::Discovered,
                endpoint: Some("192.168.1.11".to_string()),
                public_key: Some("aa".repeat(32)),
            })
            .unwrap();

        assert!(store.trusted_device(&id).is_some());
        assert_eq!(
            store.trusted_device(&id).unwrap().endpoint.as_deref(),
            Some("192.168.1.11")
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn discovery_keeps_same_name_new_key_as_new_device() {
        let path = temp_file("same-name-new-key.tsv");
        let _ = fs::remove_file(&path);

        let mut store = TrustStore::load(&path).unwrap();
        store
            .trust(
                DeviceId::new("old-phone").unwrap(),
                "Phone".to_string(),
                Platform::Android,
                Some("192.168.1.10".to_string()),
                Some("aa".repeat(32)),
            )
            .unwrap();

        store
            .record_discovered(DeviceInfo {
                id: DeviceId::new("new-phone").unwrap(),
                name: "Phone".to_string(),
                platform: Platform::Android,
                trust_state: TrustState::Discovered,
                endpoint: Some("192.168.1.11".to_string()),
                public_key: Some("bb".repeat(32)),
            })
            .unwrap();

        assert_eq!(store.devices().len(), 2);
        assert_eq!(store.trusted_devices().len(), 1);
        assert!(
            store
                .devices()
                .iter()
                .any(|device| device.id.as_str() == "new-phone"
                    && device.trust_state == TrustState::Discovered)
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_merges_duplicate_history_by_public_key() {
        let path = temp_file("duplicate-history.tsv");
        let _ = fs::remove_file(&path);
        fs::write(
            &path,
            concat!(
                "old-phone\tPhone\tandroid\tdiscovered\t192.168.1.10\t",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
                "new-phone\tPhone\tandroid\ttrusted\t192.168.1.11\t",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n"
            ),
        )
        .unwrap();

        let store = TrustStore::load(&path).unwrap();
        assert_eq!(store.devices().len(), 1);
        assert_eq!(store.trusted_devices().len(), 1);
        assert_eq!(store.trusted_devices()[0].id.as_str(), "new-phone");
        assert_eq!(
            store.trusted_devices()[0].endpoint.as_deref(),
            Some("192.168.1.11")
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn compact_persists_deduplicated_history() {
        let path = temp_file("compact-history.tsv");
        let _ = fs::remove_file(&path);
        fs::write(
            &path,
            concat!(
                "old-phone\tPhone\tandroid\tdiscovered\t192.168.1.10\t",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\n",
                "new-phone\tPhone\tandroid\ttrusted\t192.168.1.11\t",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\n"
            ),
        )
        .unwrap();

        let mut store = TrustStore::load(&path).unwrap();
        assert!(!store.compact().unwrap());

        let raw = fs::read_to_string(&path).unwrap();
        assert_eq!(raw.lines().count(), 1);
        assert!(raw.contains("new-phone"));

        let _ = fs::remove_file(&path);
    }

    fn temp_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("span-{}-{}", std::process::id(), name))
    }
}
