use super::phone_profile::PhoneProfile;

/// A decoded runtime profile and the changes made while reading its source schema.
#[derive(Debug)]
pub struct PhoneProfileMigration {
    pub profile: PhoneProfile,
    pub source_schema_version: u32,
    pub diagnostics: Vec<&'static str>,
}
