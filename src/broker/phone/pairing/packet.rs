use facet::Facet;
use teamy_broker_handoff::envelope::Envelope;
#[derive(Facet, Debug)]
pub struct Packet {
    pub operation: String,
    pub envelope: Envelope,
}
