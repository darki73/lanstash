use lanstash_ports::releases::{NoReleases, Releases};

#[test]
fn no_release_source_knows_no_release() {
    assert_eq!(NoReleases.latest(), Ok(None));
}
