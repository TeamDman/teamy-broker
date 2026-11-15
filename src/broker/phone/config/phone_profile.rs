use super::JavaExecutablePath;
use super::PhoneProfileMigration;
use super::TailscaleTailnetIpv4Addr;
use super::TailscaleTailnetNodeId;
use super::TailscaleTailnetNodeName;
use super::TailscaleTailnetUserId;
use super::phone_profile_preflight::PhoneProfilePreflight;
use super::phone_profile_v0::PhoneProfileV0;
use super::phone_profile_v1::PhoneProfileV1;
use crate::paths::AppHome;
#[cfg(test)]
use std::env;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::str;

const MAX_PROFILE_BYTES: usize = 8192;

/// Exact peer scope loaded at runtime.
#[derive(Clone, PartialEq, Eq)]
pub struct PhoneProfile {
    pub desktop: TailscaleTailnetIpv4Addr,
    pub phone: TailscaleTailnetIpv4Addr,
    pub desktop_node: TailscaleTailnetNodeId,
    pub phone_node: TailscaleTailnetNodeId,
    pub user: TailscaleTailnetUserId,
    pub phone_name: TailscaleTailnetNodeName,
    pub java: JavaExecutablePath,
}
impl fmt::Debug for PhoneProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PhoneProfile([redacted])")
    }
}
impl PhoneProfile {
    /// Load the current configuration from the supplied application home.
    ///
    /// # Errors
    /// Returns I/O, size limit, UTF-8, and configuration decoding errors.
    pub fn load(home: &AppHome) -> eyre::Result<Self> {
        let file = File::open(home.file_path("phone-profile.json"))?;
        let mut bytes = Vec::new();
        file.take(u64::try_from(MAX_PROFILE_BYTES + 1)?)
            .read_to_end(&mut bytes)?;
        eyre::ensure!(
            bytes.len() <= MAX_PROFILE_BYTES,
            "phone profile exceeds the {MAX_PROFILE_BYTES}-byte size limit"
        );
        let text = str::from_utf8(&bytes)?;
        let migration = Self::parse_json(text)?;
        for diagnostic in &migration.diagnostics {
            tracing::info!(
                source_schema_version = migration.source_schema_version,
                target_schema_version = PhoneProfileV1::SCHEMA_VERSION,
                diagnostic,
                "phone profile migrated in memory"
            );
        }
        Ok(migration.profile)
    }

    /// Read a supported schema and upgrade it without changing the source file.
    ///
    /// # Errors
    /// Returns JSON, unsupported-version, and profile validation errors.
    pub fn parse_json(input: &str) -> eyre::Result<PhoneProfileMigration> {
        let preflight: PhoneProfilePreflight = facet_json::from_str(input)?;
        let source_schema_version = preflight.schema_version()?;
        let (document, diagnostics) = match source_schema_version {
            0 => {
                let legacy: PhoneProfileV0 = facet_json::from_str(input)?;
                (
                    legacy.upgrade(),
                    vec![
                        "Upgraded legacy phone profile to schema_version 1; peer scope is unchanged.",
                    ],
                )
            }
            PhoneProfileV1::SCHEMA_VERSION => (facet_json::from_str(input)?, Vec::new()),
            version => eyre::bail!(
                "unsupported phone profile schema_version {version}; supported versions are 0 (legacy) and 1"
            ),
        };
        Ok(PhoneProfileMigration {
            profile: document.into_profile()?,
            source_schema_version,
            diagnostics,
        })
    }

    /// Serialize using the latest supported document schema.
    ///
    /// # Errors
    /// Returns profile validation, encoded size limit, or JSON serialization errors.
    pub fn to_json(&self) -> eyre::Result<String> {
        self.validate()?;
        let encoded = facet_json::to_string(&PhoneProfileV1::from(self))?;
        eyre::ensure!(
            encoded.len() <= MAX_PROFILE_BYTES,
            "phone profile exceeds the {MAX_PROFILE_BYTES}-byte size limit"
        );
        Ok(encoded)
    }

    /// Verify that an operation's configuration has not changed on disk.
    ///
    /// # Errors
    /// Rejects missing, invalid or different configuration.
    pub fn require_current(&self, home: &AppHome) -> eyre::Result<()> {
        eyre::ensure!(Self::load(home)? == *self, "phone profile changed");
        Ok(())
    }

    pub(super) fn validate(&self) -> eyre::Result<()> {
        eyre::ensure!(
            self.desktop != self.phone,
            "phone profile desktop and phone IP addresses must differ"
        );
        Ok(())
    }
}

