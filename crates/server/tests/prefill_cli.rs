//! `lanstash prefill`: the path that starts it and the progress lines.

use lanstash::prefill_cli::{progress, start_path};
use lanstash_domain::json;

#[test]
fn start_paths() {
    assert_eq!(start_path("blizzard", "ovw", false, false), "/api/v1/prefill/blizzard/ovw");
    assert_eq!(start_path("blizzard", "ovw", true, true), "/api/v1/prefill/blizzard/ovw?build=pre&from=lancache");
    assert_eq!(start_path("blizzard", "ovw", false, true), "/api/v1/prefill/blizzard/ovw?from=lancache");
}

#[test]
fn progress_lines() {
    let running = json::parse(
        r#"{"game":"ovw","version":"2.25","prerelease":false,"from":"lancache","state":"running","files_total":10,
        "files_done":4,"files_skipped":1,"files_missing":0,"files_local":2,"bytes_local":3000000000,
        "files_rejected":1,"bytes_total":8000000000,"bytes":4000000000,"current":null,"error":null}"#,
    )
    .unwrap();
    let p = progress(&running).unwrap();
    assert!(p.running());
    assert_eq!(
        p.line,
        "ovw 2.25: running, 4/10 requests, 4.00 GB of 8.00 GB, from lancache 3.00 GB (2 requests, 1 rejected), 1 already stashed"
    );
    // Bytes count only what the stash lacked: 36.7 MB of 91.31 GB, the rest stashed.
    let held = json::parse(
        r#"{"game":"wowf","version":"1","state":"done","files_total":50920,"files_done":50920,"files_skipped":50900,
        "bytes_skipped":91273300000,"files_missing":0,"bytes_total":91310000000,"bytes":36700000}"#,
    )
    .unwrap();
    assert_eq!(
        progress(&held).unwrap().line,
        "wowf 1: done, 50920/50920 requests, 0.04 GB of 0.04 GB, 50900 already stashed (91.27 GB)"
    );
    let failed = json::parse(r#"{"game":"ovw","state":"failed","error":"no plans"}"#).unwrap();
    let f = progress(&failed).unwrap();
    assert!(!f.running());
    assert_eq!(f.line, "ovw ?: failed, 0/0 requests, 0.00 GB of 0.00 GB: no plans");
    assert_eq!(progress(&json::parse("null").unwrap()), None);
}
