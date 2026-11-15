use super::phone_config_values::PhoneConfigValues;
use facet::Facet;

/// Effective schema and local runtime readiness; contains no values by default.
#[derive(Debug, Facet)]
pub struct PhoneConfigReport {
    pub schema_version: u32,
    pub changed: bool,
    pub java_available: bool,
    pub values: Option<PhoneConfigValues>,
}
