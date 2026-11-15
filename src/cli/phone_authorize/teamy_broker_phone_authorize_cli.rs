use crate::broker::synthetic_credential_fixture;
use crate::protocol::AcquireRequest;
use crate::protocol::Backend;
use crate::protocol::Request;
use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;
/// Explicit single exact request permission, valid five minutes; no stored trust expansion.
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct PhoneAuthorizeArgs {
    #[facet(args::named)]
    pub account: String,
    #[facet(args::named)]
    pub secret_ref: String,
    #[facet(args::named)]
    pub requesting_app: String,
    #[facet(args::named)]
    pub purpose: String,
    #[facet(args::named)]
    pub destination: String,
    /// Also permit explicit phone or desktop value supply for this request only.
    #[facet(args::named, default)]
    pub supply: bool,
    /// Built-in isolated fixture only: no live provider or arbitrary entered values.
    #[facet(args::named, default)]
    pub synthetic_fixture: bool,
}
impl PhoneAuthorizeArgs {
    /// # Errors
    /// Rejects invalid metadata, unavailable state, cancellation or expiry.
    pub fn request(self) -> eyre::Result<Request> {
        let request = Request {
            operation: if self.synthetic_fixture {
                "phone-authorize-fixture"
            } else if self.supply {
                "phone-authorize-supply"
            } else {
                "phone-authorize-desktop"
            }
            .into(),
            acquire: Some(AcquireRequest {
                account: self.account,
                secret_ref: self.secret_ref,
                requesting_app: self.requesting_app,
                purpose: self.purpose,
                destination: self.destination,
                backend: Backend::Memory,
            }),
            lease_id: None,
        };
        request.validate()?;
        if self.synthetic_fixture {
            synthetic_credential_fixture::validate(
                request
                    .acquire
                    .as_ref()
                    .ok_or_else(|| eyre::eyre!("missing fixture"))?,
            )?;
        }
        Ok(request)
    }
}
