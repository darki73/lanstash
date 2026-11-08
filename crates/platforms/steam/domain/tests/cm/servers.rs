//! Tests for `lanstash_steam_domain::cm::servers`.

use lanstash_steam_domain::cm::servers::websockets;

#[test]
fn only_websocket_servers_in_order() {
    let answer = br#"{"response":{"serverlist":[
        {"endpoint":"cmp1-fra1.steamserver.net:27018","legacy_endpoint":"x:1","type":"websockets","dc":"fra1"},
        {"endpoint":"203.0.113.40:27017","type":"netfilter"},
        {"endpoint":"ext1-dxb1.steamserver.net:443","type":"websockets"},
        {"endpoint":"no-port","type":"websockets"}
    ],"success":true}}"#;
    assert_eq!(
        websockets(answer).unwrap(),
        [("cmp1-fra1.steamserver.net".to_string(), 27018), ("ext1-dxb1.steamserver.net".to_string(), 443)]
    );
    assert!(websockets(b"{}").is_err());
    assert!(websockets(b"not json").is_err());
}
