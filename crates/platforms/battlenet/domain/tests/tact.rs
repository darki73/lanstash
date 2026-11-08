//! Tests for `lanstash_battlenet_domain::tact`, on hand-made tables and configs shaped
//! like Battle.net's.

use lanstash_battlenet_domain::tact::*;

const H1: &str = "ca8e463507cfff95a5759b5550f3eae5";
const H2: &str = "053433e65726aa5f1ae961168e3cb576";

#[test]
fn cdn_host_and_path_of_a_region() {
    let cdns = "Name!STRING:0|Path!STRING:0|Hosts!STRING:0\n## seqn = 2\n\
                eu|tpr/catalogs|level3.blizzard.com eu.cdn.blizzard.com\n\
                us|tpr/catalogs|us.cdn.blizzard.com level3.blizzard.com\n";
    assert_eq!(cdn(cdns, "us"), Some(("us.cdn.blizzard.com".into(), "tpr/catalogs".into())));
    assert_eq!(cdn(cdns, "kr"), Some(("level3.blizzard.com".into(), "tpr/catalogs".into())), "else the first");
    assert_eq!(cdn("Name|Path|Hosts\nus|../etc|h\n", "us"), None);
    assert_eq!(
        cdn_hosts(cdns, "eu"),
        Some((vec!["level3.blizzard.com".into(), "eu.cdn.blizzard.com".into()], "tpr/catalogs".into()))
    );
    assert_eq!(cdn_hosts("Name|Path|Hosts\nus|tpr/x|a/b c\n", "us"), Some((vec!["c".into()], "tpr/x".into())));
}

#[test]
fn file_targets_and_config_values() {
    assert_eq!(
        target("tpr/catalogs", "config", H1).as_deref(),
        Some("/tpr/catalogs/config/ca/8e/ca8e463507cfff95a5759b5550f3eae5")
    );
    assert_eq!(target("tpr/catalogs", "data", "../x"), None);
    assert_eq!(target("tpr/catalogs", "data", &H1.to_uppercase()), None);
    let config = format!("# Build Configuration\n\nroot = {H2}\nbuild-num = 5240\n");
    assert_eq!(config_value(&config, "root"), Some(H2));
    assert_eq!(config_value(&config, "nope"), None);
}

#[test]
fn a_products_build_in_a_region() {
    let versions = format!(
        "Region!STRING:0|BuildConfig!HEX:16|CDNConfig!HEX:16|KeyRing!HEX:16|BuildId!DEC:4|VersionsName!String:0\n\
         ## seqn = 3\n\
         us|{H1}|{H2}||93847|3.3.93847\n\
         eu|{H2}|{H1}||93846|3.3.93846\n"
    );
    let eu = version(&versions, "eu").unwrap();
    assert_eq!((eu.build_config.as_str(), eu.cdn_config.as_str(), eu.name.as_str()), (H2, H1, "3.3.93846"));
    assert_eq!(version(&versions, "kr"), None);
    assert_eq!(version("No matched data", "us"), None);
}

#[test]
fn list_values_of_a_config() {
    let config = format!("archives = {H1} {H2}\narchive-group = {H1}\n");
    assert_eq!(config_list(&config, "archives"), vec![H1, H2]);
    assert!(config_list(&config, "patch-archives").is_empty());
}

#[test]
fn files_a_build_config_names() {
    let (ck, ek, dk) =
        ("c0000000000000000000000000000000", "e0000000000000000000000000000000", "d0000000000000000000000000000000");
    let bc = format!(
        "# Build Configuration
root = {ck}
encoding = {ck} {ek}
encoding-size = 10 11
download = {ck} {dk}
build-name = 1 2
bad = {ck} nothex
install = {ck} {ek}
"
    );
    assert_eq!(build_files(&bc), vec![("encoding".into(), ek.into(), 11), ("download".into(), dk.into(), 0)]);
}

/// An index as the CDN writes it: `entries` in blocks of `block_kib` KiB,
/// a table of contents, and the footer.
fn index(entries: &[([u8; 16], u32, Option<u32>)], block_kib: u8) -> Vec<u8> {
    let block = usize::from(block_kib) * 1024;
    let entry = 16 + 4 + if entries.first().is_some_and(|e| e.2.is_some()) { 4 } else { 0 };
    let mut data = Vec::new();
    for (key, size, offset) in entries {
        if data.len() % block + entry > block {
            data.resize(data.len().div_ceil(block) * block, 0);
        }
        data.extend_from_slice(key);
        data.extend_from_slice(&size.to_be_bytes());
        if let Some(o) = offset {
            data.extend_from_slice(&o.to_be_bytes());
        }
    }
    data.resize(data.len().div_ceil(block) * block, 0);
    data.extend_from_slice(&[0xAB; 24]); // table of contents (not read)
    data.extend_from_slice(&[0xCD; 8]); // its hash
    let offset_bytes = if entry == 24 { 4 } else { 0 };
    data.extend_from_slice(&[1, 0, 0, block_kib, offset_bytes, 4, 16, 8]);
    data.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    data.extend_from_slice(&[0xEF; 8]); // footer checksum
    data
}

#[test]
fn file_and_archive_indexes() {
    let key = |b: u8| [b; 16];
    let file_index = index(&[(key(0x0a), 8946, None), (key(0x15), 4011, None)], 4);
    let e = parse_index(&file_index).unwrap();
    assert_eq!(e.len(), 2);
    assert_eq!((e[0].key.as_str(), e[0].size, e[0].offset), ("0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a", 8946, None));
    assert_eq!(e[1].size, 4011);
    // Entries do not straddle blocks: 1024 / 24 = 42 per 1 KiB block.
    let many: Vec<_> = (0..100u8).map(|i| (key(i + 1), u32::from(i), Some(u32::from(i) * 10))).collect();
    let archive = parse_index(&index(&many, 1)).unwrap();
    assert_eq!(archive.len(), 100);
    assert_eq!((archive[42].size, archive[42].offset), (42, Some(420)), "first of the second block");
    assert_eq!(archive[99].key, "64646464646464646464646464646464");
}

#[test]
fn malformed_indexes_are_refused() {
    assert_eq!(parse_index(&[0; 10]), Err(IndexError::TooShort));
    let mut bad = index(&[([1; 16], 1, None)], 4);
    let n = bad.len();
    bad[n - 14] = 0; // key size 0
    assert_eq!(parse_index(&bad), Err(IndexError::BadFooter));
    let mut short = index(&[([1; 16], 1, None)], 4);
    let n = short.len();
    short[n - 12] = 200; // more entries than the block holds
    assert_eq!(parse_index(&short), Err(IndexError::Truncated));
}
