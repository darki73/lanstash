//! `run` against a program every test machine has: the cargo running the tests.

use std::time::Duration;

use lanstash_adapter_os::command::run;

fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}

#[test]
fn runs_a_program_and_reads_its_output() {
    let out = run(&cargo(), &["--version"], Duration::from_secs(60)).unwrap();
    assert!(out.success, "{out:?}");
    assert_eq!(out.code, Some(0));
    assert!(out.stdout.starts_with("cargo "), "{:?}", out.stdout);
}

#[test]
fn a_failing_program_is_not_an_error() {
    let out = run(&cargo(), &["--no-such-flag-for-lanstash"], Duration::from_secs(60)).unwrap();
    assert!(!out.success);
    assert_ne!(out.code, Some(0));
}

#[test]
fn a_missing_program_is_an_error() {
    let e = run("lanstash-no-such-program", &[], Duration::from_secs(5)).unwrap_err();
    assert!(e.starts_with("lanstash-no-such-program: "), "{e}");
}
