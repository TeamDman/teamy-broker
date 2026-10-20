use std::ffi::OsString;
use std::path::PathBuf;
pub(super) enum Arguments {
    Proposal,
    Ready { receipt: PathBuf, phases: PathBuf },
}
impl std::fmt::Debug for Arguments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PreviewArguments([redacted])")
    }
}
impl Arguments {
    pub fn parse(args: &[OsString]) -> eyre::Result<Self> {
        if args.is_empty() {
            return Ok(Self::Proposal);
        }
        eyre::ensure!(
            args.len() == 6
                && args[0] == "--activate-preview-pairing"
                && args[1] == "--phone-ready"
                && args[2] == "--approval-file"
                && args[4] == "--phase-file"
                && !args[3].is_empty()
                && !args[5].is_empty(),
            "explicit preview approval receipt, phone readiness and phase file required"
        );
        Ok(Self::Ready {
            receipt: PathBuf::from(&args[3]),
            phases: PathBuf::from(&args[5]),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_is_proposal_and_partial_or_legacy_activation_fails() {
        assert!(matches!(
            Arguments::parse(&[]).unwrap(),
            Arguments::Proposal
        ));
        for args in [
            vec!["--activate-preview-pairing"],
            vec!["--activate-preview-pairing", "--phone-ready"],
            vec!["--activate-dummy-pairing", "--phone-ready"],
        ] {
            let args: Vec<_> = args.into_iter().map(OsString::from).collect();
            assert!(Arguments::parse(&args).is_err());
        }
        let args: Vec<_> = [
            "--activate-preview-pairing",
            "--phone-ready",
            "--approval-file",
            "approval.json",
            "--phase-file",
            "phases.jsonl",
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        assert!(matches!(
            Arguments::parse(&args).unwrap(),
            Arguments::Ready { .. }
        ));
    }
}
