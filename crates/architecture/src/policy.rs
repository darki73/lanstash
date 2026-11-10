//! The lanstash tables the rules check against: every crate with its layer and
//! allowed internal dependencies, the module import bans, the file ceiling and
//! the explicit allow-lists.

use crate::module_imports::{ImportBan, ban};

/// Where a crate sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// Pure logic and formats (`lanstash-domain`).
    Domain,
    /// Traits the use cases need, and the platform contract.
    Ports,
    /// The stash and the generic use cases.
    Core,
    /// Implementations of ports over files, network and processes.
    Adapter,
    /// One platform's crates; the name groups the crates of one platform.
    Platform(&'static str),
    /// The composition root.
    Root,
    /// Workspace tooling (these rules).
    Tool,
}

impl Layer {
    /// Domain, ports and core: what platforms may build on.
    #[must_use]
    pub fn is_core(self) -> bool {
        matches!(self, Self::Domain | Self::Ports | Self::Core)
    }
}

/// The rules of one crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CratePolicy {
    /// Directory from the workspace root.
    pub dir: &'static str,
    pub package: &'static str,
    pub layer: Layer,
    /// Workspace crates it may depend on (normal and build dependencies).
    pub internal: &'static [&'static str],
    /// Further workspace crates its tests may depend on.
    pub dev_internal: &'static [&'static str],
    /// The crate root carries `#![no_std]`.
    pub no_std: bool,
    /// Its sources name none of [`NO_IO_STD`].
    pub no_io: bool,
}

/// `std` modules a crate without I/O never names.
pub const NO_IO_STD: &[&str] = &["fs", "net", "process", "env", "io", "time"];

/// Calls only the composition root may make.
pub const ROOT_ONLY_CALLS: &[&str] = &["env::args", "env::var", "SystemTime::now", "Instant::now"];

/// Markers of process-wide mutable or lazily initialised state.
pub const GLOBAL_STATE: &[&str] = &["static mut", "OnceLock", "LazyLock", "lazy_static", "thread_local!", "OnceCell"];

/// Everything the rules are checked against.
#[derive(Debug, Clone, Copy)]
pub struct Policy {
    pub crates: &'static [CratePolicy],
    /// Third-party crates allowed in `Cargo.lock`.
    pub external: &'static [&'static str],
    /// Which package may depend on which of them directly: (package, crates). A third-party crate
    /// is approved for the packages named here only.
    pub direct_external: &'static [(&'static str, &'static [&'static str])],
    /// Line ceiling of every `.rs` file under `src/` and `tests/`.
    pub max_lines: usize,
    /// Files over the ceiling, by path from the root.
    pub oversized: &'static [&'static str],
    /// Files that may still hold `pub use`, by path from the root.
    pub reexports: &'static [&'static str],
    /// Packages that may still lack `README.md` or `CHANGELOG.md`.
    pub undocumented: &'static [&'static str],
    /// Source modules without a test file: (package, module path under `src/` without `.rs`, why).
    pub untested: &'static [(&'static str, &'static str, &'static str)],
    pub module_bans: &'static [ImportBan],
    /// The only files that may hold `unsafe`: (package, path from the root). Such a package's
    /// root denies `unsafe` (rather than forbidding it, which no item could lift) and it keeps
    /// lints of its own (the workspace's forbid it); everywhere else it is forbidden.
    pub unsafe_code: &'static [(&'static str, &'static str)],
}

impl Policy {
    /// The policy of `package`.
    #[must_use]
    pub fn crate_policy(&self, package: &str) -> Option<&CratePolicy> {
        self.crates.iter().find(|c| c.package == package)
    }

    /// Whether `package` holds the `unsafe` the policy allows.
    #[must_use]
    pub fn allows_unsafe(&self, package: &str) -> bool {
        self.unsafe_code.iter().any(|(p, _)| *p == package)
    }
}

