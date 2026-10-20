use facet::Facet;
#[derive(Facet, Debug)]
pub struct Packet {
    pub operation: String,
    pub envelope: teamy_broker_handoff::envelope::Envelope,
}
