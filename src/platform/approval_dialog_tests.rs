use super::disposition;
use crate::protocol::ApprovalPreviewDisposition;
#[test]
fn cancelled_or_expired_test_cannot_report_approval() {
    assert_eq!(
        disposition(true, false, 0, 100),
        ApprovalPreviewDisposition::Cancelled
    );
    assert_eq!(
        disposition(false, true, 0, 100),
        ApprovalPreviewDisposition::TimedOut
    );
    assert_eq!(
        disposition(true, true, 0, 100),
        ApprovalPreviewDisposition::Cancelled
    );
}
#[test]
fn close_and_native_failure_are_distinct_from_test_deny() {
    assert_eq!(
        disposition(false, false, 0, 100),
        ApprovalPreviewDisposition::ApprovedTest
    );
    assert_eq!(
        disposition(false, false, 0, 101),
        ApprovalPreviewDisposition::DeniedTest
    );
    assert_eq!(
        disposition(false, false, 0, 2),
        ApprovalPreviewDisposition::Closed
    );
    assert_eq!(
        disposition(false, false, -1, 100),
        ApprovalPreviewDisposition::Unavailable
    );
}
