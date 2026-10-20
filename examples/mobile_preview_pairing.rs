//! Receipt-gated bounded authenticated bridge to actual resident no-vault preview; no acquisition.
#[path = "mobile_preview_pairing/activation.rs"]
mod activation;
use activation::Arguments;
use mobile_dummy::preview_activation::PreviewActivation;
use mobile_dummy::preview_phase::PreviewMetadata;
use mobile_dummy::preview_phase::PreviewPhase;
use std::time::Duration;
use teamy_broker::broker::mobile_dummy;
use teamy_broker::sdk::Client;
use teamy_broker_handoff::pairing_binding::PairingBinding;
use teamy_broker_handoff::session::Session;
fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    if run().is_err() {
        std::process::exit(1);
    }
}
fn run() -> eyre::Result<()> {
    let args = Arguments::parse(&std::env::args_os().skip(1).collect::<Vec<_>>())?;
    let Arguments::Ready { receipt, phases } = args else {
        std::io::Write::write_all(&mut std::io::stdout().lock(),b"PROPOSAL ONLY: bounded resident no-vault preview bridge. Exact one-use action-time approval receipt, --activate-preview-pairing --phone-ready --approval-file PATH --phase-file PATH required. Exact endpoint/peer, 90s pairing / 180s total / 60s request, both Match actions. No listener started.\n")?;
        return Ok(());
    };
    let activation = PreviewActivation::consume(&receipt, &phases)?;
    let trace = activation.trace();
    let java = teamy_broker::broker::mobile_dummy::java_pairing_worker::configured_java()?;
    let artifacts = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("pairing/java");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let result=runtime.block_on(async {
        let client=Client::new(None)?.with_deadlines(Duration::from_secs(2),Duration::from_secs(5));
        client.shared_preview_state().await?;
        mobile_dummy::tailnet_peer::TailnetPeer::verify_desktop().await?;
        activation.require_live()?;
        let listener=tokio::net::TcpListener::bind((mobile_dummy::tailnet_peer::DESKTOP,mobile_dummy::tailnet_peer::PORT)).await?;
        trace.emit(PreviewPhase::ListenerBound,PreviewMetadata::default());
        let until=tokio::time::Instant::now()+Duration::from_mins(3);
        let binding=PairingBinding::dummy(teamy_broker::protocol::wire::unix_ms()?)?;
        let session=Session::random()?;
        std::io::Write::write_all(&mut std::io::stdout().lock(),b"Finite dummy pairing listener bound. Resident no-vault preview bridge only; no credential acquisition.\n")?;
        let paired=mobile_dummy::pairing_server::accept_preview(listener,&java,&artifacts,&binding,&session,until,&activation).await?;
        std::io::Write::write_all(&mut std::io::stdout().lock(),b"Authenticated preview bridge ready; no vault or lease.\n")?;
        paired.serve_preview_traced(session,client,Some(std::sync::Arc::clone(&trace))).await
    });
    drop(runtime);
    if result.is_err() {
        trace.emit(PreviewPhase::ProtocolFailed, PreviewMetadata::default());
    }
    trace.emit(PreviewPhase::Cleanup, PreviewMetadata::default());
    result
}
