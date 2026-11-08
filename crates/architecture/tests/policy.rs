use std::collections::BTreeSet;

use lanstash_architecture::deps::check_layers;
use lanstash_architecture::policy::{LANSTASH, Layer};

#[test]
fn every_row_is_unique_and_names_known_crates() {
    let packages: BTreeSet<&str> = LANSTASH.crates.iter().map(|c| c.package).collect();
    let dirs: BTreeSet<&str> = LANSTASH.crates.iter().map(|c| c.dir).collect();
    assert_eq!(packages.len(), LANSTASH.crates.len());
    assert_eq!(dirs.len(), LANSTASH.crates.len());
    for c in LANSTASH.crates {
        for d in c.internal.iter().chain(c.dev_internal) {
            assert!(packages.contains(d), "{} names {d}, which has no row", c.package);
        }
    }
    assert!(check_layers(&LANSTASH).is_empty());
}

#[test]
fn only_the_root_names_a_platform() {
    let platforms: BTreeSet<&str> =
        LANSTASH.crates.iter().filter(|c| matches!(c.layer, Layer::Platform(_))).map(|c| c.package).collect();
    for c in LANSTASH.crates.iter().filter(|c| !matches!(c.layer, Layer::Platform(_) | Layer::Root)) {
        assert!(
            c.internal.iter().chain(c.dev_internal).all(|d| !platforms.contains(d)),
            "{} names a platform",
            c.package
        );
    }
}

#[test]
fn the_ceiling_is_400_lines_and_third_party_crates_only_for_tls_sqlite_crypto_and_sendfile() {
    assert_eq!(LANSTASH.max_lines, 400);
    let direct: Vec<&str> = LANSTASH.direct_external.iter().map(|(p, _)| *p).collect();
    assert_eq!(
        direct,
        vec![
            "lanstash-adapter-steam",
            "lanstash-adapter-http",
            "lanstash-adapter-sqlite",
            "lanstash-adapter-crypto",
            "lanstash-adapter-zerocopy"
        ],
        "TLS for Steam and for HTTPS to web APIs on the network (a DNS server's), \
         SQLite for what is kept beyond a restart, ring for lanstash's own accounts, libc for sendfile, \
         nowhere else"
    );
    assert_eq!(
        LANSTASH.direct_external.iter().find(|(p, _)| *p == "lanstash-adapter-crypto").map(|(_, c)| *c),
        Some(&["ring"][..]),
        "the crypto adapter uses ring alone, which rustls already brings in"
    );
    assert_eq!(
        LANSTASH.unsafe_code,
        [("lanstash-adapter-zerocopy", "crates/adapters/zerocopy/src/sendfile.rs")],
        "`unsafe` in one file: the sendfile call"
    );
    assert!(LANSTASH.external.contains(&"rustls") && LANSTASH.external.contains(&"webpki-roots"));
    assert!(LANSTASH.external.contains(&"rusqlite") && LANSTASH.external.contains(&"libsqlite3-sys"));
}
