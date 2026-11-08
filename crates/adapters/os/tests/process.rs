//! Tests for `lanstash_adapter_os::process`: the soft open-file limit read from
//! a `/proc/<pid>/limits` text (as Linux writes it), and an error where there is
//! no `/proc`.

use lanstash_adapter_os::process::{Whose, max_open_files, open_files};

const LIMITS: &str = "Limit                     Soft Limit           Hard Limit           Units     \n\
Max cpu time              unlimited            unlimited            seconds   \n\
Max open files            65536                524288               files     \n\
Max locked memory         8388608              8388608              bytes     \n";

#[test]
fn the_soft_open_file_limit_is_read() {
    assert_eq!(max_open_files(LIMITS), Some(65_536));
    assert_eq!(max_open_files(&LIMITS.replace("65536 ", "unlimited")), Some(u64::MAX));
    assert_eq!(max_open_files("Max cpu time unlimited unlimited seconds\n"), None);
}

#[test]
fn a_process_that_is_not_there_is_an_error() {
    assert!(open_files(Whose::Pid(u32::MAX)).is_err());
    // This process: on Linux its own figures, elsewhere an error (no /proc).
    match open_files(Whose::This) {
        Ok((open, limit)) => assert!(open > 0 && limit >= open),
        Err(e) => assert!(e.contains("/proc/self")),
    }
}
