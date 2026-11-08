//! Source rules: crate roots forbid `unsafe` (and carry `#![no_std]` where the
//! row says), no re-exports, no inline tests, no global state, only the
//! composition root reads arguments, environment and clocks, and crates
//! without I/O name no I/O module of `std`.

use std::path::Path;

use crate::policy::{CratePolicy, GLOBAL_STATE, Layer, NO_IO_STD, Policy, ROOT_ONLY_CALLS};
use crate::source::{code_only, std_modules_used};
use crate::violation::Violation;
use crate::workspace::{Crate, slash};

fn is_root(relative: &Path) -> bool {
    relative == Path::new("lib.rs") || relative == Path::new("main.rs")
}

/// Whether a line of code has the keyword `unsafe` (a block, a function, an impl), or lifts
/// the lint against it; `unsafe_code` in `forbid`/`deny` names the lint, not the keyword.
fn names_unsafe(line: &str) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    line.contains("allow(unsafe_code)")
        || line.match_indices("unsafe").any(|(at, _)| {
            let before = line[..at].chars().next_back();
            let after = line[at + "unsafe".len()..].chars().next();
            !before.is_some_and(word) && !after.is_some_and(word)
        })
}

/// Every source rule for one crate.
#[must_use]
pub fn check_sources(krate: &Crate, rules: &CratePolicy, policy: &Policy) -> Vec<Violation> {
    let name = krate.name();
    let mut violations = Vec::new();
    let mut add = |rule: &'static str, detail: String| violations.push(Violation::new(name, rule, detail));
    for file in &krate.sources {
        let shown = format!("{}/src/{}", krate.dir, slash(&file.relative));
        let code = code_only(&file.text);
        if is_root(&file.relative) {
            // The crate the policy lets hold `unsafe` denies it (a forbid could not be lifted).
            let wanted = if policy.allows_unsafe(name) { "#![deny(unsafe_code)]" } else { "#![forbid(unsafe_code)]" };
            if !code.contains(wanted) {
                add("forbid-unsafe", format!("{shown} lacks {wanted}"));
            }
            if rules.no_std && file.relative == Path::new("lib.rs") && !code.contains("#![no_std]") {
                add("no-std", format!("{shown} lacks #![no_std]"));
            }
        }
        let unsafe_here = policy.unsafe_code.iter().any(|(p, f)| *p == name && *f == shown);
        for (i, line) in code.lines().enumerate() {
            let t = line.trim_start();
            if !unsafe_here && names_unsafe(t) {
                add("unsafe-where-allowed", format!("{shown}:{}: `unsafe` only in the files the policy names", i + 1));
            }
            if (t.starts_with("pub use ") || t.starts_with("pub(crate) use "))
                && !policy.reexports.contains(&shown.as_str())
            {
                add("no-reexport", format!("{shown}:{}", i + 1));
            }
            for p in ["#[cfg(test)]", "#[test]"] {
                if t.contains(p) {
                    add("no-inline-tests", format!("{shown}:{}: tests belong in tests/", i + 1));
                }
            }
            for p in GLOBAL_STATE {
                if t.contains(p) {
                    add("no-global-state", format!("{shown}:{} contains `{p}`: pass configuration in", i + 1));
                }
            }
            if !matches!(rules.layer, Layer::Root | Layer::Tool) {
                for p in ROOT_ONLY_CALLS {
                    if t.contains(p) {
                        add("root-only", format!("{shown}:{} uses `{p}`: only the composition root may", i + 1));
                    }
                }
            }
        }
        if rules.no_io {
            let used = std_modules_used(&file.text);
            let bad: Vec<&&str> = NO_IO_STD.iter().filter(|m| used.contains(**m)).collect();
            if !bad.is_empty() {
                add("no-io", format!("{shown} uses std::{bad:?}; go through a port instead"));
            }
        }
    }
    violations
}
