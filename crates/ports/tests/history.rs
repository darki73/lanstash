use lanstash_ports::history::{Change, SeenAll};

#[test]
fn changes_by_name() {
    for c in [Change::Added, Change::NewBuild, Change::Current] {
        assert_eq!(Change::parse(c.as_str()), Some(c));
    }
    assert_eq!(
        [Change::Added, Change::NewBuild, Change::Current].map(Change::as_str),
        ["added", "new-build", "current"]
    );
    assert_eq!(Change::parse("gone"), None);
}

#[test]
fn nothing_seen_at_first() {
    assert!(SeenAll::default().games.is_empty() && SeenAll::default().platforms.is_empty());
}
