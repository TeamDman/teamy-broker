use sha2::Digest;
use std::env::current_exe;
use std::fs::File;
use std::io;
use std::io::Read;
pub(crate) fn executable() -> (String, Option<String>) {
    let Ok(path) = current_exe() else {
        return ("unavailable".into(), None);
    };
    let hash = (|| -> io::Result<String> {
        let mut input = File::open(&path)?;
        let mut digest = sha2::Sha256::new();
        let mut buffer = [0_u8; 8192];
        loop {
            let count = input.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            digest.update(&buffer[..count]);
        }
        Ok(format!("{:X}", digest.finalize()))
    })()
    .ok();
    (path.to_string_lossy().into_owned(), hash)
}
