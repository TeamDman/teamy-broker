use crate::broker::phone::config::phone_config_action::PhoneConfigAction;
use crate::broker::phone::config::phone_config_request::PhoneConfigRequest;
use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;
use std::path::PathBuf;

#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct PhoneConfigShowArgs {
    /// Explicit absolute configuration directory; defaults to the application home.
    #[facet(args::named)]
    pub app_home: Option<String>,
    /// Include private peer identifiers and the Java path in command output.
    #[facet(args::named, default)]
    pub show_values: bool,
}
impl PhoneConfigShowArgs {
    /// # Errors
    /// Rejects a relative application home.
    pub fn into_request(self) -> eyre::Result<PhoneConfigRequest> {
        PhoneConfigRequest::new(
            self.app_home.map(PathBuf::from),
            PhoneConfigAction::Show {
                show_values: self.show_values,
            },
        )
    }
}