const DOMAIN: &str = "lanstash-domain";
const PORTS: &str = "lanstash-ports";
const STORE: &str = "lanstash-store";
const APP: &str = "lanstash-app";
const ADAPTER_OS: &str = "lanstash-adapter-os";
const ADAPTER_FS: &str = "lanstash-adapter-fs";
const ADAPTER_DNS: &str = "lanstash-adapter-dns";
const ADAPTER_HTTP: &str = "lanstash-adapter-http";
const ADAPTER_ZEROCOPY: &str = "lanstash-adapter-zerocopy";
const ADAPTER_LOGS: &str = "lanstash-adapter-logs";
const ADAPTER_NAMES: &str = "lanstash-adapter-names";
const ADAPTER_ADMIN_API: &str = "lanstash-adapter-admin-api";
const ADAPTER_STEAM: &str = "lanstash-adapter-steam";
const ADAPTER_SQLITE: &str = "lanstash-adapter-sqlite";
const ADAPTER_CRYPTO: &str = "lanstash-adapter-crypto";

/// The row of an adapter crate: it does I/O, with `std`.
const fn adapter(
    dir: &'static str,
    package: &'static str,
    internal: &'static [&'static str],
    dev_internal: &'static [&'static str],
) -> CratePolicy {
    CratePolicy { dir, package, layer: Layer::Adapter, internal, dev_internal, no_std: false, no_io: false }
}
const BATTLENET_DOMAIN: &str = "lanstash-battlenet-domain";
const BATTLENET_APP: &str = "lanstash-battlenet-app";
const BATTLENET_API: &str = "lanstash-battlenet-api";
const STEAM_DOMAIN: &str = "lanstash-steam-domain";
const STEAM_APP: &str = "lanstash-steam-app";
const EPIC: &str = "lanstash-epic";
const EPIC_APP: &str = "lanstash-epic-app";
const RIOT: &str = "lanstash-riot";
const DNS_ADGUARD: &str = "lanstash-dns-adguard";
const DNS_PIHOLE: &str = "lanstash-dns-pihole";

