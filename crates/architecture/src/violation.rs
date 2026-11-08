//! One broken rule.

use std::fmt;

/// One broken rule in one package (or file, for repository-wide rules).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub package: String,
    pub rule: &'static str,
    pub detail: String,
}

impl Violation {
    #[must_use]
    pub fn new(package: &str, rule: &'static str, detail: impl Into<String>) -> Self {
        Self { package: package.to_owned(), rule, detail: detail.into() }
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]: {}", self.package, self.rule, self.detail)
    }
}
