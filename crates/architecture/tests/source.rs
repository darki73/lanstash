use lanstash_architecture::source::{code_only, std_modules_used, strip_comments};

#[test]
fn code_only_empties_string_and_char_literals() {
    assert_eq!(code_only("let a = \"static mut\"; // x\nb"), "let a = \"\"; \nb");
    assert_eq!(code_only("r#\"a \"q\" b\"# + 'x' + '\\'' + \"\\\"\" c"), "r#\"\"# + '' + '' + \"\" c");
    assert_eq!(code_only("fn f<'a>(s: &'a str) {}"), "fn f<'a>(s: &'a str) {}");
    assert!(std_modules_used("let s = \"std::fs\";").is_empty());
}

#[test]
fn finds_std_modules_in_all_forms() {
    let src = "use std::fs::File;\nuse std::{\n    io::{self, Read},\n    net,\n};\nfn f() { ::std::process::exit(0) }\n\
               // std::env in a comment is ignored\nuse my_std::thing;\nlet t = std :: time::Instant::now();";
    let m = std_modules_used(src);
    assert_eq!(m, ["fs", "io", "net", "process"].iter().map(|s| (*s).to_owned()).collect());
    // `std :: time` with spaces is unusual; it is not caught.
    assert!(!m.contains("time"));
}

#[test]
fn strips_line_and_block_comments_and_keeps_text() {
    assert_eq!(strip_comments("a // b\nc /* d\ne */ f"), "a \nc  f");
    assert_eq!(strip_comments("x = 1 / 2; // ü\n\"é\""), "x = 1 / 2; \n\"é\"");
}
