#[derive(Debug)]
pub(crate) struct ApprovalDialogText {
    pub title: Vec<u16>,
    pub instruction: Vec<u16>,
    pub details: Vec<u16>,
    pub yes: Vec<u16>,
    pub no: Vec<u16>,
}
