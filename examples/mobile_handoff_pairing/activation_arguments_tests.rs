use super::PairingActivation;
use std::ffi::OsString;
#[test]
fn default_is_inert_proposal() {
    assert_eq!(
        PairingActivation::parse(&[]).unwrap(),
        PairingActivation::Proposal
    );
}
#[test]
fn activation_without_phone_readiness_is_rejected() {
    assert!(PairingActivation::parse(&[OsString::from("--activate-dummy-pairing")]).is_err());
}
#[test]
fn readiness_alone_reordered_or_extra_arguments_are_rejected() {
    for values in [
        vec!["--phone-ready"],
        vec!["--phone-ready", "--activate-dummy-pairing"],
        vec!["--activate-dummy-pairing", "--phone-ready", "--retry"],
    ] {
        let arguments = values.into_iter().map(OsString::from).collect::<Vec<_>>();
        assert!(PairingActivation::parse(&arguments).is_err());
    }
}
#[test]
fn exact_explicit_activation_and_readiness_are_accepted() {
    let arguments = [
        OsString::from("--activate-dummy-pairing"),
        OsString::from("--phone-ready"),
    ];
    assert_eq!(
        PairingActivation::parse(&arguments).unwrap(),
        PairingActivation::Ready
    );
}
