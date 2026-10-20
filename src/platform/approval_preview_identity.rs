use sha2::Digest;
use std::io::Read;
pub(crate) fn executable() -> (String, Option<String>) {
    let Ok(path) = std::env::current_exe() else {
        return ("unavailable".into(), None);
    };
    let hash = (|| -> std::io::Result<String> {
        let mut input = std::fs::File::open(&path)?;
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
