//! Tests for `lanstash_domain::qr`: every mask of codes of versions 1, 3, 6,
//! 8 and 10 against Python's `qrcode` 8.2 (byte mode, level M), compared by
//! the SHA-1 of the module matrix (one byte per module, row by row).

mod render;
mod rs;

use lanstash_domain::checksum::sha1::sha1;
use lanstash_domain::qr::{MAX_BYTES, Qr, encode};

fn hash(q: &Qr) -> String {
    let flat: Vec<u8> =
        (0..q.size).flat_map(|r| (0..q.size).map(move |c| (r, c))).map(|(r, c)| u8::from(q.dark(r, c))).collect();
    sha1(&flat).iter().map(|b| format!("{b:02x}")).collect()
}

fn cases() -> Vec<Vec<u8>> {
    vec![
        b"".to_vec(),
        b"A".to_vec(),
        b"https://s.team/q/1/18400974508802468659".to_vec(),
        (32..127).collect(),
        b"lanstash ".repeat(15),
        vec![b'x'; 213],
    ]
}

/// (version, bytes, SHA-1 per mask 0..=7) from Python.
const EXPECTED: &[(u8, usize, [&str; 8])] = &[
    (
        1,
        0,
        [
            "46d662b62234e70c6da43fdeb547631df06aee8e",
            "c0b4f03887537aa498722dcf6c6d8099dfe339d1",
            "e24ac26f1b2e3aced83f6d0aa391162ab91113ae",
            "f94bcd3964086c46afbd0328f66ee15dacdd05b0",
            "af9d17e4ec4ebf46ff53c4faa7c310b7bd76187b",
            "b39d06b63ed44e0b12e5c1245b585e85abacd513",
            "e9ca53cfd7bc5d4036b18516b898c88cd7987d84",
            "3fde9bfa539f42087ef2bc24638bea28d9265284",
        ],
    ),
    (
        1,
        1,
        [
            "818b68dce4c2cbfe9876b0f1c19b6dec13ff57be",
            "860d975aa2a15ed5725c9ae3ab41921b6648b7d3",
            "7aa9cdfdad7071ecf113cf59f4f17c5f108945b2",
            "3105357f45f0040f81da0a79c0389f7ed3f7a9c1",
            "3ed82cf92902a6ee7a894162b1af5c1e1a096083",
            "0713a61a5689f999932da7166fcf0c056ef1d3bf",
            "ed810cd9550a9d28791bc4ae8b8f20489b33de23",
            "7387ed3d7e7b3d51769941ba92c5c288e579f396",
        ],
    ),
    (
        3,
        39,
        [
            "a334d765e6622b59d4e092e30edb8a6fc7afee84",
            "9dc91282be8d756003b1c84de02890165b5d7863",
            "680d057dcdd0457ef3e9222e156a8a9733697531",
            "00382181432f2003a52acd3756f860d8998be4c8",
            "4a1724b2940fdad81868cd28b47275d4d77ae173",
            "63873f571561bef3864e4da02db3f83c0a387dee",
            "b5ce9dd9072f46e50aa4ba5c1e6cf5f0597626f8",
            "1bc0d61a01bfcf74d75b20226ec29459c4468b5a",
        ],
    ),
    (
        6,
        95,
        [
            "f1afa64f551416ac241d4e51479c5e4f3ebb0fe5",
            "e8eb50e16ed890b934201475b78ec2d69b476749",
            "2ba0a8d89274d685ec72eda939244b3c5b749967",
            "afeb2bf4341732accd25f0608caccaeca8282318",
            "261b2d027ed1032be50c70800d30c9f7aaf6d803",
            "43ff8fadba88b6f039022f85f753583b2040bc4a",
            "1408741f136939f3fde9c89045f350b1b28cff08",
            "a35a07163fe606c843255451a991f3e7892493c1",
        ],
    ),
    (
        8,
        135,
        [
            "2455d633bd5be5be4e65d8ace576beb10cd4fc49",
            "ebee47d39d52f73784f93a945d88ca98f019e42c",
            "91f47a065cc5102a15b52d9f2d1cf8b52395f25a",
            "041f8db9f201414beebd25823d83261e968c798d",
            "9b0b1e16557dd7e5b6ce70a8708a5099ac658fb8",
            "88ce788e283c4cccd2878a7f71249a4f2d973753",
            "b7a5582cedfe571354e0e2a8a19f979f2a42dbf0",
            "7d5b46e26bb472bf6753917e30b9f3e35b5b893d",
        ],
    ),
    (
        10,
        213,
        [
            "6040874c5211ef7e7e5289454f870c3de45398a0",
            "51763028c851a55a879d0883c978eba4aeb7ed39",
            "6c7e7a44fc5afd584f9a26831734f9230a64858b",
            "ef1bc82bcd05cc6a7775f1703b565dead4634994",
            "7e1767a837094eae15ee4228b9224d5febf36a3e",
            "e3f5168f1c7ec5952cb72817fc8a44859a6e8b0b",
            "f94ec6ddd967f9432dffbdbd2c74cd7a175f0be1",
            "51b7a67f0341cf9ff68785792dbf7124ae6cd362",
        ],
    ),
];

#[test]
fn every_mask_matches_the_reference() {
    for (data, (version, len, masks)) in cases().iter().zip(EXPECTED) {
        assert_eq!(data.len(), *len);
        for (m, want) in masks.iter().enumerate() {
            let q = encode(data, Some(m as u8)).unwrap();
            assert_eq!((q.version, q.mask, q.size), (*version, m as u8, usize::from(*version) * 4 + 17));
            assert_eq!(hash(&q), *want, "{len} bytes, mask {m}");
        }
    }
}

#[test]
fn the_best_mask_is_one_of_the_eight_and_too_long_is_refused() {
    let q = encode(b"https://s.team/q/1/18400974508802468659", None).unwrap();
    assert!(q.mask < 8);
    assert_eq!(q, encode(b"https://s.team/q/1/18400974508802468659", Some(q.mask)).unwrap());
    assert!(encode(&[b'x'; MAX_BYTES + 1], None).is_none());
    assert!(!q.dark(q.size, 0), "outside the code is light");
}
