//! Tests for `lanstash_steam_domain::cm::content`: requests read back, and
//! answers shaped like Steam's (a CDN list, a request code).

use lanstash_steam_domain::cm::content::{ContentServer, request_code, request_code_request, servers, servers_request};
use lanstash_steam_domain::cm::proto::{Wire, Writer, fields};

#[test]
fn requests_name_the_cell_and_the_manifest() {
    let servers_body = servers_request(127, 20);
    let f: Vec<_> = fields(&servers_body).map(Result::unwrap).collect();
    assert_eq!(f, [(1, Wire::Varint(127)), (2, Wire::Varint(20))]);
    let code_body = request_code_request(1_422_450, 1_422_451, 99, "public");
    let f: Vec<_> = fields(&code_body).map(Result::unwrap).collect();
    assert_eq!(f[2], (3, Wire::Varint(99)));
    assert_eq!(f[3], (4, Wire::Bytes(b"public")));
}

#[test]
fn servers_and_codes_read_back() {
    let one = Writer::new()
        .string(1, "CDN")
        .varint(3, 127)
        .string(8, "fastly.cdn.steampipe.steamcontent.com")
        .string(9, "fastly.cdn.steampipe.steamcontent.com")
        .string(12, "mandatory")
        .finish();
    let only = Writer::new().string(1, "SteamCache").string(8, "10.0.0.1").varint(13, 7).finish();
    let list = servers(&Writer::new().bytes(1, &one).bytes(1, &only).finish()).unwrap();
    assert_eq!(
        list[0],
        ContentServer {
            kind: "CDN".into(),
            host: "fastly.cdn.steampipe.steamcontent.com".into(),
            vhost: "fastly.cdn.steampipe.steamcontent.com".into(),
            https: "mandatory".into(),
            allowed_apps: vec![],
        }
    );
    assert_eq!(list[1].allowed_apps, [7]);
    assert_eq!(request_code(&Writer::new().varint(1, 1234).finish()), Ok(Some(1234)));
    assert_eq!(request_code(&Writer::new().varint(1, 0).finish()), Ok(None));
}
