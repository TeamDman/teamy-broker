use std::process::ExitCode;
use teamy_broker::broker::failure_message;
fn main() -> ExitCode {
    teamy_broker::entrypoint_with_failure_message(failure_message::classify)
}
