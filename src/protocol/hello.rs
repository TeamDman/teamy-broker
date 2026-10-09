use facet::Facet;
#[derive(Facet, Debug)]
pub struct Hello {
    pub version: u32,
    pub capabilities: Vec<String>,
    pub instance_id: String,
}
impl Hello {
    #[must_use]
    pub fn compatible(&self) -> bool {
        self.version == super::VERSION
            && super::CAPABILITIES
                .iter()
                .all(|cap| self.capabilities.iter().any(|c| c == cap))
    }
}
