//! Receipt-gated bounded authenticated bridge to actual resident no-vault preview; no acquisition.
use std::env;
use std::io;
use std::io::Write;
use std::panic;
use std::path::Path;
use std::process;
use std::sync::Arc;
use teamy_broker::broker::mobile_dummy::java_pairing_worker;
use teamy_broker::protocol::wire;
use tokio::net::TcpListener;
use tokio::runtime::Builder;
use tokio::time::Instant;
#[path = "mobile_preview_pairing/activation.rs"]
mod activation;
use activation::Arguments;
use mobile_dummy::preview_activation::PreviewActivation;
use mobile_dummy::preview_phase::PreviewMetadata;
use mobile_dummy::preview_phase::PreviewPhase;
use std::time::Duration;
use teamy_broker::broker::mobile_dummy;
use teamy_broker::paths::AppHome;
use teamy_broker::sdk::Client;
use teamy_broker_handoff::pairing_binding::PairingBinding;
use teamy_broker_handoff::session::Session;
fn main() {
    panic::set_hook(Box::new(|_| {}));
    if run().is_err() {
        process::exit(1);
    }
}
fn run() -> eyre::Result<()> {
    let args = Arguments::parse(&env::args_os().skip(1).collect::<Vec<_>>())?;
    let Arguments::Ready { receipt, phases } = args else {
        Write::write_all(&mut io::stdout().lock(),b"PROPOSAL ONLY: bounded resident no-vault preview bridge. Exact one-use action-time approval receipt, --activate-preview-pairing --phone-ready --approval-file PATH --phase-file PATH required. Exact endpoint/peer, 90s pairing / 180s total / 60s request, both Match actions. No listener started.\n")?;
        return Ok(());
    };
    let activation = PreviewActivation::consume(&receipt, &phases)?;
    let trace = activation.trace();
    let java = java_pairing_worker::configured_java()?;
    let artifacts = Path::new(env!("CARGO_MANIFEST_DIR")).join("pairing/java");
    let runtime = Builder::new_current_thread().enable_all().build()?;
    let result=runtime.block_on(async {
        let client=Client::new(None)?.with_deadlines(Duration::from_secs(2),Duration::from_secs(5));
        client.shared_preview_state().await?;
        let profile=mobile_dummy::phone_profile::PhoneProfile::load(&AppHome::resolve()?)?;
        mobile_dummy::tailnet_peer::TailnetPeer::verify_desktop_for(&profile).await?;
        activation.require_live()?;
        let listener=TcpListener::bind((profile.desktop.get(),mobile_dummy::tailnet_peer::PORT)).await?;
        trace.emit(PreviewPhase::ListenerBound,PreviewMetadata::default());
        let until=Instant::now()+Duration::from_mins(3);
        let mut binding=PairingBinding::dummy(wire::unix_ms()?)?;
        binding.desktop_node=profile.desktop_node.as_str().to_owned();binding.phone_node=profile.phone_node.as_str().to_owned();binding.endpoint=format!("{}:{}",profile.desktop.get(),mobile_dummy::tailnet_peer::PORT);
        let session=Session::random()?;
        Write::write_all(&mut io::stdout().lock(),b"Finite dummy pairing listener bound. Resident no-vault preview bridge only; no credential acquisition.\n")?;
        let paired=mobile_dummy::pairing_server::accept_preview(listener,&java,&artifacts,&binding,&session,until,&activation).await?;
        Write::write_all(&mut io::stdout().lock(),b"Authenticated preview bridge ready; no vault or lease.\n")?;
        paired.serve_preview_traced(session,client,Some(Arc::clone(&trace))).await
    });
    drop(runtime);
    if result.is_err() {
        trace.emit(PreviewPhase::ProtocolFailed, PreviewMetadata::default());
    }
    trace.emit(PreviewPhase::Cleanup, PreviewMetadata::default());
    result
}
