//! Runs CLI logging and output against one owned status-only pipe, with no resident broker access.
use std::env;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::process::ExitCode;
use std::time::Duration;
use teamy_broker::broker::failure_message;
use teamy_broker::platform::pipe_security;
use teamy_broker::protocol::Hello;
use teamy_broker::protocol::Request;
use teamy_broker::protocol::Response;
use teamy_broker::protocol::Status;
use teamy_broker::protocol::{self};
use teamy_broker::sdk::Client;
use tokio::net::windows::named_pipe::NamedPipeServer;
use tokio::runtime::Builder;
use tokio::time::timeout;

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(_redacted_error) => {
            tracing::error!("Synthetic logging CLI fixture failed");
            ExitCode::FAILURE
        }
    }
}

fn run() -> eyre::Result<ExitCode> {
    require_status_arguments(env::args_os().skip(1).collect())?;
    let runtime = Builder::new_multi_thread().enable_all().build()?;
    let endpoint = format!(
        r"\\.\pipe\teamy-broker-test-logging-{}",
        uuid::Uuid::new_v4()
    );
    let client = Client::new(None)?.with_endpoint(endpoint.clone());
    let pipe = {
        let _runtime = runtime.enter();
        pipe_security::create(&endpoint, true)?
    };
    let server =
        runtime.spawn(async move { timeout(Duration::from_secs(15), serve_status(pipe)).await? });
    let result = teamy_broker::entrypoint_with_client(client, failure_message::classify);
    if result != ExitCode::SUCCESS {
        server.abort();
        let _ = runtime.block_on(server);
        return Ok(result);
    }
    // Success requires this pipe to receive the command, not merely a successful CLI exit.
    runtime.block_on(server)??;
    Ok(result)
}

fn require_status_arguments(mut args: Vec<OsString>) -> eyre::Result<()> {
    eyre::ensure!(
        args.pop().as_deref() == Some(OsStr::new("status")),
        "synthetic logging helper requires status"
    );
    // Limit this fixture to logging options plus status. In particular it cannot invoke serve,
    // cleanup, or any other local operation that does not use the supplied client.
    eyre::ensure!(
        args.len().is_multiple_of(2),
        "logging option pairs required"
    );
    for pair in args.chunks_exact(2) {
        eyre::ensure!(
            matches!(
                pair[0].to_str(),
                Some(
                    "--log-format"
                        | "--output-format"
                        | "--log-file"
                        | "--log-filter"
                        | "--log-file-format"
                        | "--log-file-filter"
                )
            ),
            "unsupported synthetic logging option"
        );
    }
    Ok(())
}

async fn serve_status(mut pipe: NamedPipeServer) -> eyre::Result<()> {
    pipe.connect().await?;
    let hello = Hello {
        version: protocol::VERSION,
        capabilities: protocol::CAPABILITIES
            .iter()
            .map(|capability| (*capability).to_owned())
            .collect(),
        instance_id: uuid::Uuid::new_v4().to_string(),
    };
    protocol::wire::write(&mut pipe, &hello).await?;
    let peer: Hello = protocol::wire::read(&mut pipe).await?;
    eyre::ensure!(peer.compatible(), "incompatible synthetic client");
    let request: Request = protocol::wire::read(&mut pipe).await?;
    request.validate()?;
    eyre::ensure!(request.operation == "status", "status-only fixture");
    let response = Response {
        code: "ok".into(),
        status: Some(Status {
            running: true,
            ..Status::stopped()
        }),
        ..Response::default()
    };
    protocol::wire::write(&mut pipe, &response).await
}
