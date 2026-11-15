//! One-socket loopback synthetic test only; no persistent identity or enrollment.
use std::env;
use std::io;
use std::io::Write;
use std::path::Path;
use std::process;
use std::time::Duration;
use teamy_broker::broker::mobile_dummy::java_pairing_worker;
use teamy_broker::broker::mobile_dummy::java_pairing_worker::JavaPairingWorker;
use teamy_broker::broker::phone::SCOPE;
use teamy_broker::protocol::wire;
use teamy_broker_handoff::pairing_binding::PairingBinding;
use teamy_broker_handoff::session::Session;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::time;
use tokio::time::Instant;
#[tokio::main]
async fn main() {
    if run(&mut io::stdout()).await.is_err() {
        tracing::error!(
            phase = "synthetic-test",
            "Synthetic transport test failed; no enrollment"
        );
        process::exit(1);
    }
}
async fn run(output: &mut impl Write) -> eyre::Result<()> {
    eyre::ensure!(
        matches!(
            env::args().nth(1).as_deref(),
            Some("--synthetic-loopback-delay-test" | "--synthetic-slow-startup-delay-test")
        ),
        "explicit synthetic mode required"
    );
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let slow_start = env::args().nth(1).as_deref() == Some("--synthetic-slow-startup-delay-test");
    let now = wire::unix_ms()?;
    let mut binding = PairingBinding::dummy(now)?;
    binding.scope = SCOPE.into();
    binding.pairing_expires_unix_ms = now + 300_000;
    binding.session_expires_unix_ms = now + 300_000;
    let until = Instant::now() + Duration::from_mins(5);
    let java = java_pairing_worker::configured_java()?;
    let artifacts = Path::new(env!("CARGO_MANIFEST_DIR")).join("pairing/java");
    let mut worker = JavaPairingWorker::start(&java, &artifacts, &binding, until).await?;
    output
        .write_all(format!("{{\"loopback_port\":{port},\"synthetic_only\":true}}\n").as_bytes())?;
    output.flush()?;
    let (mut socket, _) = time::timeout_at(until, listener.accept()).await??;
    drop(listener);
    if slow_start {
        time::sleep(Duration::from_secs(8)).await;
    }
    write_frame(&mut socket, binding.encode()?.as_bytes()).await?;
    let initial = read_frame(&mut socket).await?;
    worker.call(1, &initial).await?;
    let reply = worker.call(2, &[]).await?;
    write_frame(&mut socket, &reply).await?;
    let finish = read_frame(&mut socket).await?;
    worker.call(1, &finish).await?;
    let code = worker.authentication_code().await?;
    drop(code);
    let started = Instant::now();
    let mut byte = [0];
    eyre::ensure!(
        time::timeout_at(until, socket.peek(&mut byte)).await?? > 0,
        "synthetic peer closed"
    );
    let delayed = started.elapsed().as_millis();
    eyre::ensure!(delayed >= 120_000, "two minute delay missing");
    let desktop = worker.call(4, &[]).await?;
    write_frame(&mut socket, &desktop).await?;
    let phone = read_frame(&mut socket).await?;
    worker.call(5, &phone).await?;
    let session = Session::random()?;
    let bootstrap = worker.call(6, session.transport_key().as_ref()).await?;
    write_frame(&mut socket, &bootstrap).await?;
    let sealed = read_frame(&mut socket).await?;
    let ready = worker.call(7, &sealed).await?;
    eyre::ensure!(
        ready.as_slice() == b"teamy-broker/dummy/ready/v2",
        "synthetic ready rejected"
    );
    output.write_all(format!("{{\"socket_delay_ms\":{delayed},\"official_ukey2_confirmed\":true,\"enrollment\":false,\"credential_operations\":0}}\n").as_bytes())?;
    Ok(())
}

async fn read_frame(socket: &mut TcpStream) -> eyre::Result<zeroize::Zeroizing<Vec<u8>>> {
    time::timeout(Duration::from_secs(5), async {
        let n = socket.read_u32().await? as usize;
        eyre::ensure!(n <= 16384, "synthetic frame oversized");
        let mut data = zeroize::Zeroizing::new(vec![0; n]);
        socket.read_exact(&mut data).await?;
        Ok(data)
    })
    .await?
}
async fn write_frame(socket: &mut TcpStream, data: &[u8]) -> eyre::Result<()> {
    eyre::ensure!(data.len() <= 16384, "synthetic frame oversized");
    time::timeout(Duration::from_secs(5), async {
        socket.write_u32(u32::try_from(data.len())?).await?;
        socket.write_all(data).await?;
        Ok(())
    })
    .await?
}
