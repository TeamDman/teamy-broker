use facet::Facet;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::net::Ipv4Addr;
use std::str::FromStr;

/// Profile IPv4 address restricted to 100.64.0.0/10. Membership is checked live.
#[derive(Clone, Copy, PartialEq, Eq, Facet)]
#[facet(opaque, proxy = Ipv4Addr)]
pub struct TailscaleTailnetIpv4Addr(Ipv4Addr);

impl TailscaleTailnetIpv4Addr {
    #[must_use]
    pub fn get(self) -> Ipv4Addr {
        self.0
    }
}

impl TryFrom<Ipv4Addr> for TailscaleTailnetIpv4Addr {
    type Error = eyre::Report;

    fn try_from(value: Ipv4Addr) -> Result<Self, Self::Error> {
        let octets = value.octets();
        eyre::ensure!(
            octets[0] == 100 && (64..=127).contains(&octets[1]),
            "tailnet IPv4 address rejected"
        );
        Ok(Self(value))
    }
}

impl FromStr for TailscaleTailnetIpv4Addr {
    type Err = eyre::Report;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(
            value
                .parse::<Ipv4Addr>()
                .map_err(|_redacted| eyre::eyre!("tailnet IPv4 address rejected"))?,
        )
    }
}

impl From<&TailscaleTailnetIpv4Addr> for Ipv4Addr {
    fn from(value: &TailscaleTailnetIpv4Addr) -> Self {
        value.0
    }
}

impl Debug for TailscaleTailnetIpv4Addr {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("TailscaleTailnetIpv4Addr([redacted])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_accepts_profile_range_boundaries_and_redacts_debug() {
        for address in ["100.64.0.0", "100.127.255.255"] {
            let json = format!("\"{address}\"");
            let parsed: TailscaleTailnetIpv4Addr = facet_json::from_str(&json).unwrap();
            assert_eq!(parsed.get(), address.parse::<Ipv4Addr>().unwrap());
            assert_eq!(facet_json::to_string(&parsed).unwrap(), json);
            assert_eq!(
                format!("{parsed:?}"),
                "TailscaleTailnetIpv4Addr([redacted])"
            );
        }
    }

    #[test]
    fn construction_and_json_reject_addresses_outside_profile_range() {
        for address in [
            "100.63.255.255",
            "100.128.0.0",
            "127.0.0.1",
            "192.0.2.1",
            "not-an-ip",
            "::1",
        ] {
            let error = address.parse::<TailscaleTailnetIpv4Addr>().unwrap_err();
            assert_eq!(error.to_string(), "tailnet IPv4 address rejected");
            let _ = facet_json::from_str::<TailscaleTailnetIpv4Addr>(&format!("\"{address}\""))
                .unwrap_err();
        }
    }
}
