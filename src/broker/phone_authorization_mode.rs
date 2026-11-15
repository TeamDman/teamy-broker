/// The permission being granted to one exact acquisition request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhoneAuthorizationMode {
    DesktopOnly,
    AllowSupply,
    SyntheticFixture,
}
