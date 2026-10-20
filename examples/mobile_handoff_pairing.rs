//! Revised session-only matching-code demo. Production serve never registers it.
#[path = "mobile_handoff_pairing/activation_arguments.rs"]
mod activation_arguments;
use activation_arguments::PairingActivation;
use std::sync::Arc;
use std::time::Duration;
use teamy_broker::broker::engine::Engine;
use teamy_broker::broker::mobile_dummy;
use teamy_broker::broker::mobile_dummy::adapter::MobileDummyAdapter;
use teamy_broker::broker::mobile_dummy::broadcast_approval::BroadcastDummyApproval;
use teamy_broker::broker::mobile_dummy::storage::DummyMemoryStorage;
use teamy_broker::sdk::Client;
use teamy_broker_handoff::pairing_binding::PairingBinding;
use teamy_broker_handoff::queue::Queue;
use teamy_broker_handoff::session::Session;
fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    if run().is_err() {
        std::process::exit(1);
    }
}
fn run() -> eyre::Result<()> {
    let activation = PairingActivation::parse(&std::env::args_os().skip(1).collect::<Vec<_>>())?;
    if activation == PairingActivation::Proposal {
        std::io::Write::write_all(&mut std::io::stdout().lock(),b"PROPOSAL ONLY: one UKEY2 P256 dummy test against the configured exact phone, one connection attempt, 90s pairing / 180s total. Both Match actions required. Explicit --activate-dummy-pairing --phone-ready required after current phone readiness. No listener started.\n")?;
        return Ok(());
    }
    eyre::ensure!(
        mobile_dummy::pairing_server::PAIRING_ACTIVATION_APPROVED,
        "pairing activation blocked"
    );
    let java = teamy_broker::broker::mobile_dummy::java_pairing_worker::configured_java()?;
    let artifacts = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("pairing/java");
    eyre::ensure!(
        java.is_file() && artifacts.join("build/ukey2-worker.jar").is_file(),
        "pairing runtime unavailable"
    );
    tokio::runtime::Builder::new_current_thread().enable_all().build()?.block_on(async{
        mobile_dummy::tailnet_peer::TailnetPeer::verify_desktop().await?;
        let listener=tokio::net::TcpListener::bind((mobile_dummy::tailnet_peer::DESKTOP,mobile_dummy::tailnet_peer::PORT)).await?;
        let until=tokio::time::Instant::now()+Duration::from_mins(3);
        let binding=PairingBinding::dummy(teamy_broker::protocol::wire::unix_ms()?)?;
        let session=Session::random()?;
        std::io::Write::write_all(&mut std::io::stdout().lock(),b"Finite dummy pairing listener bound. No credential request queued before both Match actions and authenticated phone readiness.\n")?;
        let paired=mobile_dummy::pairing_server::accept(listener,&java,&artifacts,&binding,&session,until).await?;
        let queue=Arc::new(Queue::new(session));
        let engine=Arc::new(Engine::new(Arc::new(MobileDummyAdapter),Arc::new(BroadcastDummyApproval{queue:Arc::clone(&queue)}),Arc::new(DummyMemoryStorage)));
        let mut tcp=tokio::spawn(paired.serve(queue));
        let mut tcp_finished=false;
        let endpoint=format!(r"\\.\pipe\teamy-broker-mobile-dummy-{}",uuid::Uuid::new_v4());
        let ipc_endpoint=endpoint.clone();
        let ipc=tokio::spawn(async move{teamy_broker::broker::server::serve_at(engine,teamy_cancellation::CancellationToken::new(),&ipc_endpoint,false).await});
        let client=Client::new(None)?.with_endpoint(endpoint).with_deadlines(Duration::from_secs(2),Duration::from_secs(65));
        let result=tokio::time::timeout_at(until,async{
            tokio::time::timeout(Duration::from_secs(3),async{
                while !client.status().await?.running{tokio::task::yield_now().await;}
                Ok::<_,eyre::Report>(())
            }).await.map_err(|_redacted_error|eyre::eyre!("dummy endpoint unavailable"))??;
            tokio::select!{
                _=&mut tcp=>{tcp_finished = true; eyre::bail!("phone session unavailable")},
                result=client.acquire(mobile_dummy::request())=>{
                    let secret=result?;
                    eyre::ensure!(secret.expose(|bytes|bytes==teamy_broker_handoff::DUMMY_SECRET)?,"dummy grant required");
                    client.purge(Some(secret.lease_id.clone())).await?;drop(secret);
                    std::io::Write::write_all(&mut std::io::stdout().lock(),b"Dummy SDK handoff completed; memory lease purged.\n")?;
                    Ok::<_,eyre::Report>(())
                }
            }
        }).await.map_err(|_redacted_error|eyre::eyre!("dummy session expired"));
        if !tcp_finished {tcp.abort();let _=tcp.await;}
        if client.stop_force().await.is_err(){ipc.abort();}else{let _=ipc.await;}
        std::io::Write::write_all(&mut std::io::stdout().lock(),b"Dummy pairing session stopped; no persistent listener or pairing.\n")?;
        result?
    })
}
