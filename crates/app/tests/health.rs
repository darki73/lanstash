//! Tests for `lanstash_app::health`: the open-file rule, telling a process's
//! open-file limit apart from damage when the store is loaded, and the JSON.

use lanstash_app::health::{
    Check, Level, background_check, free_space, is_open_file_limit, json, open_files, store_load,
};

#[test]
fn open_files_warn_near_the_limit_and_fail_at_it() {
    assert_eq!(open_files("the service", 3_703, 65_536).level, Level::Ok);
    assert_eq!(open_files("the service", 3_703, 65_536).detail, "the service: 3703 of 65536 (6 %)");
    assert_eq!(open_files("the service", 900, 1_024).level, Level::Warn);
    assert!(open_files("the service", 900, 1_024).detail.contains("LimitNOFILE"));
    assert_eq!(open_files("the service", 1_000, 1_024).level, Level::Fail);
    assert_eq!(open_files("the service", 1, 0).level, Level::Fail, "no limit to speak of");
}

#[test]
fn files_not_opened_for_the_limit_are_not_damage() {
    // A shell may open 1,024 files while the stash has thousands of segments: every one past
    // the limit fails to open.
    let refused: Vec<(String, String)> = (0..2_000)
        .map(|i| (format!("steam/{i}/seg/0.lcseg"), "seg: Too many open files (os error 24)".to_string()))
        .collect();
    let c = store_load(300, 1_500_000_000_000, &refused, 0, Some(1_024));
    assert_eq!(c.level, Level::Warn);
    assert!(!c.detail.contains("problems"), "{}", c.detail);
    assert!(c.detail.contains("2000 files not opened: too many open files for this process (it may open 1024)"));
    assert!(c.detail.contains("ulimit -n 65536"));
    let damaged = [("steam/1/seg/0.lcseg".to_string(), "bad header".to_string())];
    let c = store_load(1, 5, &damaged, 2, None);
    assert!(c.detail.contains("3 problems") && !c.detail.contains("not opened"), "{}", c.detail);
    assert_eq!(store_load(1, 5, &[], 0, None).level, Level::Ok);
    assert!(is_open_file_limit("Too many open files") && !is_open_file_limit("No such file"));
}

#[test]
fn free_space_against_keep_free() {
    use lanstash_domain::evict::KeepFree;
    let c = free_space(8_219_000_000_000, 12_050_000_000_000, Some(KeepFree::Percent(30)));
    assert_eq!((c.level, c.detail.as_str()), (Level::Ok, "8219 GB free of 12050 GB (keep_free 30%)"));
    assert_eq!(free_space(1_000, 10_000, Some(KeepFree::Percent(30))).level, Level::Warn);
    assert!(free_space(1, 10, None).detail.ends_with("(keep_free off)"));
}

#[test]
fn what_the_background_check_found() {
    assert_eq!(background_check(0, 0, false).detail, "no round yet");
    assert_eq!(background_check(0, 0, true).detail, "its first round is running");
    assert_eq!(background_check(0, 5, false).level, Level::Ok);
    let c = background_check(3, 5, false);
    assert_eq!((c.level, c.detail.as_str()), (Level::Warn, "3 problems (`lanstash store verify` for details)"));
}

#[test]
fn checks_as_json() {
    let checks = [Check::new(Level::Ok, "store", "writable"), Check::new(Level::Warn, "open \"files\"", "x")];
    assert_eq!(
        json(&checks),
        "{\"checks\":[{\"level\":\"ok\",\"what\":\"store\",\"detail\":\"writable\"},\
         {\"level\":\"warn\",\"what\":\"open \\\"files\\\"\",\"detail\":\"x\"}]}\n"
    );
}
