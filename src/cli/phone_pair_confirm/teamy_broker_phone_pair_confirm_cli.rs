use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;
use std::io::Read;
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct PhonePairConfirmArgs {
    /// Exact active attempt UUID returned by phone-pair. No latest or wildcard.
    #[facet(args::named)]
    pub attempt_id: String,
    /// Read the peer-presented four-group comparison from bounded stdin, never argv.
    #[facet(args::named, default)]
    pub comparison_stdin: bool,
}

pub(crate) fn read_code(
    reader: impl std::io::Read + Send + 'static,
    timeout: std::time::Duration,
) -> eyre::Result<crate::protocol::pairing_comparison::PairingCode> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut bytes = zeroize::Zeroizing::new(Vec::new());
        let mut input = reader.take(129);
        let result = (|| -> std::io::Result<_> {
            let mut byte = [0u8; 1];
            while input.read(&mut byte)? != 0 {
                bytes.push(byte[0]);
                if byte[0] == b'\n' {
                    break;
                }
            }
            Ok(bytes)
        })();
        let _ = sender.send(result);
    });
    let bytes = receiver
        .recv_timeout(timeout)
        .map_err(|_redacted_error| eyre::eyre!("comparison input deadline"))?
        .map_err(|_redacted_error| eyre::eyre!("comparison input rejected"))?;
    eyre::ensure!(bytes.len() <= 128, "comparison input rejected");
    let text = std::str::from_utf8(&bytes)
        .map_err(|_redacted_error| eyre::eyre!("comparison input rejected"))?
        .trim_end_matches(['\r', '\n']);
    Ok(crate::protocol::pairing_comparison::PairingCode {
        comparison: text.into(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stdin_bounds_and_newline_only() {
        let t = std::time::Duration::from_secs(1);
        let c = read_code(std::io::Cursor::new(b"ABCD 1234 DCBA 5678\r\n".to_vec()), t).unwrap();
        assert_eq!(c.comparison, "ABCD 1234 DCBA 5678");
        let _ =
            read_code(std::io::Cursor::new(vec![b'x'; 129]), t).expect_err("expected rejection");
        let _ = read_code(std::io::Cursor::new(vec![255]), t).expect_err("expected rejection");
        assert!(!format!("{c:?}").contains("ABCD"));
    }
    #[test]
    fn blocked_input_has_deadline() {
        struct Slow;
        impl std::io::Read for Slow {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                std::thread::sleep(std::time::Duration::from_millis(30));
                Ok(0)
            }
        }
        let _ =
            read_code(Slow, std::time::Duration::from_millis(1)).expect_err("expected rejection");
    }
}

#[cfg(test)]
mod line_tests {
    use super::*;
    #[test]
    fn enter_finishes_without_eof() {
        struct LineThenBlock {
            bytes: std::io::Cursor<Vec<u8>>,
        }
        impl std::io::Read for LineThenBlock {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                let n = self.bytes.read(buf)?;
                if n == 0 {
                    std::thread::sleep(std::time::Duration::from_millis(40));
                }
                Ok(n)
            }
        }
        let code = read_code(
            LineThenBlock {
                bytes: std::io::Cursor::new(b"ABCD 1234 DCBA 5678\n".to_vec()),
            },
            std::time::Duration::from_millis(20),
        )
        .unwrap();
        assert_eq!(code.comparison, "ABCD 1234 DCBA 5678");
    }
}
