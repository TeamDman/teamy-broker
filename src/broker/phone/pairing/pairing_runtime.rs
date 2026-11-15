use std::env;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;

/// Runtime artifacts distributed beside the broker, independent of its build checkout.
pub(crate) struct PairingRuntime(PathBuf);

impl PairingRuntime {
    pub(crate) fn installed() -> eyre::Result<Self> {
        let executable =
            env::current_exe().map_err(|_redacted| eyre::eyre!("pairing runtime unavailable"))?;
        Self::beside(&executable)
    }

    fn beside(executable: &Path) -> eyre::Result<Self> {
        eyre::ensure!(executable.is_absolute(), "pairing runtime unavailable");
        let directory = executable
            .parent()
            .ok_or_else(|| eyre::eyre!("pairing runtime unavailable"))?;
        Ok(Self(directory.join("pairing")))
    }

    pub(crate) fn as_path(&self) -> &Path {
        &self.0
    }

    #[cfg(test)]
    pub(crate) fn development_fixture() -> Self {
        Self(Path::new(env!("CARGO_MANIFEST_DIR")).join("pairing/java"))
    }
}

impl fmt::Debug for PairingRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PairingRuntime([redacted])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relocation_uses_the_executable_directory_without_build_paths() {
        let parent = env::temp_dir();
        for directory in ["synthetic-first-install", "synthetic-relocated-install"] {
            let install = parent.join(directory);
            let runtime = PairingRuntime::beside(&install.join("teamy-broker.exe")).unwrap();
            assert_eq!(runtime.as_path(), install.join("pairing"));
            assert!(!format!("{runtime:?}").contains(directory));
        }
        drop(PairingRuntime::beside(Path::new("relative.exe")).unwrap_err());
    }
}
