use std::path::Path;
use std::process::Command;

pub fn command() -> Command {
    let helper = Path::new(env!("CARGO_BIN_EXE_teamy-broker"))
        .parent()
        .expect("binary directory")
        .join("examples/synthetic_logging_cli.exe");
    assert!(
        helper.is_file(),
        "build the synthetic_logging_cli example before the logging integration tests"
    );
    let mut command = Command::new(helper);
    command.env_remove("RUST_LOG");
    command
}
