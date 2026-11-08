//! Architecture rules for the lanstash workspace and the checks that enforce them.
//!
//! `workspace` loads the members and their files from disk; `manifest` and
//! `source` parse `Cargo.toml`, `Cargo.lock` and Rust sources; `deps`,
//! `sources`, `files`, `mirror`, `file_size`, `module_imports` and `comments`
//! hold the checks as pure functions over that data; `policy` holds the
//! lanstash-specific tables and allow-lists; `audit` runs every check.

#![forbid(unsafe_code)]

pub mod audit;
pub mod comments;
pub mod deps;
pub mod external;
pub mod file_size;
pub mod files;
pub mod manifest;
pub mod mirror;
pub mod module_imports;
pub mod policy;
pub mod source;
pub mod sources;
pub mod violation;
pub mod workspace;