#[cfg(test)]
pub(crate) fn synthetic() -> PhoneProfile {
    PhoneProfile {
        desktop: "100.64.0.1".parse().unwrap(),
        phone: "100.64.0.2".parse().unwrap(),
        desktop_node: TailscaleTailnetNodeId::try_from("test-desktop".to_owned()).unwrap(),
        phone_node: TailscaleTailnetNodeId::try_from("test-phone".to_owned()).unwrap(),
        user: TailscaleTailnetUserId::try_from(42).unwrap(),
        phone_name: TailscaleTailnetNodeName::try_from("phone.invalid.".to_owned()).unwrap(),
        java: JavaExecutablePath::try_from(env::temp_dir().join("synthetic-java")).unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Error as IoError;
    use std::io::ErrorKind;
    use std::net::Ipv4Addr;
    use std::str::Utf8Error;
    use uuid::Uuid;

    fn json() -> String {
        let java = facet_json::to_string(&env::temp_dir().join("synthetic-java").to_str().unwrap())
            .unwrap();
        format!(
            r#"{{
            "TEAMY_BROKER_DESKTOP_IP":"100.64.0.1",
            "TEAMY_BROKER_PHONE_IP":"100.64.0.2",
            "TEAMY_BROKER_DESKTOP_NODE":"test-desktop",
            "TEAMY_BROKER_PHONE_NODE":"test-phone",
            "TEAMY_BROKER_TAILNET_USER":"42",
            "TEAMY_BROKER_PHONE_NAME":"phone.invalid.",
            "TEAMY_BROKER_JAVA":{java}
        }}"#
        )
    }

    #[test]
    fn legacy_json_upgrades_and_serializes_as_v1() {
        let migration = PhoneProfile::parse_json(&json()).unwrap();
        assert_eq!(migration.source_schema_version, 0);
        assert_eq!(migration.diagnostics.len(), 1);
        let profile = migration.profile;
        assert_eq!(profile.desktop_node.as_str(), "test-desktop");
        assert_eq!(profile.phone_node.as_str(), "test-phone");
        assert_eq!(
            profile.desktop.get(),
            "100.64.0.1".parse::<Ipv4Addr>().unwrap()
        );
        assert_eq!(profile.user.get(), 42);
        assert_eq!(
            profile.java.as_path(),
            env::temp_dir().join("synthetic-java")
        );
        let encoded = profile.to_json().unwrap();
        assert!(encoded.contains(r#""schema_version":1"#));
        assert!(encoded.contains(r#""TEAMY_BROKER_TAILNET_USER":42"#));
        let current = PhoneProfile::parse_json(&encoded).unwrap();
        assert_eq!(current.source_schema_version, 1);
        assert!(current.diagnostics.is_empty());
        assert_eq!(current.profile, profile);
        assert_eq!(format!("{profile:?}"), "PhoneProfile([redacted])");
    }

    #[test]
    fn supported_schemas_enforce_profile_and_node_rules() {
        for json in [json(), json().replacen('{', r#"{"schema_version":1,"#, 1)] {
            for (before, after) in [
                ("100.64.0.2", "100.64.0.1"),
                ("100.64.0.2", "127.0.0.2"),
                ("100.64.0.1", "not-an-ip"),
                ("test-desktop", ""),
                ("test-phone", "invalid node"),
                ("phone.invalid.", "invalid name"),
                ("42", "0"),
                ("42", "not-a-number"),
            ] {
                let _ = PhoneProfile::parse_json(&json.replace(before, after)).unwrap_err();
            }
            let java =
                facet_json::to_string(&env::temp_dir().join("synthetic-java").to_str().unwrap())
                    .unwrap();
            let _ =
                PhoneProfile::parse_json(&json.replace(&java, r#""relative-java""#)).unwrap_err();
        }
    }

    #[test]
    fn unsupported_versions_are_rejected_before_decoding_profile_fields() {
        for version in [2, 99, u32::MAX] {
            let error = PhoneProfile::parse_json(&format!(r#"{{"schema_version":{version}}}"#))
                .unwrap_err();
            assert!(error.to_string().contains(&format!(
                "unsupported phone profile schema_version {version}"
            )));
        }
    }

    #[test]
    fn malformed_versions_and_incomplete_documents_are_rejected() {
        for version in [
            "null",
            "-1",
            "true",
            "1.5",
            r#""invalid""#,
            r#""1""#,
            "4294967296",
        ] {
            let input = json().replacen('{', &format!(r#"{{"schema_version":{version},"#), 1);
            let _ = PhoneProfile::parse_json(&input).unwrap_err();
        }
        for input in ["{}", r#"{"schema_version":1}"#, "{", "[]"] {
            let _ = PhoneProfile::parse_json(input).unwrap_err();
        }
    }

    #[test]
    fn explicit_legacy_version_preserves_the_same_profile() {
        let unversioned = PhoneProfile::parse_json(&json()).unwrap();
        let versioned =
            PhoneProfile::parse_json(&json().replacen('{', r#"{"schema_version":0,"#, 1)).unwrap();
        assert_eq!(versioned.source_schema_version, 0);
        assert_eq!(versioned.profile, unversioned.profile);
        assert_eq!(versioned.diagnostics, unversioned.diagnostics);
    }

    #[test]
    fn serialization_rejects_invalid_runtime_profiles() {
        let mut profile = synthetic();
        profile.phone = profile.desktop;
        let error = profile.to_json().unwrap_err();
        assert!(error.to_string().contains("IP addresses must differ"));
    }

    #[test]
    fn serialization_cannot_publish_a_profile_larger_than_the_reader_accepts() {
        let mut profile = synthetic();
        let encoded = profile.to_json().unwrap();
        assert!(encoded.len() <= MAX_PROFILE_BYTES);
        assert_eq!(PhoneProfile::parse_json(&encoded).unwrap().profile, profile);
        // Availability is deliberately irrelevant to this pure serialization boundary.
        // A configured path can otherwise make the document exceed the load limit.
        profile.java = JavaExecutablePath::try_from(
            env::temp_dir().join("synthetic-path".repeat(MAX_PROFILE_BYTES)),
        )
        .unwrap();
        assert_eq!(
            profile.to_json().unwrap_err().to_string(),
            "phone profile exceeds the 8192-byte size limit"
        );
    }

    #[test]
    fn loading_upgrades_in_memory_and_preserves_source_errors() {
        let home =
            AppHome(env::temp_dir().join(format!("teamy-phone-profile-schema-{}", Uuid::new_v4())));
        let path = home.file_path("phone-profile.json");
        let missing = PhoneProfile::load(&home).unwrap_err();
        assert_eq!(
            missing.downcast_ref::<IoError>().unwrap().kind(),
            ErrorKind::NotFound
        );
        home.ensure_dir().unwrap();
        for input in [json(), json().replacen('{', r#"{"schema_version":1,"#, 1)] {
            fs::write(&path, &input).unwrap();
            let profile = PhoneProfile::load(&home).unwrap();
            assert_eq!(profile, synthetic());
            assert_eq!(fs::read_to_string(&path).unwrap(), input);
        }
        fs::write(&path, [0xff]).unwrap();
        let invalid_utf8 = PhoneProfile::load(&home).unwrap_err();
        assert!(invalid_utf8.downcast_ref::<Utf8Error>().is_some());
        fs::write(&path, r#"{"schema_version":2}"#).unwrap();
        let unsupported = PhoneProfile::load(&home).unwrap_err();
        assert!(
            unsupported
                .to_string()
                .contains("unsupported phone profile schema_version 2")
        );
        fs::write(&path, vec![b' '; 8193]).unwrap();
        let oversized = PhoneProfile::load(&home).unwrap_err();
        assert!(oversized.to_string().contains("8192-byte size limit"));
        fs::remove_file(&path).unwrap();
        fs::remove_dir(&home.0).unwrap();
    }

    #[test]
    fn snapshot_rejects_replaced_invalid_and_removed_configuration() {
        let home =
            AppHome(env::temp_dir().join(format!("teamy-profile-snapshot-{}", Uuid::new_v4())));
        home.ensure_dir().unwrap();
        let path = home.file_path("phone-profile.json");
        let profile = synthetic();
        fs::write(&path, profile.to_json().unwrap()).unwrap();
        profile.require_current(&home).unwrap();

        let mut changed = profile.clone();
        changed.phone = "100.64.0.3".parse().unwrap();
        fs::write(&path, changed.to_json().unwrap()).unwrap();
        let _ = profile.require_current(&home).unwrap_err();
        changed.require_current(&home).unwrap();

        changed.java =
            JavaExecutablePath::try_from(env::temp_dir().join("other-synthetic-java")).unwrap();
        let _ = changed.require_current(&home).unwrap_err();
        fs::write(&path, "{}").unwrap();
        let _ = profile.require_current(&home).unwrap_err();
        fs::remove_file(&path).unwrap();
        let _ = profile.require_current(&home).unwrap_err();
        fs::remove_dir(&home.0).unwrap();
    }
}
