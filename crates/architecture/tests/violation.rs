use lanstash_architecture::violation::Violation;

#[test]
fn shows_package_rule_and_detail() {
    let v = Violation::new("lanstash-app", "no-io", "src/x.rs uses std::fs");
    assert_eq!(v.to_string(), "lanstash-app [no-io]: src/x.rs uses std::fs");
}
