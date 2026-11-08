//! Why an append failed, in words, and from the errors below it.

use lanstash_domain::segment::error::FormatError;
use lanstash_ports::storage::{StoreError, StoreErrorKind};
use lanstash_store::segment::append_error::AppendError;

#[test]
fn every_kind_says_what_went_wrong() {
    assert_eq!(AppendError::Full.to_string(), "segment full");
    assert_eq!(AppendError::Sealed.to_string(), "segment already sealed");
    assert_eq!(AppendError::Busy { in_flight: 2 }.to_string(), "2 appends still in flight");
    assert_eq!(AppendError::Mismatch("key").to_string(), "record does not match its reservation: key");
    let store = AppendError::from(StoreError::new(StoreErrorKind::NotFound, "x"));
    assert!(matches!(&store, AppendError::Store(e) if e.kind == StoreErrorKind::NotFound));
    assert!(store.to_string().starts_with("storage: "));
    let format = AppendError::from(FormatError::UnsupportedVersion(9));
    assert!(matches!(format, AppendError::Format(FormatError::UnsupportedVersion(9))));
}
