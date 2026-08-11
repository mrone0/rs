use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct DeviceId(String);

impl DeviceId {
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        (!value.trim().is_empty()).then_some(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Platform {
    Windows,
    MacOs,
    Linux,
    Android,
    Ios,
    IpadOs,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustState {
    Discovered,
    Pending,
    Trusted,
    Blocked,
    Revoked,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceInfo {
    pub id: DeviceId,
    pub name: String,
    pub platform: Platform,
    pub trust_state: TrustState,
    pub endpoint: Option<String>,
    pub public_key: Option<String>,
}

impl DeviceInfo {
    pub fn can_receive_broadcast(&self) -> bool {
        self.trust_state == TrustState::Trusted
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardText(String);

impl ClipboardText {
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        (!value.is_empty()).then_some(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn preview(&self, max_chars: usize) -> String {
        let mut preview = self.0.chars().take(max_chars).collect::<String>();
        if self.0.chars().count() > max_chars {
            preview.push('…');
        }
        preview
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WireMessage {
    Announce(DeviceInfo),
    TrustRequest { from: DeviceId, fingerprint: String },
    TrustAccepted { from: DeviceId },
    TrustRevoked { from: DeviceId },
    ClipboardBroadcast { from: DeviceId, text: ClipboardText },
}

pub fn broadcast_targets(devices: &[DeviceInfo]) -> Vec<&DeviceInfo> {
    devices
        .iter()
        .filter(|device| device.can_receive_broadcast())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_trusted_devices_receive_broadcasts() {
        let trusted = DeviceInfo {
            id: DeviceId::new("trusted").unwrap(),
            name: "MacBook".to_string(),
            platform: Platform::MacOs,
            trust_state: TrustState::Trusted,
            endpoint: Some("192.168.1.10".to_string()),
            public_key: Some("00".to_string()),
        };
        let discovered = DeviceInfo {
            id: DeviceId::new("discovered").unwrap(),
            name: "Office PC".to_string(),
            platform: Platform::Windows,
            trust_state: TrustState::Discovered,
            endpoint: Some("192.168.1.11".to_string()),
            public_key: None,
        };

        let devices = vec![trusted.clone(), discovered];
        let targets = broadcast_targets(&devices);

        assert_eq!(targets, vec![&trusted]);
    }

    #[test]
    fn text_preview_is_small() {
        let text = ClipboardText::new("1234567890").unwrap();

        assert_eq!(text.preview(4), "1234…");
    }
}
