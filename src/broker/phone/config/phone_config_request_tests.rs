use super::*;
use crate::broker::phone::config::JavaExecutablePath;
use crate::broker::phone::config::phone_profile::synthetic;
use crate::broker::phone::identity as phone_identity;
use std::env;
use std::net::Ipv4Addr;
use std::os::windows::fs::OpenOptionsExt;

struct Fixture {
    home: AppHome,
    store: Store,
    endpoint: String,
    profile: PhoneProfile,
}

impl Fixture {
    fn new() -> Self {
        let id = Uuid::new_v4();
        let home = AppHome(env::temp_dir().join(format!("teamy-config-test-{id}")));
        home.ensure_dir().unwrap();
        let java = home.file_path("synthetic-java.exe");
        fs::write(&java, b"test availability only; never executed").unwrap();
        let mut profile = synthetic();
        profile.java = JavaExecutablePath::try_from(java).unwrap();
        Self {
            store: Store(home.file_path("synthetic-identity.dpapi")),
            home,
            endpoint: format!(r"\\.\pipe\teamy-config-test-{id}"),
            profile,
        }
    }

    fn save(&self, profile: &PhoneProfile) -> eyre::Result<PhoneConfigReport> {
        PhoneConfigRequest::configure(&self.home, &self.store, &self.endpoint, profile)
    }

    fn profile_bytes(&self) -> Vec<u8> {
        fs::read(self.home.file_path("phone-profile.json")).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Ok(entries) = fs::read_dir(&self.home.0) {
            for entry in entries.flatten() {
                let _ = fs::remove_file(entry.path());
            }
        }
        let _ = fs::remove_dir(&self.home.0);
    }
}

#[tokio::test]
async fn runtime_profiles_change_without_rebuild_and_show_is_redacted() {
    let fixture = Fixture::new();
    assert!(fixture.save(&fixture.profile).unwrap().changed);
    assert!(!fixture.save(&fixture.profile).unwrap().changed);
    let mut second = fixture.profile.clone();
    second.phone = "100.64.0.3".parse().unwrap();
    assert!(fixture.save(&second).unwrap().changed);
    assert_eq!(PhoneProfile::load(&fixture.home).unwrap(), second);
    let request = PhoneConfigRequest::new(
        Some(fixture.home.0.clone()),
        PhoneConfigAction::Show { show_values: false },
    )
    .unwrap();
    let redacted = facet_json::to_string(&request.invoke().unwrap()).unwrap();
    for private in [
        "100.64.0.3",
        "test-desktop",
        "phone.invalid.",
        "synthetic-java",
    ] {
        assert!(!redacted.contains(private));
    }
    let report = PhoneConfigRequest::new(
        Some(fixture.home.0.clone()),
        PhoneConfigAction::Show { show_values: true },
    )
    .unwrap()
    .invoke()
    .unwrap();
    assert_eq!(report.values.unwrap().phone_ip, "100.64.0.3");
}

#[tokio::test]
async fn active_daemon_excludes_configuration_publication() {
    let fixture = Fixture::new();
    fixture.save(&fixture.profile).unwrap();
    let before = fixture.profile_bytes();
    let _daemon = pipe_security::create(&fixture.endpoint, true).unwrap();
    let mut second = fixture.profile.clone();
    second.phone = "100.64.0.3".parse().unwrap();
    assert!(matches!(
        fixture
            .save(&second)
            .unwrap_err()
            .downcast_ref::<PhoneConfigError>(),
        Some(PhoneConfigError::BrokerActive)
    ));
    assert_eq!(fixture.profile_bytes(), before);
}

#[tokio::test]
async fn paired_trust_accepts_only_identical_profile_and_never_changes_identity() {
    let fixture = Fixture::new();
    fixture.save(&fixture.profile).unwrap();
    let mut record = phone_identity::generate(Ipv4Addr::LOCALHOST).unwrap();
    record.phone_certificate = Some(vec![1]);
    fixture.store.save(&record).unwrap();
    let identity = fs::read(&fixture.store.0).unwrap();
    let before = fixture.profile_bytes();
    assert!(!fixture.save(&fixture.profile).unwrap().changed);
    let mut changed = fixture.profile.clone();
    changed.phone = "100.64.0.3".parse().unwrap();
    drop(fixture.save(&changed).unwrap_err());
    assert_eq!(fixture.profile_bytes(), before);
    assert_eq!(fs::read(&fixture.store.0).unwrap(), identity);
    fs::remove_file(fixture.home.file_path("phone-profile.json")).unwrap();
    drop(fixture.save(&fixture.profile).unwrap_err());
    assert!(!fixture.home.file_path("phone-profile.json").exists());
    assert_eq!(fs::read(&fixture.store.0).unwrap(), identity);
}

#[tokio::test]
async fn corrupt_trust_or_unavailable_java_never_changes_configuration() {
    let fixture = Fixture::new();
    fixture.save(&fixture.profile).unwrap();
    let before = fixture.profile_bytes();
    let mut changed = fixture.profile.clone();
    changed.phone = "100.64.0.3".parse().unwrap();
    fs::write(&fixture.store.0, b"invalid protected identity").unwrap();
    drop(fixture.save(&changed).unwrap_err());
    assert_eq!(fixture.profile_bytes(), before);
    fs::remove_file(&fixture.store.0).unwrap();
    fs::remove_file(changed.java.as_path()).unwrap();
    drop(fixture.save(&changed).unwrap_err());
    assert_eq!(fixture.profile_bytes(), before);
}

#[tokio::test]
async fn failed_atomic_replacement_preserves_original_and_removes_staging() {
    let fixture = Fixture::new();
    fixture.save(&fixture.profile).unwrap();
    let before = fixture.profile_bytes();
    let locked = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(fixture.home.file_path("phone-profile.json"))
        .unwrap();
    let mut changed = fixture.profile.clone();
    changed.phone = "100.64.0.3".parse().unwrap();
    drop(fixture.save(&changed).unwrap_err());
    drop(locked);
    assert_eq!(fixture.profile_bytes(), before);
    assert_eq!(fs::read_dir(&fixture.home.0).unwrap().count(), 2);
}

#[test]
fn request_conversion_rejects_relative_homes_without_io() {
    drop(
        PhoneConfigRequest::new(Some(PathBuf::from("relative")), PhoneConfigAction::Validate)
            .unwrap_err(),
    );
}
