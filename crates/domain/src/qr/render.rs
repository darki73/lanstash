//! Drawing a [`Qr`]: an SVG for the web UI and half-block text for a
//! terminal, both with the quiet zone (light border) scanners need.

use alloc::string::String;
use core::fmt::Write;

use super::Qr;

/// Light modules around the code (the standard asks for 4).
pub const QUIET: usize = 4;

/// An SVG image, one unit per module, dark modules as one path; black on
/// white whatever the page's theme.
#[must_use]
pub fn svg(qr: &Qr) -> String {
    let n = qr.size + 2 * QUIET;
    let mut path = String::new();
    for row in 0..qr.size {
        let mut col = 0;
        while col < qr.size {
            if !qr.dark(row, col) {
                col += 1;
                continue;
            }
            let start = col;
            while col < qr.size && qr.dark(row, col) {
                col += 1;
            }
            let _ = write!(path, "M{} {}h{}v1h-{}z", start + QUIET, row + QUIET, col - start, col - start);
        }
    }
    let mut out = String::new();
    let _ = write!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {n} {n}\" shape-rendering=\"crispEdges\">\
         <rect width=\"{n}\" height=\"{n}\" fill=\"#fff\"/><path d=\"{path}\" fill=\"#000\"/></svg>"
    );
    out
}

/// Text for a terminal: two module rows per line in half blocks, black on
/// bright white (ANSI colours), so it scans on dark and light terminals alike.
#[must_use]
pub fn text(qr: &Qr) -> String {
    let q = 2usize; // a terminal's cells are tall; 2 light modules (4 rows) suffice
    let n = qr.size + 2 * q;
    let at = |r: usize, c: usize| r >= q && c >= q && qr.dark(r - q, c - q);
    let mut out = String::new();
    for r in (0..n).step_by(2) {
        out.push_str("\x1b[30;107m");
        for c in 0..n {
            out.push(match (at(r, c), at(r + 1, c)) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            });
        }
        out.push_str("\x1b[0m\n");
    }
    out
}
