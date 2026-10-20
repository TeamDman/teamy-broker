#[test]
fn windows_resources_remain_wired_into_the_build() {
    let manifest = include_str!("../resources/app.manifest");
    assert!(manifest.contains("PerMonitorV2,PerMonitor"));
    assert!(manifest.contains(">true/pm</dpiAware>"));
    assert!(manifest.contains("Microsoft.Windows.Common-Controls"));
    let resource = include_str!("../resources/app.rc");
    assert!(resource.contains("1 RT_MANIFEST \"app.manifest\""));
    let build = include_str!("../build.rs");
    assert!(build.contains("rerun-if-changed=resources"));
    assert!(build.contains(".manifest_required()"));
    assert!(build.contains("embed_resource::compile_for_everything("));
}

#[cfg(windows)]
#[test]
fn built_executable_has_native_dpi_manifest() {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(concat!(env!("CARGO_PKG_NAME"), ".exe"));
    for executable in [executable, std::env::current_exe().unwrap()] {
        let output = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/check-windows-manifest.ps1"
            ))
            .arg("-Executable")
            .arg(executable)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "manifest check failed: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
