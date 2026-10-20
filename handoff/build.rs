use std::env;
use std::fs;
use std::net::Ipv4Addr;
use std::path::PathBuf;

#[expect(
    clippy::disallowed_macros,
    reason = "Build-script stdout is Cargo's instruction protocol"
)]
fn main() {
    let names = [
        "TEAMY_BROKER_DESKTOP_IP",
        "TEAMY_BROKER_PHONE_IP",
        "TEAMY_BROKER_DESKTOP_NODE",
        "TEAMY_BROKER_PHONE_NODE",
        "TEAMY_BROKER_TAILNET_USER",
        "TEAMY_BROKER_PHONE_NAME",
    ];
    for name in names {
        println!("cargo:rerun-if-env-changed={name}");
    }
    let values: Vec<_> = names.iter().map(|name| env::var(name).ok()).collect();
    let enabled = values.iter().all(Option::is_some);
    assert!(
        enabled || values.iter().all(Option::is_none),
        "incomplete private phone configuration"
    );
    let defaults = [
        "192.0.2.10",
        "192.0.2.11",
        "desktop-placeholder",
        "phone-placeholder",
        "0",
        "phone.invalid.",
    ];
    let values: Vec<_> = values
        .iter()
        .zip(defaults)
        .map(|(value, default)| value.as_deref().unwrap_or(default))
        .collect();
    let desktop: Ipv4Addr = values[0].parse().expect("invalid desktop address");
    let phone: Ipv4Addr = values[1].parse().expect("invalid phone address");
    if enabled {
        for address in [desktop, phone] {
            let octets = address.octets();
            assert!(
                octets[0] == 100 && (64..=127).contains(&octets[1]),
                "phone addresses must be Tailscale IPv4 addresses"
            );
        }
        assert!(desktop != phone, "phone addresses must differ");
        for value in [values[2], values[3], values[5]] {
            assert!(
                !value.is_empty()
                    && value.len() <= 255
                    && value
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b".-_".contains(&c)),
                "invalid phone identity"
            );
        }
    }
    let user: u64 = values[4].parse().expect("invalid tailnet user");
    assert!(!enabled || user != 0, "invalid tailnet user");
    let code = format!(
        "pub const CONFIGURED: bool = {enabled};\npub const DESKTOP: std::net::Ipv4Addr = std::net::Ipv4Addr::from_bits({});\npub const PHONE: std::net::Ipv4Addr = std::net::Ipv4Addr::from_bits({});\npub const PORT: u16 = 43188;\npub const DESKTOP_NODE: &str = {:?};\npub const PHONE_NODE: &str = {:?};\npub const USER: u64 = {user};\npub const PHONE_NAME: &str = {:?};\n",
        u32::from(desktop),
        u32::from(phone),
        values[2],
        values[3],
        values[5]
    );
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("missing build output"));
    fs::write(out.join("phone_config.rs"), code).expect("cannot write phone configuration");
}
