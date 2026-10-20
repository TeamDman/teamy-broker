#[derive(Clone, Copy, Debug)]
#[repr(usize)]
pub enum WorkKind {
    Request,
    Queued,
    Approval,
    Dialog,
    Upstream,
    Storage,
}
