//! Tests for `lanstash_ports::passwords`: the trait works as an object.

use lanstash_ports::passwords::Passwords;

/// Keeps a password as itself with a mark: enough to show the contract.
struct Plain;

impl Passwords for Plain {
    fn hash(&self, password: &str) -> Result<String, String> {
        Ok(format!("plain${password}"))
    }
    fn verify(&self, password: &str, kept: &str) -> bool {
        kept.strip_prefix("plain$") == Some(password)
    }
}

#[test]
fn a_password_is_kept_as_a_hash_and_checked_against_it() {
    let p: &dyn Passwords = &Plain;
    let kept = p.hash("correct horse").unwrap();
    assert!(p.verify("correct horse", &kept));
    assert!(!p.verify("wrong", &kept));
    assert!(!p.verify("correct horse", "not a hash"));
}
