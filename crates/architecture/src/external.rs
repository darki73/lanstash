//! The third-party crates lanstash may have in its lock file, and why.
//!
//! - TLS for the Steam transport and HTTPS to web APIs, with what it pulls in
//!   (licences: MIT, Apache-2.0, ISC, BSD-3-Clause; webpki-roots CDLA-Permissive-2.0; ring
//!   Apache-2.0 AND ISC). The platform-only ones (windows-*, libc, wasi) are getrandom's and ring's
//!   targets.
//! - SQLite, for what is kept beyond a restart: rusqlite and libsqlite3-sys (MIT; SQLite 3.53.2,
//!   public domain, compiled in), bitflags, fallible-iterator, fallible-streaming-iterator,
//!   smallvec, pkg-config and vcpkg (MIT OR Apache-2.0; the last two look for a system SQLite,
//!   which the bundled build never uses).

/// Third-party crates allowed in `Cargo.lock`.
pub const ALLOWED: &[&str] = &[
    "bitflags",
    "cc",
    "cfg-if",
    "fallible-iterator",
    "fallible-streaming-iterator",
    "find-msvc-tools",
    "getrandom",
    "libc",
    "libsqlite3-sys",
    "once_cell",
    "pkg-config",
    "ring",
    "rusqlite",
    "rustls",
    "rustls-pki-types",
    "rustls-webpki",
    "shlex",
    "smallvec",
    "subtle",
    "untrusted",
    "vcpkg",
    "wasi",
    "webpki-roots",
    "windows-sys",
    "windows-targets",
    "windows_aarch64_gnullvm",
    "windows_aarch64_msvc",
    "windows_i686_gnu",
    "windows_i686_gnullvm",
    "windows_i686_msvc",
    "windows_x86_64_gnu",
    "windows_x86_64_gnullvm",
    "windows_x86_64_msvc",
    "zeroize",
];
