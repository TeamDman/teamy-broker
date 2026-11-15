use crate::broker::phone::config::PhoneProfile;
use facet::Facet;
use std::fmt;

/// Profile values disclosed only when explicitly requested by the caller.
#[derive(Facet)]
pub struct PhoneConfigValues {
    pub desktop_ip: String,
    pub phone_ip: String,
    pub desktop_node: String,
    pub phone_node: String,
    pub tailnet_user: u64,
    pub phone_name: String,
    pub java_executable: String,
}

impl From<&PhoneProfile> for PhoneConfigValues {
    fn from(profile: &PhoneProfile) -> Self {
        Self {
            desktop_ip: profile.desktop.get().to_string(),
            phone_ip: profile.phone.get().to_string(),
            desktop_node: profile.desktop_node.as_str().to_owned(),
            phone_node: profile.phone_node.as_str().to_owned(),
            tailnet_user: profile.user.get(),
            phone_name: profile.phone_name.as_str().to_owned(),
            java_executable: profile.java.as_path().to_string_lossy().into_owned(),
        }
    }
}

impl fmt::Debug for PhoneConfigValues {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PhoneConfigValues([redacted])")
    }
}
