use super::phone_config_action::PhoneConfigAction;
use super::phone_config_error::PhoneConfigError;
use super::phone_config_report::PhoneConfigReport;
use super::phone_config_values::PhoneConfigValues;
use crate::broker::phone::config::PhoneProfile;
use crate::broker::phone::store;
use crate::broker::phone::store::Store;
use crate::paths::AppHome;
use crate::platform::identity;
use crate::platform::pipe_security;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use uuid::Uuid;

/// Validated operation inputs. Platform state is resolved only when invoked.
#[derive(Debug)]
pub struct PhoneConfigRequest {
    home: Option<AppHome>,
    action: PhoneConfigAction,
}

impl PhoneConfigRequest {
    /// # Errors
    /// Rejects a relative explicit home or an invalid profile.
    pub fn new(home: Option<PathBuf>, action: PhoneConfigAction) -> eyre::Result<Self> {
        eyre::ensure!(
            home.as_ref().is_none_or(|path| path.is_absolute()),
            PhoneConfigError::RelativeHome
        );
        if let PhoneConfigAction::Configure(profile) = &action {
            profile.to_json()?;
        }
        Ok(Self {
            home: home.map(AppHome),
            action,
        })
    }

    /// # Errors
    /// Rejects unavailable configuration, an active daemon, or a paired profile change.
    pub fn invoke(self) -> eyre::Result<PhoneConfigReport> {
        let home = self.home.map_or_else(AppHome::resolve, Ok)?;
        match self.action {
            PhoneConfigAction::Configure(profile) => {
                let endpoint = identity::endpoint()?;
                let store = Store::current_user()?;
                Self::configure(&home, &store, &endpoint, &profile)
            }
            PhoneConfigAction::Show { show_values } => {
                let profile = Self::load(&home)?;
                Ok(Self::report(&profile, false, show_values))
            }
            PhoneConfigAction::Validate => {
                let profile = Self::load(&home)?;
                profile
                    .java
                    .require_available()
                    .map_err(|_redacted| PhoneConfigError::JavaUnavailable)?;
                Ok(Self::report(&profile, false, false))
            }
        }
    }

    fn load(home: &AppHome) -> eyre::Result<PhoneProfile> {
        PhoneProfile::load(home)
            .map_err(|_redacted| PhoneConfigError::MissingOrInvalidProfile.into())
    }

    fn report(profile: &PhoneProfile, changed: bool, show_values: bool) -> PhoneConfigReport {
        PhoneConfigReport {
            schema_version: 1,
            changed,
            java_available: profile.java.as_path().is_file(),
            values: show_values.then(|| PhoneConfigValues::from(profile)),
        }
    }

    fn configure(
        home: &AppHome,
        store: &Store,
        endpoint: &str,
        profile: &PhoneProfile,
    ) -> eyre::Result<PhoneConfigReport> {
        let encoded = profile.to_json()?;
        profile
            .java
            .require_available()
            .map_err(|_redacted| PhoneConfigError::JavaUnavailable)?;
        // The first pipe instance is the daemon's own exclusion mechanism. Hold it
        // across trust inspection and publication so another daemon cannot start.
        let _stopped = pipe_security::create(endpoint, true)
            .map_err(|_redacted| PhoneConfigError::BrokerActive)?;
        let paired = store
            .load()
            .map_err(|_redacted| PhoneConfigError::ProtectedIdentityUnavailable)?
            .is_some_and(|record| record.phone_certificate.is_some());
        if paired {
            eyre::ensure!(
                Self::load(home)? == *profile,
                PhoneConfigError::PairedProfileChange
            );
            return Ok(Self::report(profile, false, false));
        }
        let path = home.file_path("phone-profile.json");
        if fs::read(&path).is_ok_and(|bytes| bytes == encoded.as_bytes()) {
            return Ok(Self::report(profile, false, false));
        }
        Self::publish(home, encoded.as_bytes())
            .map_err(|_redacted| PhoneConfigError::PublicationFailed)?;
        Ok(Self::report(profile, true, false))
    }

    fn publish(home: &AppHome, bytes: &[u8]) -> eyre::Result<()> {
        home.ensure_dir()?;
        let temporary = home.file_path(&format!("phone-profile-{}.tmp", Uuid::new_v4()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            drop(file);
            store::replace(&temporary, &home.file_path("phone-profile.json"))
        })();
        let _ = fs::remove_file(&temporary);
        result
    }
}

#[cfg(test)]
#[path = "phone_config_request_tests.rs"]
mod tests;