/// The lanstash workspace.
pub const LANSTASH: Policy = Policy {
    crates: &[
        CratePolicy {
            dir: "crates/domain",
            package: DOMAIN,
            layer: Layer::Domain,
            internal: &[],
            dev_internal: &[],
            no_std: true,
            no_io: true,
        },
        CratePolicy {
            dir: "crates/ports",
            package: PORTS,
            layer: Layer::Ports,
            internal: &[DOMAIN],
            dev_internal: &[],
            no_std: true,
            no_io: true,
        },
        CratePolicy {
            dir: "crates/store",
            package: STORE,
            layer: Layer::Core,
            internal: &[DOMAIN, PORTS],
            dev_internal: &[STORE],
            no_std: false,
            no_io: true,
        },
        CratePolicy {
            dir: "crates/app",
            package: APP,
            layer: Layer::Core,
            internal: &[DOMAIN, PORTS, STORE],
            dev_internal: &[APP],
            no_std: false,
            no_io: true,
        },
        adapter("crates/adapters/os", ADAPTER_OS, &[PORTS], &[]),
        adapter("crates/adapters/fs", ADAPTER_FS, &[DOMAIN, PORTS, ADAPTER_OS], &[STORE]),
        adapter("crates/adapters/dns", ADAPTER_DNS, &[DOMAIN, PORTS], &[]),
        adapter("crates/adapters/zerocopy", ADAPTER_ZEROCOPY, &[], &[]),
        adapter(
            "crates/adapters/http",
            ADAPTER_HTTP,
            &[DOMAIN, PORTS, APP, ADAPTER_DNS, ADAPTER_ZEROCOPY],
            &[ADAPTER_CRYPTO],
        ),
        adapter("crates/adapters/logs", ADAPTER_LOGS, &[DOMAIN, PORTS, ADAPTER_HTTP], &[]),
        adapter("crates/adapters/names", ADAPTER_NAMES, &[DOMAIN, PORTS], &[]),
        adapter("crates/adapters/admin-api", ADAPTER_ADMIN_API, &[DOMAIN, PORTS, APP], &[STORE, ADAPTER_HTTP]),
        adapter("crates/adapters/steam", ADAPTER_STEAM, &[DOMAIN, PORTS], &[]),
        adapter("crates/adapters/sqlite", ADAPTER_SQLITE, &[PORTS], &[]),
        adapter("crates/adapters/crypto", ADAPTER_CRYPTO, &[DOMAIN, PORTS], &[]),
        CratePolicy {
            dir: "crates/platforms/battlenet/domain",
            package: BATTLENET_DOMAIN,
            layer: Layer::Platform("battlenet"),
            internal: &[DOMAIN],
            dev_internal: &[],
            no_std: true,
            no_io: true,
        },
        CratePolicy {
            dir: "crates/platforms/battlenet/app",
            package: BATTLENET_APP,
            layer: Layer::Platform("battlenet"),
            internal: &[DOMAIN, PORTS, STORE, APP, BATTLENET_DOMAIN],
            dev_internal: &[],
            no_std: false,
            no_io: true,
        },
        CratePolicy {
            dir: "crates/platforms/battlenet/api",
            package: BATTLENET_API,
            layer: Layer::Platform("battlenet"),
            internal: &[DOMAIN, PORTS, BATTLENET_DOMAIN, BATTLENET_APP],
            dev_internal: &[STORE, APP],
            no_std: false,
            no_io: true,
        },
        CratePolicy {
            dir: "crates/platforms/steam/domain",
            package: STEAM_DOMAIN,
            layer: Layer::Platform("steam"),
            internal: &[DOMAIN, PORTS],
            dev_internal: &[],
            no_std: true,
            no_io: true,
        },
        CratePolicy {
            dir: "crates/platforms/steam/app",
            package: STEAM_APP,
            layer: Layer::Platform("steam"),
            internal: &[DOMAIN, PORTS, STORE, APP, STEAM_DOMAIN],
            dev_internal: &[],
            no_std: false,
            no_io: true,
        },
        CratePolicy {
            dir: "crates/platforms/epic/domain",
            package: EPIC,
            layer: Layer::Platform("epic"),
            internal: &[DOMAIN, PORTS],
            dev_internal: &[],
            no_std: true,
            no_io: true,
        },
        CratePolicy {
            dir: "crates/platforms/epic/app",
            package: EPIC_APP,
            layer: Layer::Platform("epic"),
            internal: &[DOMAIN, PORTS, STORE, APP, EPIC],
            dev_internal: &[],
            no_std: false,
            no_io: true,
        },
        CratePolicy {
            dir: "crates/platforms/riot",
            package: RIOT,
            layer: Layer::Platform("riot"),
            internal: &[DOMAIN, PORTS],
            dev_internal: &[],
            no_std: true,
            no_io: true,
        },
        // A DNS server lanstash works with is built like a platform: on core alone, self-contained.
        CratePolicy {
            dir: "crates/dns/adguard",
            package: DNS_ADGUARD,
            layer: Layer::Platform("dns-adguard"),
            internal: &[DOMAIN, PORTS],
            dev_internal: &[],
            no_std: true,
            no_io: true,
        },
        CratePolicy {
            dir: "crates/dns/pihole",
            package: DNS_PIHOLE,
            layer: Layer::Platform("dns-pihole"),
            internal: &[DOMAIN, PORTS],
            dev_internal: &[],
            no_std: true,
            no_io: true,
        },
        CratePolicy {
            dir: "crates/server",
            package: "lanstash",
            layer: Layer::Root,
            internal: &[
                DOMAIN,
                PORTS,
                STORE,
                APP,
                ADAPTER_OS,
                ADAPTER_FS,
                ADAPTER_DNS,
                ADAPTER_HTTP,
                ADAPTER_LOGS,
                ADAPTER_NAMES,
                ADAPTER_ADMIN_API,
                ADAPTER_SQLITE,
                ADAPTER_CRYPTO,
                BATTLENET_DOMAIN,
                BATTLENET_APP,
                BATTLENET_API,
                STEAM_DOMAIN,
                STEAM_APP,
                ADAPTER_STEAM,
                EPIC,
                EPIC_APP,
                RIOT,
                DNS_ADGUARD,
                DNS_PIHOLE,
            ],
            dev_internal: &[],
            no_std: false,
            no_io: false,
        },
        CratePolicy {
            dir: "crates/architecture",
            package: "lanstash-architecture",
            layer: Layer::Tool,
            internal: &[],
            dev_internal: &[],
            no_std: false,
            no_io: false,
        },
    ],
    // TLS and SQLite, with what they pull in: see `external`.
    external: crate::external::ALLOWED,
    // TLS: the Steam transport, and HTTPS to web APIs on the network (a DNS server's admin API;
    // ring for a pinned certificate's SHA-256).
    direct_external: &[
        (ADAPTER_STEAM, &["rustls", "webpki-roots"]),
        (ADAPTER_HTTP, &["rustls", "webpki-roots", "ring"]),
        (ADAPTER_SQLITE, &["rusqlite"]),
        // lanstash's own accounts: PBKDF2 password hashes and secure random bytes.
        (ADAPTER_CRYPTO, &["ring"]),
        // The one system call std has no safe function for (sendfile), behind a safe one.
        (ADAPTER_ZEROCOPY, &["libc"]),
    ],
    max_lines: 400,
    oversized: &[],
    reexports: &[],
    undocumented: &[],
    untested: &[
        (DOMAIN, "segment/bytes", "crate-private field access; every segment structure's round trip runs it"),
        (APP, "serve/reply", "crate-private answers; every request test in tests/serve runs them"),
        ("lanstash", "run", "the binary's wiring; not reachable from tests/"),
        ("lanstash", "run/platforms", "the binary's platform wiring; not reachable from tests/"),
        ("lanstash", "run/serve", "the binary's serve and stop commands; not reachable from tests/"),
        ("lanstash", "run/store", "the binary's store commands; not reachable from tests/"),
        ("lanstash", "setup", "the binary's install and doctor commands; not reachable from tests/"),
        ("lanstash", "remote", "the binary's admin-API client commands; not reachable from tests/"),
        ("lanstash", "run/steam", "the binary's Steam login commands; not reachable from tests/"),
        (
            "lanstash",
            "run/admin",
            "the binary's admin servers (HTTPS: lanstash_adapter_http tests/link.rs); not reachable",
        ),
        ("lanstash", "run/users", "the binary's account commands (their rules: lanstash_app::accounts); not reachable"),
        ("lanstash", "run/epic", "the binary's Epic wiring; not reachable from tests/"),
        ("lanstash", "run/kept", "the binary's database wiring; not reachable from tests/"),
        ("lanstash", "run/clients", "the binary's client-naming wiring; not reachable from tests/"),
        ("lanstash", "run/health", "the binary's health wiring (its rules: lanstash_app::health); not reachable"),
        ("lanstash", "run/releases", "the binary's release wiring (its rules: lanstash_app::releases); not reachable"),
    ],
    module_bans: &[
        // The admin API reads the stash only through the status use case and the platforms.
        ban(ADAPTER_ADMIN_API, "*", "lanstash_store"),
        // A platform's views render what its use cases found; they start no work of their own.
        ban(BATTLENET_API, "*", "std::thread"),
        ban(BATTLENET_API, "*", "lanstash_store"),
        ban("lanstash", "cli", "std::fs"),
        ban("lanstash", "cli", "std::process"),
        ban("lanstash", "cli", "std::thread"),
        ban("lanstash", "cli", "lanstash_adapter_*"),
        ban("lanstash", "config", "std::fs"),
        ban("lanstash", "config", "std::process"),
        ban("lanstash", "config", "std::thread"),
        ban("lanstash", "config", "lanstash_adapter_*"),
        ban("lanstash", "install", "std::fs"),
        ban("lanstash", "install", "std::process"),
        ban("lanstash", "install", "std::thread"),
        ban("lanstash", "install", "lanstash_adapter_*"),
        ban("lanstash", "report", "std::fs"),
        ban("lanstash", "report", "std::process"),
        ban("lanstash", "report", "std::thread"),
        ban("lanstash", "report", "lanstash_adapter_*"),
        ban("lanstash", "prefill_cli", "std::fs"),
        ban("lanstash", "prefill_cli", "std::process"),
        ban("lanstash", "prefill_cli", "std::thread"),
        ban("lanstash", "prefill_cli", "lanstash_adapter_*"),
        ban("lanstash", "dns", "std::fs"),
        ban("lanstash", "dns", "std::process"),
        ban("lanstash", "dns", "std::thread"),
        ban("lanstash", "dns", "lanstash_adapter_*"),
    ],
    // Sending a file to a socket without copying it through lanstash: one system call.
    unsafe_code: &[(ADAPTER_ZEROCOPY, "crates/adapters/zerocopy/src/sendfile.rs")],
};
