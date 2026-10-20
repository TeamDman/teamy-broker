use ring::rand::SecureRandom;
use ring::rand::SystemRandom;
pub fn random<const N: usize>() -> eyre::Result<[u8; N]> {
    let mut bytes = [0; N];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_redacted_error| eyre::eyre!("random unavailable"))?;
    Ok(bytes)
}
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 15)]));
    }
    text
}
pub fn unhex(text: &str) -> eyre::Result<zeroize::Zeroizing<Vec<u8>>> {
    eyre::ensure!(
        text.len() <= 32768 && text.len().is_multiple_of(2),
        "invalid encoding"
    );
    let digit = |c: u8| -> eyre::Result<u8> {
        match c {
            b'0'..=b'9' => Ok(c - b'0'),
            b'a'..=b'f' => Ok(c - b'a' + 10),
            _ => eyre::bail!("invalid encoding"),
        }
    };
    let mut bytes = zeroize::Zeroizing::new(Vec::with_capacity(text.len() / 2));
    for pair in text.as_bytes().chunks_exact(2) {
        bytes.push(digit(pair[0])? * 16 + digit(pair[1])?);
    }
    Ok(bytes)
}
