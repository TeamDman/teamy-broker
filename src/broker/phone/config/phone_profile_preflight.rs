use eyre::WrapErr;
use facet::Facet;
use facet_json::RawJson;

/// Version zero denotes the original unversioned profile layout.
#[derive(Debug, Facet)]
pub(super) struct PhoneProfilePreflight {
    // Capture the value so an explicit null cannot become the legacy default.
    #[facet(default = RawJson::new("0"))]
    schema_version: RawJson<'static>,
}

impl PhoneProfilePreflight {
    pub fn schema_version(&self) -> eyre::Result<u32> {
        self.schema_version
            .as_str()
            .trim()
            .parse()
            .wrap_err("phone profile schema_version must be a nonnegative JSON integer")
    }
}
