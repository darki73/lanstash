use lanstash_architecture::comments::check_em_dashes;

#[test]
fn finds_em_dashes_by_line() {
    let text = "one\ntwo \u{2014} three\nfour - five\n\u{2014}";
    let v = check_em_dashes("crates/x/README.md", text);
    let lines: Vec<&str> = v.iter().map(|v| v.detail.as_str()).collect();
    assert_eq!(lines, vec!["em-dash at crates/x/README.md:2", "em-dash at crates/x/README.md:4"]);
    assert!(check_em_dashes("a.rs", "a - b \u{2013} c").is_empty(), "hyphens and en-dashes are fine");
}
