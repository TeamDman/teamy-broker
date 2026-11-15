use crate::broker::app_request::AppRequest;
use crate::protocol::pairing_comparison::PairingCode;
use arbitrary::Arbitrary;
use facet::Facet;
use figue as args;
use std::io;
use std::io::Read;
use std::io::Result as IoResult;
use std::os::windows::io::AsRawHandle;
use std::str;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use teamy_cancellation::CancellationToken;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::IO::CancelSynchronousIo;
use zeroize::Zeroizing;

const COMPARISON_INPUT_LIMIT: Duration = Duration::from_mins(5);
const CANCELLATION_POLL: Duration = Duration::from_millis(10);
#[derive(Facet, Arbitrary, Debug, PartialEq)]
pub struct PhonePairConfirmArgs {
    /// Exact active attempt UUID returned by phone-pair. No latest or wildcard.
    #[facet(args::named)]
    pub attempt_id: String,
    /// Read the peer-presented comparison from stdin, with up to five minutes to enter it.
    #[facet(args::named, default)]
    pub comparison_stdin: bool,
}

impl PhonePairConfirmArgs {
    /// # Errors
    /// Requires an exact attempt UUID and separately collected comparison input.
    pub fn into_request(self, comparison: Option<PairingCode>) -> eyre::Result<AppRequest> {
        let attempt_id = self.attempt_id.try_into()?;
        eyre::ensure!(self.comparison_stdin, "--comparison-stdin required");
        let code = comparison.ok_or_else(|| eyre::eyre!("comparison input required"))?;
        Ok(AppRequest::PairingConfirm { attempt_id, code })
    }

    pub(crate) fn validate(&self) -> eyre::Result<()> {
        uuid::Uuid::parse_str(&self.attempt_id)
            .map_err(|_redacted_error| eyre::eyre!("invalid attempt ID"))?;
        eyre::ensure!(self.comparison_stdin, "--comparison-stdin required");
        Ok(())
    }

    pub(crate) fn read_comparison(
        &self,
        cancellation: &CancellationToken,
    ) -> eyre::Result<PairingCode> {
        self.validate()?;
        read_code(io::stdin(), cancellation, COMPARISON_INPUT_LIMIT)
    }
}

fn read_code(
    reader: impl Read + Send + 'static,
    cancellation: &CancellationToken,
    timeout: Duration,
) -> eyre::Result<PairingCode> {
    cancellation.bail_if_cancelled()?;
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop);
    let worker = thread::Builder::new()
        .name("pairing-comparison-input".into())
        .spawn(move || read_line(reader, &worker_stop))
        .map_err(|_redacted_error| eyre::eyre!("comparison input unavailable"))?;
    let started = Instant::now();
    let mut expired = false;
    while !worker.is_finished() {
        expired |= started.elapsed() >= timeout;
        if expired || cancellation.is_cancelled() {
            stop.store(true, Ordering::Release);
            // SAFETY: JoinHandle owns this live thread handle until join below.
            // Cancellation is retried: ERROR_NOT_FOUND can mean the worker has
            // not entered ReadFile/ReadConsole yet, rather than that it finished.
            let _ = unsafe { CancelSynchronousIo(HANDLE(worker.as_raw_handle())) };
        }
        thread::sleep(CANCELLATION_POLL);
    }
    let result = worker
        .join()
        .map_err(|_redacted_error| eyre::eyre!("comparison input unavailable"))?;
    cancellation.bail_if_cancelled()?;
    eyre::ensure!(!expired, "comparison input deadline");
    let bytes = result.map_err(|_redacted_error| eyre::eyre!("comparison input rejected"))?;
    eyre::ensure!(bytes.len() <= 128, "comparison input rejected");
    let text = str::from_utf8(&bytes)
        .map_err(|_redacted_error| eyre::eyre!("comparison input rejected"))?
        .trim_end_matches(['\r', '\n']);
    Ok(PairingCode {
        comparison: text.into(),
    })
}

