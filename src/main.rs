fn main() -> std::process::ExitCode {
    teamy_broker::entrypoint_with_failure_message(teamy_broker::broker::failure_message::classify)
}
