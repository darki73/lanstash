//! The in-memory nginx cache holds slices as nginx names and writes them.

use lanstash_domain::nginx_cache::{cache_file_path, slice_key};
use lanstash_ports::nginx::NginxCacheSource;
use lanstash_store::test_support::nginx::MemNginx;

#[test]
fn slices_are_stored_by_key_and_counted_when_opened() {
    let n = MemNginx::default();
    n.whole("blizzard/tpr/a", &[7u8; 5], 2);
    let names: Vec<String> = (0..3).map(|i| cache_file_path(&slice_key("blizzard/tpr/a", i * 2, 2))).collect();
    let mut listed = n.list_files(&[]).unwrap();
    listed.sort();
    let mut want = names.clone();
    want.sort();
    assert_eq!(listed, want, "three slices of 2 bytes");
    let f = n.open(&names[2]).unwrap();
    assert!(f.size().unwrap() > 1, "header and the last byte");
    assert_eq!(*n.opened.lock().unwrap(), vec![names[2].clone()]);
    assert!(n.exists(&names[2]).unwrap());
    n.remove(&slice_key("blizzard/tpr/a", 0, 2));
    assert!(!n.exists(&names[0]).unwrap());
    assert!(n.open("nothing").is_err());
}
