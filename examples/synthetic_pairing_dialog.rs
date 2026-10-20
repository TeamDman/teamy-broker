//! No listener or live comparison code. Refuses to open UI on the input desktop.
fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    if run().is_err() {
        std::process::exit(1);
    }
}
fn run() -> eyre::Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        std::io::Write::write_all(&mut std::io::stdout().lock(),b"PROPOSAL ONLY: exact private-desktop synthetic pairing UI probe; no live code, network or authentication.\n")?;
        return Ok(());
    }
    eyre::ensure!(
        args.len() == 2 || args.len() == 3,
        "private probe scope required"
    );
    let repaired = match args[1].as_str() {
        "repaired" => true,
        "baseline" => false,
        _ => eyre::bail!("private probe mode required"),
    };
    let report = if args.len() == 3 {
        let accept = match args[2].as_str() {
            "cli-confirm" => true,
            "cli-cancel" => false,
            _ => eyre::bail!("synthetic resolution mode required"),
        };
        teamy_broker::broker::mobile_dummy::pairing_prompt_probe::run_confirmation(
            &args[0], accept,
        )?
    } else {
        teamy_broker::broker::mobile_dummy::pairing_prompt_probe::run(&args[0], repaired)?
    };
    let json = facet_json::to_string(&report)?;
    std::io::Write::write_all(&mut std::io::stdout().lock(), json.as_bytes())?;
    Ok(())
}