fn read_line(reader: impl Read, stop: &AtomicBool) -> IoResult<Zeroizing<Vec<u8>>> {
    let mut bytes = Zeroizing::new(Vec::new());
    let mut input = reader.take(129);
    let mut byte = [0u8; 1];
    while !stop.load(Ordering::Acquire) && input.read(&mut byte)? != 0 {
        bytes.push(byte[0]);
        if byte[0] == b'\n' {
            break;
        }
    }
    Ok(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::io::PipeReader;
    use std::sync::mpsc;
    #[test]
    fn stdin_bounds_and_newline_only() {
        let t = Duration::from_secs(1);
        let cancellation = CancellationToken::new();
        let c = read_code(
            Cursor::new(b"ABCD 1234 DCBA 5678\r\n".to_vec()),
            &cancellation,
            t,
        )
        .unwrap();
        assert_eq!(c.comparison, "ABCD 1234 DCBA 5678");
        let _ = read_code(Cursor::new(vec![b'x'; 129]), &cancellation, t)
            .expect_err("expected rejection");
        let _ =
            read_code(Cursor::new(vec![255]), &cancellation, t).expect_err("expected rejection");
        assert!(!format!("{c:?}").contains("ABCD"));
    }
    #[test]
    fn blocked_input_has_deadline() {
        struct Slow;
        impl Read for Slow {
            fn read(&mut self, _: &mut [u8]) -> IoResult<usize> {
                thread::sleep(Duration::from_millis(30));
                Ok(0)
            }
        }
        let _ = read_code(Slow, &CancellationToken::new(), Duration::from_millis(1))
            .expect_err("expected rejection");
    }

    #[test]
    fn cancellation_before_native_read_is_retried_and_worker_is_joined() {
        struct DelayedPipe {
            reader: PipeReader,
            before_read: mpsc::Sender<()>,
            dropped: Arc<AtomicBool>,
        }
        impl Read for DelayedPipe {
            fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
                self.before_read.send(()).unwrap();
                // The first native cancellation sees no pending I/O. The reader
                // enters a blocking pipe read only after cancellation was requested.
                thread::sleep(Duration::from_millis(100));
                self.reader.read(buf)
            }
        }
        impl Drop for DelayedPipe {
            fn drop(&mut self) {
                self.dropped.store(true, Ordering::Release);
            }
        }

        let (reader, writer) = io::pipe().unwrap();
        let (before_read, entered) = mpsc::channel();
        let (release, released) = mpsc::channel();
        // EOF is a fallback so a cancellation regression fails instead of hanging.
        let pipe_owner = thread::spawn(move || {
            let _ = released.recv_timeout(Duration::from_secs(2));
            drop(writer);
        });
        let cancellation = CancellationToken::new();
        let trigger = cancellation.clone();
        let canceller = thread::spawn(move || {
            entered.recv_timeout(Duration::from_secs(1)).unwrap();
            trigger.request_cancel("comparison cancelled for test");
        });
        let dropped = Arc::new(AtomicBool::new(false));
        let started = Instant::now();
        let result = read_code(
            DelayedPipe {
                reader,
                before_read,
                dropped: Arc::clone(&dropped),
            },
            &cancellation,
            COMPARISON_INPUT_LIMIT,
        );
        let elapsed = started.elapsed();
        let _ = release.send(());
        pipe_owner.join().unwrap();
        canceller.join().unwrap();
        assert!(
            dropped.load(Ordering::Acquire),
            "input worker must finish before returning"
        );
        assert!(
            elapsed < Duration::from_secs(1),
            "cancelled input waited for EOF: {elapsed:?}"
        );
        assert_eq!(
            result.unwrap_err().to_string(),
            "comparison cancelled for test"
        );
    }
}

#[cfg(test)]
mod line_tests {
    use super::*;
    use std::io::Cursor;
    #[test]
    fn enter_finishes_without_eof() {
        struct LineThenBlock {
            bytes: Cursor<Vec<u8>>,
        }
        impl Read for LineThenBlock {
            fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
                let n = self.bytes.read(buf)?;
                if n == 0 {
                    thread::sleep(Duration::from_millis(40));
                }
                Ok(n)
            }
        }
        let code = read_code(
            LineThenBlock {
                bytes: Cursor::new(b"ABCD 1234 DCBA 5678\n".to_vec()),
            },
            &CancellationToken::new(),
            Duration::from_millis(20),
        )
        .unwrap();
        assert_eq!(code.comparison, "ABCD 1234 DCBA 5678");
    }
}
