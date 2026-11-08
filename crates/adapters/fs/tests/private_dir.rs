//! Tests for `lanstash_adapter_fs::private_dir`.

mod support;

use lanstash_adapter_fs::private_dir::PrivateDir;
use lanstash_ports::secrets::Secrets;
use support::TempDir;

#[test]
fn a_secret_is_kept_replaced_and_forgotten() {
    let t = TempDir::new("secrets");
    let s = PrivateDir::new(t.0.join("steam"));
    assert_eq!(s.load("refresh-token"), Ok(None));
    s.save("refresh-token", "one").unwrap();
    s.save("refresh-token", "two").unwrap();
    assert_eq!(s.load("refresh-token"), Ok(Some("two".into())));
    let names: Vec<_> = std::fs::read_dir(t.0.join("steam")).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, ["refresh-token"], "no temporary file left behind");
    s.remove("refresh-token").unwrap();
    s.remove("refresh-token").unwrap();
    assert_eq!(s.load("refresh-token"), Ok(None));
}

#[test]
fn saves_at_once_each_land_whole() {
    let t = TempDir::new("secrets-race");
    let s = std::sync::Arc::new(PrivateDir::new(t.0.join("epic")));
    let values: Vec<String> = (0..4).map(|i| format!("{i}").repeat(10_000)).collect();
    let threads: Vec<_> = values
        .iter()
        .cloned()
        .map(|v| {
            let s = std::sync::Arc::clone(&s);
            std::thread::spawn(move || (0..50).all(|_| s.save("token", &v).is_ok()))
        })
        .collect();
    assert!(threads.into_iter().all(|t| t.join().unwrap()), "every save succeeded");
    let kept = s.load("token").unwrap().unwrap();
    assert!(values.contains(&kept), "one whole value, not a mix");
}

#[test]
fn names_cannot_leave_the_directory() {
    let t = TempDir::new("secrets-names");
    let s = PrivateDir::new(&t.0);
    for bad in ["", "../x", "a/b", "Upper", ".hidden"] {
        assert!(s.save(bad, "v").is_err(), "{bad:?}");
        assert!(s.load(bad).is_err(), "{bad:?}");
    }
}

#[cfg(unix)]
#[test]
fn only_the_owner_can_read_them() {
    use std::os::unix::fs::PermissionsExt;
    let t = TempDir::new("secrets-mode");
    let dir = t.0.join("steam");
    PrivateDir::new(&dir).save("refresh-token", "x").unwrap();
    let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&dir.join("refresh-token")), 0o600);
}

#[test]
fn state_is_kept_as_bytes_alongside() {
    use lanstash_ports::keep::Keep;
    let t = TempDir::new("keep");
    let k = PrivateDir::new(t.0.join("state"));
    assert_eq!(Keep::load(&k, "steam-lancache"), Ok(None));
    Keep::save(&k, "steam-lancache", &[0, 1, 255]).unwrap();
    assert_eq!(Keep::load(&k, "steam-lancache"), Ok(Some(vec![0, 1, 255])));
    Keep::remove(&k, "steam-lancache").unwrap();
    assert_eq!(Keep::load(&k, "steam-lancache"), Ok(None), "forgotten");
    assert_eq!(Keep::remove(&k, "steam-lancache"), Ok(()), "forgetting it again is nothing");
}
