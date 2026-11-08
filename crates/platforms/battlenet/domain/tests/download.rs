//! Tests for `lanstash_battlenet_domain::download`, on manifests built here in the
//! layout Battle.net's have.

use lanstash_battlenet_domain::download::{ManifestError, install_tags, parse, retarget, select};

/// A version-1 manifest: entries (key byte, size) and tags (name, type, entry indexes).
fn manifest(version: u8, entries: &[(u8, u64)], tags: &[(&str, u16, &[usize])]) -> Vec<u8> {
    let mut d = b"DL".to_vec();
    d.extend_from_slice(&[version, 16, 1]);
    d.extend_from_slice(&(entries.len() as u32).to_be_bytes());
    d.extend_from_slice(&(tags.len() as u16).to_be_bytes());
    if version >= 2 {
        d.push(1); // one flag byte per entry
    }
    if version >= 3 {
        d.extend_from_slice(&[0; 4]);
    }
    for (k, size) in entries {
        d.extend_from_slice(&[*k; 16]);
        d.extend_from_slice(&size.to_be_bytes()[3..]);
        d.push(0); // priority
        d.extend_from_slice(&[0xAA; 4]); // checksum
        if version >= 2 {
            d.push(0);
        }
    }
    for (name, kind, which) in tags {
        d.extend_from_slice(name.as_bytes());
        d.push(0);
        d.extend_from_slice(&kind.to_be_bytes());
        let mut mask = vec![0u8; entries.len().div_ceil(8)];
        for &i in *which {
            mask[i / 8] |= 0x80 >> (i % 8);
        }
        d.extend_from_slice(&mask);
    }
    d
}

