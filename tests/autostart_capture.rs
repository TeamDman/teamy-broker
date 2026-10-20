use std::time::Duration;
use teamy_broker::sdk::Client;
#[tokio::test]
async fn redirected_consumer_exits_with_eof_while_its_daemon_remains_alive() {
    let examples = std::path::Path::new(env!("CARGO_BIN_EXE_teamy-broker"))
        .parent()
        .unwrap()
        .join("examples");
    let helper = examples.join("synthetic_autostart_consumer.exe");
    let daemon = examples.join("synthetic_broker.exe");
    assert!(helper.is_file() && daemon.is_file(), "build examples first");
    let endpoint = format!(r"\\.\pipe\teamy-broker-test-{}", uuid::Uuid::new_v4());
    let reader = Client::new(None)
        .unwrap()
        .with_endpoint(endpoint.clone())
        .with_deadlines(Duration::from_secs(2), Duration::from_secs(3));
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::process::Command::new(helper)
            .args([daemon.to_str().unwrap(), &endpoint])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .creation_flags(0x0800_0000)
            .kill_on_drop(true)
            .output(),
    )
    .await;
    // Check BEFORE shutdown: output collection must not depend on stopping the daemon.
    let was_running = reader.status().await.unwrap().running;
    if was_running {
        reader.stop_force().await.unwrap();
    }
    assert!(
        was_running,
        "synthetic daemon should remain alive after its consumer exits"
    );
    let output = result
        .expect("consumer/output EOF waited for broker shutdown")
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "synthetic consumer complete\n"
    );
    assert!(output.stderr.is_empty());
    assert!(!reader.status().await.unwrap().running);
}
