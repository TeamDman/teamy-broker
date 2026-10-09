use facet::Facet;
/// Exactly one storage choice. Persistent locations are explicitly consumer-selected.
#[derive(Facet, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum Backend {
    Memory,
    WindowsCredentialManager(String),
    WellKnownFile(String),
}
impl std::fmt::Debug for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Memory => "Memory",
            Self::WindowsCredentialManager(_) => "WindowsCredentialManager([redacted])",
            Self::WellKnownFile(_) => "WellKnownFile([redacted])",
        })
    }
}
impl Backend {
    /// # Errors
    ///
    /// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
    pub fn validate(&self) -> eyre::Result<()> {
        match self {
            Self::Memory => Ok(()),
            Self::WindowsCredentialManager(target) => {
                eyre::ensure!(
                    target.starts_with("teamy-broker/")
                        && target.len() <= 200
                        && !target.chars().any(char::is_control),
                    "invalid namespaced credential target"
                );
                Ok(())
            }
            Self::WellKnownFile(path) => {
                let p = std::path::Path::new(path);
                eyre::ensure!(
                    p.is_absolute()
                        && p.file_name().is_some()
                        && path.len() <= 240
                        && !path.chars().any(char::is_control),
                    "invalid explicit file path"
                );
                Ok(())
            }
        }
    }
}
