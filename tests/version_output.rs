use std::process::Command;

#[path = "../build_support/public_repository.rs"]
mod public_repository;

#[test]
fn version_output_includes_git_repository_metadata() {
    let output = Command::new(env!("CARGO_BIN_EXE_teamy-broker"))
        .arg("--version")
        .output()
        .expect("failed to run --version");

    assert!(
        output.status.success(),
        "--version failed: status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("--version output should be UTF-8");
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")), "{stdout}");
    assert!(stdout.contains("repo "), "{stdout}");
    assert!(stdout.contains(env!("GIT_REPOSITORY_URL")), "{stdout}");
    assert_eq!(env!("GIT_REPOSITORY_URL"), env!("CARGO_PKG_REPOSITORY"));
    assert!(stdout.contains("branch "), "{stdout}");
    assert!(stdout.contains(env!("GIT_BRANCH")), "{stdout}");
    assert!(stdout.contains("rev "), "{stdout}");
    assert!(stdout.contains(env!("GIT_REVISION")), "{stdout}");
    assert!(stdout.contains("worktree "), "{stdout}");
    assert!(stdout.contains(env!("GIT_WORKTREE_STATUS")), "{stdout}");
    assert!(stdout.contains("built "), "{stdout}");
}

#[test]
fn build_repository_identity_rejects_private_or_control_bearing_values() {
    use public_repository::public_repository;

    let public = "https://github.com/example/fixture";
    assert_eq!(public_repository(Some(public)), public);
    for value in [
        "https://fixture-user:fixture-token@example.invalid/repo",
        "https://example.invalid/repo?token=fixture",
        "https://example.invalid/repo#fixture",
        "https://example.invalid/repo\ncargo:rustc-env=INJECTED=fixture",
        "file:///fixture/repo",
        "C:\\fixture\\repo",
        "git@example.invalid:fixture/repo.git",
        "https:///repo",
        "https://example.invalid/",
    ] {
        assert_eq!(public_repository(Some(value)), "unknown");
    }
    assert_eq!(public_repository(None), "unknown");
}