fn sample(version: u8) -> Vec<u8> {
    let entries: Vec<(u8, u64)> = (0..10).map(|i| (i + 1, 1000 * u64::from(i + 1))).collect();
    manifest(
        version,
        &entries,
        &[
            ("Windows", 1, &[0, 1, 2, 3, 4, 5, 6]),
            ("OSX", 1, &[0, 1, 2, 7, 8]),
            ("enUS", 2, &[0, 1, 3, 4, 7, 9]),
            ("deDE", 2, &[0, 1, 2, 5, 8, 9]),
            ("x86_64", 3, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
        ],
    )
}

#[test]
fn entries_and_tags() {
    for version in 1..=3 {
        let m = parse(&sample(version)).unwrap();
        assert_eq!(m.entries.len(), 10, "version {version}");
        assert_eq!((m.entries[0].key.as_str(), m.entries[0].size), ("01".repeat(16).as_str(), 1000));
        assert_eq!(m.entries[9].size, 10_000);
        assert_eq!(m.tags.iter().map(|t| (t.name.as_str(), t.kind)).collect::<Vec<_>>()[1], ("OSX", 1));
        assert!(m.tags[0].has(6) && !m.tags[0].has(7) && !m.tags[0].has(99));
    }
}

#[test]
fn an_install_takes_one_chosen_tag_of_each_type() {
    let m = parse(&sample(1)).unwrap();
    assert_eq!(select(&m, &["Windows", "enUS", "x86_64"]), vec![0, 1, 3, 4]);
    assert_eq!(select(&m, &["windows", "ENUS"]), vec![0, 1, 3, 4], "names without case");
    assert_eq!(select(&m, &["Windows", "enUS", "deDE"]), vec![0, 1, 2, 3, 4, 5], "either locale");
    assert_eq!(select(&m, &["Windows"]), (0..7).collect::<Vec<_>>(), "unchosen types do not filter");
    assert_eq!(select(&m, &["Unknown"]).len(), 10);
}

#[test]
fn another_platforms_install_swaps_the_tags_of_its_kinds_and_keeps_the_rest() {
    let m = parse(&sample(1)).unwrap();
    let chosen: Vec<String> = ["Windows", "x86_64", "enUS", "US"].iter().map(|s| (*s).to_string()).collect();
    let mac = retarget(&m, &chosen, &["osx"]).unwrap();
    assert_eq!(mac, ["x86_64", "enUS", "US", "OSX"], "the manifest's own spelling, the locale kept");
    let picked: Vec<&str> = mac.iter().map(String::as_str).collect();
    assert_eq!(select(&m, &picked), vec![0, 1, 7], "the Mac's English files");
    assert!(retarget(&m, &chosen, &["PlayStation"]).is_err(), "a tag the build has no install for");
    assert_eq!(retarget(&m, &chosen, &[]).unwrap(), chosen, "no other install: as set up");
    let i = install_tags(&m);
    assert_eq!((i.platforms, i.architectures), (vec!["Windows".to_string(), "OSX".into()], vec!["x86_64".to_string()]));
}

#[test]
fn another_locales_install_swaps_the_locale_alone() {
    let m = parse(&sample(1)).unwrap();
    assert_eq!(install_tags(&m).locales, ["enUS", "deDE"]);
    let chosen: Vec<String> = ["Windows", "x86_64", "enUS", "US"].iter().map(|s| (*s).to_string()).collect();
    let german = retarget(&m, &chosen, &["dede"]).unwrap();
    assert_eq!(german, ["Windows", "x86_64", "US", "deDE"]);
    let picked: Vec<&str> = german.iter().map(String::as_str).collect();
    assert_eq!(select(&m, &picked), vec![0, 1, 2, 5], "Windows' German files, not its English ones (3, 4)");
    let mac_german = retarget(&m, &chosen, &["OSX", "deDE"]).unwrap();
    assert_eq!(mac_german, ["x86_64", "US", "OSX", "deDE"], "platform and locale at once");
}

#[test]
fn only_the_platforms_the_app_installs_on_are_offered() {
    let entries: Vec<(u8, u64)> = (0..3).map(|i| (i + 1, 10)).collect();
    let consoles =
        manifest(1, &entries, &[("Windows", 1, &[0]), ("PS5", 1, &[1]), ("Linux", 1, &[2]), ("OSX", 1, &[2])]);
    let platforms = install_tags(&parse(&consoles).unwrap()).platforms;
    assert_eq!(platforms, ["Windows", "OSX"], "no console, phone or server build");
}

#[test]
fn malformed_manifests_are_refused() {
    assert_eq!(parse(b"DL"), Err(ManifestError::Truncated));
    let mut bad = sample(1);
    bad[0] = b'X';
    assert_eq!(parse(&bad), Err(ManifestError::BadHeader));
    let mut v9 = sample(1);
    v9[2] = 9;
    assert_eq!(parse(&v9), Err(ManifestError::BadHeader));
    let s = sample(1);
    assert_eq!(parse(&s[..s.len() - 1]), Err(ManifestError::Truncated));
    assert_eq!(parse(&s[..200]), Err(ManifestError::Truncated));
}

/// A real manifest, outside the repo: `LANSTASH_MANIFEST=<BLTE file>
/// LANSTASH_TAGS=Windows,x86_64,enUS cargo test -p lanstash-battlenet-domain
/// --test download -- --ignored --nocapture`.
#[test]
#[ignore = "needs a download manifest from the CDN"]
fn a_real_manifest() {
    let path = std::env::var("LANSTASH_MANIFEST").expect("LANSTASH_MANIFEST");
    let tags = std::env::var("LANSTASH_TAGS").unwrap_or_else(|_| "Windows,x86_64,enUS".into());
    let raw = std::fs::read(path).unwrap();
    let m = parse(&lanstash_battlenet_domain::blte::decode(&raw, 256 << 20).unwrap()).unwrap();
    let chosen: Vec<&str> = tags.split(',').collect();
    let picked = select(&m, &chosen);
    let bytes: u64 = picked.iter().map(|&i| m.entries[i].size).sum();
    println!("{} entries, {} tags; {tags}: {} entries, {bytes} bytes", m.entries.len(), m.tags.len(), picked.len());
    // Every tag: its type, how many entries carry it and their bytes.
    for t in &m.tags {
        let (n, b) =
            (0..m.entries.len()).filter(|&i| t.has(i)).fold((0u64, 0u64), |(n, b), i| (n + 1, b + m.entries[i].size));
        println!("  type {:>3} {:<24} {n:>8} entries {:>8.2} GB", t.kind, t.name, b as f64 / 1e9);
    }
}
