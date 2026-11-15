use crate::broker::phone::config::PhoneProfile;

#[derive(Debug)]
pub enum PhoneConfigAction {
    Configure(PhoneProfile),
    Show { show_values: bool },
    Validate,
}
