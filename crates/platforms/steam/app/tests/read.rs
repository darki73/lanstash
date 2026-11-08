//! Tests for `lanstash_steam_app::read`: what a library is made of, read from a
//! scripted account: the depots its license's package lists, and the apps'
//! info.

mod support;

use lanstash_steam_app::read::read_raw;
use support::account::services;

#[test]
fn the_licenses_depots_and_apps_are_read() {
    let (s, _) = services(true);
    let (mut c, _) = s.session("someone").unwrap();
    let raw = read_raw(&mut c).unwrap();
    assert_eq!(raw.owned, [101, 102, 103], "the depots package 37 lists");
    assert_eq!(raw.apps.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [100], "the app it lists");
    assert!(String::from_utf8_lossy(&raw.apps[0].1).contains("Some Game"));
}
