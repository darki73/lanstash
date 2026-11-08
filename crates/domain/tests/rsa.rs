//! Tests for `lanstash_domain::rsa`, against Python's integers: a 2048-bit
//! key made for this test (Miller-Rabin primes), padding bytes from a
//! seeded generator, and `pow(em, 65537, n)` as the expected ciphertext.

use lanstash_domain::rsa::{PublicKey, modpow};

fn unhex(s: &str) -> Vec<u8> {
    let s: String = s.split_whitespace().collect();
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

const N: &str = "
    9883f7b48eab9d10b698e5ea45b819db9b3c51eb0a2ea5f39260009c8df33ca04335b9bfe9e9b7b8bfce90a402badc1b
    5ac1e6effee9075e2c70568104b98484bd6a23fc9884fdb4045d2f69744a27c4dbff65b5ce5e6e36cf20d946be7834b5
    4aaae87f97b8ad99a28b401a9e1d62db432d36a2f1ab2117a4b6bb83acc0f9b24efd6409a3fa6c9f4a55d67e4c88013d
    75f36738d9aca808d15ff228a401ae42a8e1c8232a176e96fec3b07822c4dbe63c8b6ded7c56232e53ce76bbe5138eec
    b0832247fc032bd8cf4616bf4a8c1af4f88199823631f7ac68c95829d75573311cc7519a6d727ddad4c456b3aa62a668
    a099f5bdee15fd859c83e6aea5d3499b
";

const RANDOM: &str = "
    de5864a2a4ea638a58925f3f14e05adc31c550dcfa871333912b21548645ca12383a97d3a62027fa891417710f0122cf
    64006c5e227e15a882457a52a9650499c9dbd1b3e864f5932d8c214edb2d80d011a5e30c6dc7956302c2fe19da570ff2
    e371a4ea1cf975d3c0df888e94309bee76dbcec46e0d8935ec71243f4a176b4f799cf1a81572ba677f9e0263e2b2dee1
    27ab91b9138c9216062fa4471b29556ea81b6cd57997dcf69229478759871307fab83028c6e651adf813bbcc984fefd9
    68365f2d5a50bf3414137f533eb13f5bfcfa091621bc6ad15f55f461b0d06760eff3c53cb37a893172ecbff2ae113874
    b16e832262d542b30f122e3526bc4750fc6cdfe0ea6841a3df1c4e03e5dfbf2e90f4aa7396ab27a2464c791e637fb675
    c9b2920565e304c72a6a223dc248cc2a5f28e67f1efd40f2edeac4b8e0bf6c945a4a8c261718d7d92473c3d9b3ca2a0a
    75f70814af7bf6e2b018a6ec027c739f1d18aae88d715142a1ce86b7030d121dffc82c99e1b6819538187bb14a6aef2d
    4dc0c3b221bc0daa95c8fcbab42450c80a588049fce08c15550df219489175aef2d100cdf838c86523309110cf39ad15
    63a5dc91e30fa78702ce357ee514dcac1cab926dc88c26717fe691712803585b4e9d17f8d268c773cf7c484054206168
    113fd5c8d2e2d1df82d4be84d2f6988e2d5a514c7c745430205d737183d8b037
";

const CIPHER: &str = "
    429620e1be3771cc41739bdf9d8e1f887ad4f1eed2c3abe4f7b77673740a2f8f59215499217ec43fdaa43031b4ee7168
    1562ffa129cd72c019a75e9aea08f015f151c76d1369a5fcab7db3a76e7e26cb4f0037f60de3106adc47dded37e40128
    84ca80395eb49512d95fa4bb2ea97e9940abde92217faf89ecf7074e5bee933aa363cec27ed31b0c15e0cf85367aeaf4
    aa82d0be8c96038f314fa5763652c1de6ac6d2582a14d95596d25b294ee386f3f27d7d8a76871ea34e1ce3581c684b79
    0903f2ec3a92524f831a804b4998f7dc87860d44aadb177d6e2a2fed1588c2bb229dbe599f610523789f718730c32fea
    83c9c49ffc50a3b714eac40f8da645ee
";

#[test]
fn modpow_matches_small_cases() {
    assert_eq!(modpow(&[4], &[13], &[0x01, 0xF1]), Some(vec![0x01, 0xBD])); // 4^13 mod 497 = 445
    assert_eq!(modpow(&[7], &[], &[0x0B]), Some(vec![1])); // x^0 = 1
    assert_eq!(modpow(&[0x20], &[1], &[0x0B]), Some(vec![10])); // 32 mod 11
    assert_eq!(modpow(&[2], &[3], &[0]), None);
}

#[test]
fn pkcs1_encryption_matches_python() {
    let key = PublicKey::new(&unhex(N), &[1, 0, 1]).unwrap();
    assert_eq!(key.size(), 256);
    let c = key.encrypt_pkcs1(b"correct horse battery staple", &unhex(RANDOM)).unwrap();
    assert_eq!(c, unhex(CIPHER));
}

#[test]
fn too_long_messages_too_little_randomness_and_bad_keys_are_refused() {
    let key = PublicKey::new(&unhex(N), &[1, 0, 1]).unwrap();
    assert!(key.encrypt_pkcs1(&[b'x'; 246], &unhex(RANDOM)).is_none()); // over k - 11
    assert!(key.encrypt_pkcs1(&[b'x'; 245], &unhex(RANDOM)).is_some());
    assert!(key.encrypt_pkcs1(b"pw", &[0u8; 1000]).is_none());
    let mut even = unhex(N);
    *even.last_mut().unwrap() &= 0xFE;
    assert!(PublicKey::new(&even, &[3]).is_none());
    assert!(PublicKey::new(&[0xFF; 32], &[3]).is_none());
    // Leading zero bytes of the modulus do not count towards its size.
    let mut padded = vec![0, 0];
    padded.extend(unhex(N));
    assert_eq!(PublicKey::new(&padded, &[1, 0, 1]).unwrap().size(), 256);
}
