//! Tests for `lanstash_adapter_crypto::passwords`: PBKDF2 hashes made, checked and refused.

use lanstash_adapter_crypto::passwords::Pbkdf2;
use lanstash_ports::passwords::Passwords;

#[test]
fn a_hash_checks_its_password_only() {
    let p = Pbkdf2::new(10);
    let kept = p.hash("correct horse battery").unwrap();
    assert!(kept.starts_with("pbkdf2-sha256$10$"), "{kept}");
    assert_eq!(kept.split('$').map(str::len).collect::<Vec<_>>(), [13, 2, 32, 64]);
    assert!(p.verify("correct horse battery", &kept));
    assert!(!p.verify("correct horse batterY", &kept));
    assert!(!p.verify("", &kept));
}

#[test]
fn the_same_password_never_hashes_the_same() {
    let p = Pbkdf2::new(2);
    assert_ne!(p.hash("same password").unwrap(), p.hash("same password").unwrap());
}

#[test]
fn a_hash_says_its_own_iterations() {
    let kept = Pbkdf2::new(3).hash("pass phrase").unwrap();
    assert!(Pbkdf2::new(50).verify("pass phrase", &kept), "checked with the 3 it was made with");
}

#[test]
fn what_is_no_hash_of_ours_never_checks() {
    let p = Pbkdf2::new(2);
    let good = p.hash("pw-1234567").unwrap();
    let swap = |i: usize, with: &str| {
        let mut v: Vec<&str> = good.split('$').collect();
        v[i] = with;
        v.join("$")
    };
    for bad in [
        String::new(),
        "plain".into(),
        swap(0, "md5"),
        swap(1, "0"),
        swap(1, "99999999999"),
        swap(2, "zz"),
        swap(3, "abcd"),
        format!("{good}$extra"),
    ] {
        assert!(!p.verify("pw-1234567", &bad), "{bad}");
    }
}
