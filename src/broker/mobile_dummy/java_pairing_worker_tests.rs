use super::java_pairing_worker::JavaPairingWorker;
use crate::protocol::wire;
use std::path::Path;
use std::str;
use std::time::Duration;
use teamy_broker_handoff::pairing_binding::PairingBinding;
use teamy_broker_handoff::queue::Queue;
use teamy_broker_handoff::session::Session;
use teamy_broker_handoff::wire as teamy_broker_handoff_wire;
use tokio::time::Instant;
use zeroize::Zeroizing;
#[tokio::test]
async fn rust_private_pipe_java_phone_roundtrip_bound_dummy_queue_and_restart() -> eyre::Result<()>
{
    let java = super::JavaExecutablePath::for_tests()?.into_path_buf();
    let artifacts = Path::new(env!("CARGO_MANIFEST_DIR")).join("pairing/java");
    let binding = PairingBinding::dummy(wire::unix_ms()?)?;
    let until = Instant::now() + Duration::from_secs(20);
    let mut desktop = JavaPairingWorker::start(&java, &artifacts, &binding, until).await?;
    let mut phone = JavaPairingWorker::start_test_phone(&java, &artifacts, &binding, until).await?;
    let initial = phone.call(2, &[]).await?;
    desktop.call(1, &initial).await?;
    let reply = desktop.call(2, &[]).await?;
    phone.call(1, &reply).await?;
    let finish = phone.call(2, &[]).await?;
    desktop.call(1, &finish).await?;
    let a = phone.authentication_code().await?;
    let b = desktop.authentication_code().await?;
    eyre::ensure!(a.as_str() == b.as_str(), "test comparison mismatch");
    drop(a);
    drop(b);
    let pm = phone.call(4, &[]).await?;
    let dm = desktop.call(4, &[]).await?;
    eyre::ensure!(
        desktop.call(6, b"TEAMY-DUMMY-ONLY").await.is_err(),
        "unconfirmed dummy traffic accepted"
    ); // Fails closed and terminates this worker.
    // Start a fresh independent pair: no restart state or shared authorization survives.
    drop(desktop);
    drop(phone);
    let mut desktop = JavaPairingWorker::start(&java, &artifacts, &binding, until).await?;
    let mut phone = JavaPairingWorker::start_test_phone(&java, &artifacts, &binding, until).await?;
    let initial = phone.call(2, &[]).await?;
    desktop.call(1, &initial).await?;
    let reply = desktop.call(2, &[]).await?;
    phone.call(1, &reply).await?;
    let finish = phone.call(2, &[]).await?;
    desktop.call(1, &finish).await?;
    let a = phone.authentication_code().await?;
    let b = desktop.authentication_code().await?;
    eyre::ensure!(a.as_str() == b.as_str(), "test comparison mismatch");
    drop(a);
    drop(b);
    let pm2 = phone.call(4, &[]).await?;
    let dm2 = desktop.call(4, &[]).await?;
    // Previous live transcript confirmation is not valid after restart, even with identical metadata.
    let mut restart = JavaPairingWorker::start(&java, &artifacts, &binding, until).await?;
    eyre::ensure!(
        restart.call(5, &pm).await.is_err(),
        "restart confirmation accepted"
    );
    drop(restart);
    drop(pm);
    drop(dm);
    phone.call(5, &dm2).await?;
    desktop.call(5, &pm2).await?;
    let session = Session::random()?;
    let key = session.bootstrap();
    let ciphertext = desktop.call(6, key.as_bytes()).await?;
    let received = phone.call(7, &ciphertext).await?;
    let received = str::from_utf8(&received)?;
    let client = Session::from_bootstrap(received)?;
    let queue = Queue::new(session);
    let challenge = teamy_broker_handoff_wire::random::<32>()?;
    let poll = client.seal(teamy_broker_handoff::POLL_AAD, &challenge)?;
    let packet = super::packet::Packet {
        operation: "poll".into(),
        envelope: poll,
    };
    let packet = Zeroizing::new(facet_json::to_string(&packet)?);
    let ciphertext = phone.call(6, packet.as_bytes()).await?;
    let decoded = desktop.call(7, &ciphertext).await?;
    let packet: super::packet::Packet = facet_json::from_str(str::from_utf8(&decoded)?)?;
    let response = queue.poll(&packet.envelope)?;
    let response = Zeroizing::new(facet_json::to_string(&response)?);
    let ciphertext = desktop.call(6, response.as_bytes()).await?;
    let decoded = phone.call(7, &ciphertext).await?;
    let envelope = facet_json::from_str(str::from_utf8(&decoded)?)?;
    let plain = client.open(&challenge, &envelope)?;
    eyre::ensure!(plain.as_slice() == b"[]", "test poll mismatch");
    // Replay fails closed at the established library's directional sequence/MAC layer.
    eyre::ensure!(
        phone.call(7, &ciphertext).await.is_err(),
        "replayed ciphertext accepted"
    );
    Ok(())
}
