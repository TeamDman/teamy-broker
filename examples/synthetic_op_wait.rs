//! Offline test-only provider fixture. Never accepts a real reference.
use std::env;
use std::fs;
use std::io;
use std::io::Write;
use std::path::PathBuf;
use std::process;
use std::thread;
use std::time::Duration;
fn main() -> eyre::Result<()> {
    let args: Vec<_> = env::args().collect();
    eyre::ensure!(
        args.get(1).map(String::as_str) == Some("read")
            && args.last().map(String::as_str) == Some("op://Synthetic/Example/token"),
        "synthetic fixture only"
    );
    let marker = PathBuf::from(args.get(3).ok_or_else(|| eyre::eyre!("missing marker"))?);
    eyre::ensure!(
        marker.starts_with(env::temp_dir())
            && marker
                .file_name()
                .is_some_and(|s| s == "synthetic-provider.pid"),
        "temporary fixture marker required"
    );
    fs::write(marker, process::id().to_string())?;
    thread::sleep(Duration::from_secs(30));
    io::stdout().write_all(b"synthetic-provider-value")?;
    Ok(())
}
