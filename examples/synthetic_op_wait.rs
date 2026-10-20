//! Offline test-only provider fixture. Never accepts a real reference.
use std::io::Write;
fn main() -> eyre::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    eyre::ensure!(
        args.get(1).map(String::as_str) == Some("read")
            && args.last().map(String::as_str) == Some("op://Synthetic/Example/token"),
        "synthetic fixture only"
    );
    let marker =
        std::path::PathBuf::from(args.get(3).ok_or_else(|| eyre::eyre!("missing marker"))?);
    eyre::ensure!(
        marker.starts_with(std::env::temp_dir())
            && marker
                .file_name()
                .is_some_and(|s| s == "synthetic-provider.pid"),
        "temporary fixture marker required"
    );
    std::fs::write(marker, std::process::id().to_string())?;
    std::thread::sleep(std::time::Duration::from_secs(30));
    std::io::stdout().write_all(b"synthetic-provider-value")?;
    Ok(())
}
