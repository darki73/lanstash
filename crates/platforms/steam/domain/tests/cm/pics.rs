//! Tests for `lanstash_steam_domain::cm::pics`: answers built with the field
//! numbers of Steam's definitions.

use lanstash_steam_domain::cm::pics::{
    AccessTokens, License, ProductInfo, access_token_request, licenses, product_info_request,
};
use lanstash_steam_domain::cm::proto::{Wire, Writer, fields};

#[test]
fn licenses_carry_their_package_and_token() {
    let one = |id: u64, token: u64| Writer::new().varint(1, id).fixed32(2, 7).varint(17, token).finish();
    let body = Writer::new().varint(1, 1).bytes(2, &one(37, 963)).bytes(2, &one(1580, 0)).finish();
    assert_eq!(
        licenses(&body).unwrap(),
        [License { package_id: 37, access_token: 963 }, License { package_id: 1580, access_token: 0 }]
    );
}

#[test]
fn token_requests_and_answers() {
    let r = access_token_request(&[37], &[1_422_450, 7]);
    let f: Vec<_> = fields(&r).map(Result::unwrap).collect();
    assert_eq!(f, [(1, Wire::Varint(37)), (2, Wire::Varint(1_422_450)), (2, Wire::Varint(7))]);
    let pair = |id: u64, t: u64| Writer::new().varint(1, id).varint(2, t).finish();
    let body = Writer::new()
        .bytes(1, &pair(37, 5))
        .varint(2, 40)
        .bytes(3, &pair(1_422_450, 6))
        .bytes(4, &[0x07, 0x08]) // packed: 7 and 8
        .finish();
    let t = AccessTokens::decode(&body).unwrap();
    assert_eq!((t.packages, t.apps), (vec![(37, 5)], vec![(1_422_450, 6)]));
    assert_eq!((t.denied_packages, t.denied_apps), (vec![40], vec![7, 8]));
}

#[test]
fn product_info_requests_and_answers() {
    let r = product_info_request(&[(37, 5)], &[(1_422_450, 0)], true);
    let f: Vec<_> = fields(&r).map(Result::unwrap).collect();
    assert_eq!(f.len(), 3);
    assert_eq!(fields(f[0].1.bytes().unwrap()).count(), 2, "a package with its token");
    assert_eq!(fields(f[1].1.bytes().unwrap()).count(), 1, "an app without one");
    assert_eq!(f[2], (3, Wire::Varint(1)));
    let app =
        Writer::new().varint(1, 1_422_450).varint(2, 40_643_967).varint(3, 1).bytes(5, b"\"appinfo\" {}").finish();
    let pkg = Writer::new().varint(1, 37).bytes(5, &[1, 0, 0, 0, 8]).finish();
    let body = Writer::new().bytes(1, &app).varint(2, 9).bytes(3, &pkg).varint(4, 10).bool(6, true).finish();
    let p = ProductInfo::decode(&body).unwrap();
    assert_eq!((p.apps[0].id, p.apps[0].change_number, p.apps[0].missing_token), (1_422_450, 40_643_967, true));
    assert_eq!(p.apps[0].buffer, b"\"appinfo\" {}");
    assert_eq!(
        (p.packages[0].id, p.unknown_apps.clone(), p.unknown_packages.clone(), p.pending),
        (37, vec![9], vec![10], true)
    );
}
