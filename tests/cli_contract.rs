use teamy_broker::broker::app_request::AppRequest;
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
    for name in ["status", "stop"] {
        let AppRequest::Control(request) = parse(&[name]).unwrap().command.into_request().unwrap()
        else {
            panic!("control expected")
        };
        assert_eq!(request.operation, name);
        request.validate().unwrap();
    }
    assert!(matches!(
        parse(&["serve"]).unwrap().command.into_request().unwrap(),
        AppRequest::Serve(_)
    ));
    let id = uuid::Uuid::new_v4().to_string();
    let AppRequest::Control(request) = parse(&["purge", "--lease-id", &id])
        .unwrap()
        .command
        .into_request()
        .unwrap()
    else {
        panic!("control expected")
    };
    assert_eq!(request.lease_id, Some(id));
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
    let AppRequest::Control(request) = parse(&["stop", "--force"])
        .unwrap()
        .command
        .into_request()
        .unwrap()
    else {
        panic!("control expected")
    };
    assert_eq!(request.operation, "stop-force");
    request.validate().unwrap();
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
