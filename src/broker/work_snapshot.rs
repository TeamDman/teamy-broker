#[derive(Clone, Debug, Default)]
pub struct WorkSnapshot {
    pub counts: [u64; 6],
    pub stopping: bool,
}
