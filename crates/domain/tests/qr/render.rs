//! Tests for `lanstash_domain::qr::render`.

use lanstash_domain::qr::encode;
use lanstash_domain::qr::render::{QUIET, svg, text};

#[test]
fn the_svg_has_the_quiet_zone_and_a_white_background() {
    let q = encode(b"A", Some(0)).unwrap();
    let s = svg(&q);
    let n = q.size + 2 * QUIET;
    assert!(s.starts_with(&format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {n} {n}\"")));
    assert!(s.contains("fill=\"#fff\"") && s.ends_with("</svg>"));
    // The top-left finder's first row: 7 dark modules from (QUIET, QUIET).
    assert!(s.contains(&format!("M{QUIET} {QUIET}h7v1h-7z")));
}

#[test]
fn the_text_has_two_rows_per_line() {
    let q = encode(b"A", Some(0)).unwrap();
    let t = text(&q);
    let lines: Vec<&str> = t.lines().collect();
    assert_eq!(lines.len(), (q.size + 4).div_ceil(2));
    assert!(lines.iter().all(|l| l.starts_with("\x1b[30;107m") && l.ends_with("\x1b[0m")));
    // Finder rows 0 and 1 (after the two light quiet rows): all dark on top, the edges below.
    assert!(lines[1].contains("  █▀▀▀▀▀█"), "{:?}", lines[1]);
}
