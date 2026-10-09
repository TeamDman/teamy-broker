use facet::Facet;

/// Safe bootstrap diagnostic: no argument, path, credential or raw error payload.
#[derive(Debug, Facet)]
pub struct FailureRecord<'a> {
    pub level: &'a str,
    pub event_kind: &'a str,
    pub message: &'a str,
}
