//! Tests for `lanstash_epic_app::saved`.

use lanstash_epic::library::{Asset, Item};
use lanstash_epic_app::saved::{Saved, decode, encode};

#[test]
fn the_library_reads_back_as_it_was_kept() {
    let mut s = Saved { at_ms: 42, ..Saved::default() };
    let asset = Asset {
        app_name: "app".into(),
        namespace: "ns".into(),
        catalog_item_id: "c1".into(),
        build_version: "7".into(),
    };
    s.assets.push(("11111111111111111111111111111111".into(), vec![asset]));
    s.items.insert(
        "c1".into(),
        Item {
            title: "A \"Game\"".into(),
            game: true,
            images: vec![("DieselGameBox".into(), "https://x/y.jpg".into())],
        },
    );
    s.dirs.insert("a8e62719f55845c0a961ce64e3e05767".into(), "app".into());
    assert_eq!(decode(&encode(&s)), Some(s));
    let without_dirs = r#"{"at":1,"assets":[],"items":[]}"#;
    assert_eq!(decode(without_dirs).map(|d| d.dirs.len()), Some(0), "kept before directories were");
    assert_eq!(decode("{}"), None);
}
