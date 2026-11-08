//! Tests for `lanstash_battlenet_domain::check`: files checked against the hash in
//! their CDN path.

use lanstash_battlenet_domain::check::{INDEX_FOOTER, archive_range, hex16, whole_file};
use lanstash_domain::checksum::md5::md5;

fn hex(b: &[u8; 16]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn at(kind: &str, hash: &str, suffix: &str) -> String {
    format!("/tpr/ovw/{kind}/{}/{}/{hash}{suffix}", &hash[..2], &hash[2..4])
}

/// A header-less BLTE file holding `body`, and its encoding key.
fn blte(body: &[u8]) -> (Vec<u8>, String) {
    let mut f = b"BLTE\0\0\0\0N".to_vec();
    f.extend_from_slice(body);
    let key = hex(&md5(&f));
    (f, key)
}

#[test]
fn a_check_says_whether_it_can_judge_at_all() {
    use lanstash_battlenet_domain::check::Check;
    let h = "0123456789abcdef0123456789abcdef";
    for t in [at("config", h, ""), at("data", h, ".index"), at("patch", h, ".index"), at("data", h, "")] {
        assert!(Check::whole(&t).checks(), "{t}");
    }
    for t in ["/tpr/ovw/odd", "/tpr/ovw/config/01/23/not-a-hash", "/tpr/ovw/other/01/23/0123"] {
        assert!(!Check::whole(t).checks(), "{t}");
    }
    assert!(Check::range(0, &[(0, 10, h)]).checks());
    assert!(Check::range(0, &[]).checks(), "nothing inside: nothing to fail");
    assert!(!Check::range(0, &[(0, 10, h), (10, 20, "not-a-hash")]).checks(), "a file it cannot check");
}

#[test]
fn hex_hashes() {
    assert_eq!(hex16("000102030405060708090a0b0c0d0eff").unwrap()[15], 0xff);
    assert_eq!(hex16("00"), None);
    assert_eq!(hex16("zz0102030405060708090a0b0c0d0eff"), None);
}

#[test]
fn whole_files_by_kind() {
    let config = b"# Build Configuration\nroot = 00\n";
    let h = hex(&md5(config));
    assert!(whole_file(&at("config", &h, ""), config));
    assert!(!whole_file(&at("config", &h, ""), b"# changed"));
    // An index: the MD5 of its footer.
    let mut index = vec![7u8; 100];
    index.extend_from_slice(&[9u8; INDEX_FOOTER]);
    let h = hex(&md5(&index[index.len() - INDEX_FOOTER..]));
    assert!(whole_file(&at("data", &h, ".index"), &index));
    assert!(whole_file(&at("patch", &h, ".index"), &index));
    assert!(!whole_file(&at("data", &h, ".index"), &index[..INDEX_FOOTER - 1]));
    // A loose file: BLTE against its encoding key.
    let (f, key) = blte(b"loose");
    assert!(whole_file(&at("data", &key, ""), &f));
    assert!(!whole_file(&at("data", &key, ""), &f[..f.len() - 1]));
    // Anything else is never trusted.
    assert!(!whole_file(&at("patch", &key, ""), &f), "patch files are not planned");
    assert!(!whole_file("/tpr/ovw/versions", b""));
    assert!(!whole_file(&at("data", "nothex", ""), &f));
}

#[test]
fn an_archive_range_holds_its_files_intact() {
    let (a, ka) = blte(b"first file");
    let (b, kb) = blte(b"second");
    // Archive bytes from 100: a, 4 gap bytes, b.
    let mut data = a.clone();
    data.extend_from_slice(b"gap!");
    data.extend_from_slice(&b);
    let (a_at, b_at) = (100, 100 + a.len() as u64 + 4);
    let files = [(a_at, a_at + a.len() as u64, ka.as_str()), (b_at, b_at + b.len() as u64, kb.as_str())];
    assert!(archive_range(100, &data, &files));
    let mut gap = data.clone();
    gap[a.len()] = b'G';
    assert!(archive_range(100, &gap, &files), "gap bytes are not checked");
    let mut broken = data.clone();
    *broken.last_mut().unwrap() ^= 1;
    assert!(!archive_range(100, &broken, &files));
    // Files outside the range are not checked.
    assert!(archive_range(100, &data, &[(0, 50, "00000000000000000000000000000000")]));
}

/// Real CDN files in `LANSTASH_TACT_DIR`, named `<kind>-<hash>`:
/// `config-`, `index-` and `data-` (a loose BLTE file).
#[test]
#[ignore = "needs LANSTASH_TACT_DIR with real CDN files"]
fn real_files_check_against_their_paths() {
    let dir = std::env::var("LANSTASH_TACT_DIR").expect("LANSTASH_TACT_DIR");
    let mut checked = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let Some((kind, hash)) = name.split_once('-') else { continue };
        let target = match kind {
            "config" => at("config", hash, ""),
            "index" => at("data", hash, ".index"),
            "data" => at("data", hash, ""),
            _ => continue,
        };
        let data = std::fs::read(&path).unwrap();
        assert!(whole_file(&target, &data), "{name} does not check");
        let mut changed = data.clone();
        changed[0] ^= 1;
        let last = changed.len() - 1;
        changed[last] ^= 1;
        assert!(!whole_file(&target, &changed), "{name} checks when changed");
        println!("{name}: {} bytes: ok", data.len());
        checked += 1;
    }
    assert!(checked > 0);
}
