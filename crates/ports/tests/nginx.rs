use std::sync::Arc;

use lanstash_ports::nginx::NginxCacheSource;
use lanstash_ports::storage::{FileRead, StoreError, StoreErrorKind};

struct Empty;

impl FileRead for Empty {
    fn size(&self) -> Result<u64, StoreError> {
        Ok(0)
    }
    fn read_exact_at(&self, _offset: u64, _buf: &mut [u8]) -> Result<(), StoreError> {
        Ok(())
    }
}

/// Holds `ab/cd/x`; `bad/...` fails to open.
struct Cache;

impl NginxCacheSource for Cache {
    fn list_files(&self, _top_dirs: &[String]) -> Result<Vec<String>, StoreError> {
        Ok(vec!["ab/cd/x".into()])
    }
    fn open(&self, rel: &str) -> Result<Arc<dyn FileRead>, StoreError> {
        match rel {
            "ab/cd/x" => Ok(Arc::new(Empty)),
            r if r.starts_with("bad/") => Err(StoreError::other("unreadable")),
            _ => Err(StoreError::new(StoreErrorKind::NotFound, rel)),
        }
    }
}

#[test]
fn exists_opens_by_default() {
    assert_eq!(Cache.list_files(&["ab".into()]).unwrap(), vec!["ab/cd/x"]);
    assert_eq!(Cache.exists("ab/cd/x"), Ok(true));
    assert_eq!(Cache.exists("ab/cd/y"), Ok(false));
    assert_eq!(Cache.exists("bad/x").unwrap_err().kind, StoreErrorKind::Other);
}
