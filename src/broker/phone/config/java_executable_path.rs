use facet::Facet;
#[cfg(test)]
use std::env;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::path::Path;
use std::path::PathBuf;

/// Explicit absolute Java executable path. Availability is checked at use time.
/// Stored as Unicode to preserve the configuration's JSON string encoding.
#[derive(Clone, PartialEq, Eq, Facet)]
#[facet(opaque, proxy = String)]
pub struct JavaExecutablePath(String);

impl JavaExecutablePath {
    #[must_use]
    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }

    #[must_use]
    pub fn into_path_buf(self) -> PathBuf {
        PathBuf::from(self.0)
    }

    /// Check availability at the worker launch boundary.
    ///
    /// # Errors
    /// Rejects a missing executable without exposing its configured path.
    pub fn require_available(&self) -> eyre::Result<()> {
        eyre::ensure!(
            self.as_path().is_file(),
            "explicit Java executable unavailable"
        );
        Ok(())
    }

    /// Host fixtures use the development JDK, never the user's phone profile.
    #[cfg(test)]
    pub(crate) fn for_tests() -> eyre::Result<Self> {
        let path = if let Some(explicit) = env::var_os("TEAMY_BROKER_TEST_JAVA") {
            PathBuf::from(explicit)
        } else if let Some(home) = env::var_os("JAVA_HOME") {
            PathBuf::from(home).join("bin/java.exe")
        } else {
            env::split_paths(&env::var_os("PATH").unwrap_or_default())
                .map(|directory| directory.join("java.exe"))
                .find(|candidate| candidate.is_file())
                .ok_or_else(|| {
                    eyre::eyre!(
                        "host pairing tests require a JDK on PATH or TEAMY_BROKER_TEST_JAVA"
                    )
                })?
        };
        let executable = Self::try_from(path)?;
        executable.require_available()?;
        Ok(executable)
    }
}

impl TryFrom<String> for JavaExecutablePath {
    type Error = eyre::Report;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        eyre::ensure!(
            Path::new(&value).is_absolute(),
            "Java executable path rejected"
        );
        Ok(Self(value))
    }
}

impl TryFrom<PathBuf> for JavaExecutablePath {
    type Error = eyre::Report;

    fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
        Self::try_from(
            value
                .into_os_string()
                .into_string()
                .map_err(|_redacted| eyre::eyre!("Java executable path rejected"))?,
        )
    }
}

impl From<&JavaExecutablePath> for String {
    fn from(value: &JavaExecutablePath) -> Self {
        value.0.clone()
    }
}

impl Debug for JavaExecutablePath {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("JavaExecutablePath([redacted])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_preserves_absolute_paths_without_requiring_a_file() {
        let path = env::temp_dir().join("synthetic-java");
        let parsed = JavaExecutablePath::try_from(path.clone()).unwrap();
        let json = facet_json::to_string(&parsed).unwrap();
        assert_eq!(
            facet_json::from_str::<String>(&json).unwrap(),
            path.to_str().unwrap()
        );
        let decoded: JavaExecutablePath = facet_json::from_str(&json).unwrap();
        assert_eq!(decoded.as_path(), path);
        assert_eq!(decoded.into_path_buf(), path);
        assert_eq!(format!("{parsed:?}"), "JavaExecutablePath([redacted])");
    }

    #[test]
    fn construction_and_json_reject_missing_or_relative_paths() {
        for path in ["", "relative-java", "./java"] {
            let error = JavaExecutablePath::try_from(path.to_owned()).unwrap_err();
            assert_eq!(error.to_string(), "Java executable path rejected");
            let _ =
                facet_json::from_str::<JavaExecutablePath>(&facet_json::to_string(path).unwrap())
                    .unwrap_err();
        }
    }
}
