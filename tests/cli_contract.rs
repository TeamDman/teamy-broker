use std::env;
use teamy_broker::broker::app_request::AppRequest;
use teamy_broker::broker::purge_scope::PurgeScope;
use teamy_broker::cli::Cli;
use teamy_broker::protocol::Request;
use teamy_broker::protocol::wire;
use tokio::io;
use tokio::io::AsyncWriteExt;
fn parse(args: &[&str]) -> eyre::Result<Cli> {
    figue::Driver::new(
        figue::builder::<Cli>()?
            .cli(|cli| cli.args(args.iter().copied()).strict())
            .build(),
    )
    .run()
    .into_result()
    .map(|output| output.value)
    .map_err(|_redacted_error| eyre::eyre!("invalid CLI"))
}
#[test]
fn all_command_schemas_convert_to_typed_requests() {
    assert!(matches!(
        parse(&["status"]).unwrap().command.into_request().unwrap(),
        AppRequest::Status
    ));
    assert!(matches!(
        parse(&["stop"]).unwrap().command.into_request().unwrap(),
        AppRequest::StopWhenIdle
    ));
    assert!(matches!(
        parse(&["serve"]).unwrap().command.into_request().unwrap(),
        AppRequest::Serve { .. }
    ));
    let id = uuid::Uuid::new_v4().to_string();
    let AppRequest::Purge(PurgeScope::Lease(lease_id)) = parse(&["purge", "--lease-id", &id])
        .unwrap()
        .command
        .into_request()
        .unwrap()
    else {
        panic!("single-lease purge expected")
    };
    assert_eq!(lease_id.as_str(), id);
}
#[test]
fn purge_requires_exactly_one_scope_and_invalid_ids_do_not_dispatch() {
    for args in [
        vec!["purge"],
        vec!["purge", "--all", "--lease-id", "invalid"],
        vec!["purge", "--lease-id", "invalid"],
    ] {
        let _ = parse(&args).unwrap().command.into_request().unwrap_err();
    }
    parse(&["purge", "--all"])
        .unwrap()
        .command
        .into_request()
        .unwrap();
}
#[tokio::test]
async fn malformed_and_oversized_wire_data_are_rejected() {
    let (mut writer, mut reader) = io::duplex(100);
    writer.write_u32(16385).await.unwrap();
    let _ = wire::read::<Request, _>(&mut reader).await.unwrap_err();
    let (mut writer, mut reader) = io::duplex(100);
    writer.write_u32(4097).await.unwrap();
    let _ = wire::read_secret(&mut reader).await.unwrap_err();
}

#[test]
fn maintenance_force_is_an_explicit_typed_stop_operation() {
    let request = parse(&["stop", "--force"])
        .unwrap()
        .command
        .into_request()
        .unwrap();
    assert!(matches!(request, AppRequest::StopForce));
}

#[test]
fn pairing_request_conversion_does_not_read_process_stdin() {
    let id = uuid::Uuid::new_v4().to_string();
    let error = parse(&[
        "phone-pair-confirm",
        "--attempt-id",
        &id,
        "--comparison-stdin",
    ])
    .unwrap()
    .command
    .into_request()
    .unwrap_err();
    assert_eq!(error.to_string(), "comparison input required");
}

#[test]
fn opaque_ids_keep_their_wire_spelling_and_invalid_inputs_fail_before_dispatch() {
    let id = uuid::Uuid::new_v4().to_string().to_uppercase();
    let AppRequest::PairingInspect(attempt_id) =
        parse(&["phone-pair-inspect", "--attempt-id", &id])
            .unwrap()
            .command
            .into_request()
            .unwrap()
    else {
        panic!("pairing inspection expected")
    };
    assert_eq!(attempt_id.as_str(), id);
    let AppRequest::Cleanup(lease_id) = parse(&["cleanup", "--lease-id", &id])
        .unwrap()
        .command
        .into_request()
        .unwrap()
    else {
        panic!("cleanup expected")
    };
    assert_eq!(lease_id.as_str(), id);
    for args in [
        ["phone-pair-inspect", "--attempt-id", "invalid"].as_slice(),
        ["phone-pair-cancel", "--attempt-id", "invalid"].as_slice(),
        [
            "phone-pair-confirm",
            "--attempt-id",
            "invalid",
            "--comparison-stdin",
        ]
        .as_slice(),
        ["cleanup", "--lease-id", "invalid"].as_slice(),
        ["serve", "--op-executable", ""].as_slice(),
    ] {
        let _ = parse(args)
            .unwrap()
            .command
            .into_request()
            .expect_err("invalid operation input");
    }
}

#[test]
fn phone_configuration_flags_build_requests_without_reading_state() {
    let java = env::temp_dir().join("synthetic-java.exe");
    let args = [
        "phone-configure",
        "--desktop-ip",
        "100.64.0.1",
        "--phone-ip",
        "100.64.0.2",
        "--desktop-node",
        "test-desktop",
        "--phone-node",
        "test-phone",
        "--tailnet-user",
        "42",
        "--phone-name",
        "phone.invalid.",
        "--java-executable",
        java.to_str().unwrap(),
    ];
    let parsed = parse(&args).unwrap();
    assert!(!format!("{:?}", parsed.command).contains("test-desktop"));
    assert!(matches!(
        parsed.command.into_request().unwrap(),
        AppRequest::PhoneConfig(_)
    ));
    for (position, invalid) in [
        (2, "127.0.0.1"),
        (4, "100.64.0.1"),
        (6, "bad node"),
        (10, "0"),
        (14, "relative-java"),
    ] {
        let mut invalid_args = args;
        invalid_args[position] = invalid;
        drop(
            parse(&invalid_args)
                .unwrap()
                .command
                .into_request()
                .unwrap_err(),
        );
    }
    for args in [
        vec!["phone-config-show"],
        vec!["phone-config-show", "--show-values"],
        vec!["phone-config-validate"],
    ] {
        assert!(matches!(
            parse(&args).unwrap().command.into_request().unwrap(),
            AppRequest::PhoneConfig(_)
        ));
    }
}
