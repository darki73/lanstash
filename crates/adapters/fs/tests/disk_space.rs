//! Tests for `lanstash_adapter_fs::disk_space`.

use lanstash_adapter_fs::disk_space::*;
use lanstash_domain::evict::Disk;

#[test]
fn parses_df_output() {
    let out = "        Avail          1B-blocks\n7306870784000 12049753047040\n";
    assert_eq!(parse_df(out), Some(Disk { free: 7_306_870_784_000, total: 12_049_753_047_040 }));
    assert_eq!(parse_df("Avail 1B-blocks\n"), None);
    assert_eq!(parse_df("Avail 1B-blocks\n12 x\n"), None);
}

/// The real `df` (Linux, where lanstash runs).
#[cfg(target_os = "linux")]
#[test]
fn asks_df_about_a_real_directory() {
    let d = df_space(&std::env::temp_dir(), std::time::Duration::from_secs(10)).unwrap();
    assert!(d.total > 0 && d.free <= d.total, "{d:?}");
}
