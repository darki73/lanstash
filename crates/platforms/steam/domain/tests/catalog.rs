//! Tests for `lanstash_steam_domain::catalog`.

use lanstash_domain::json::{self, Value};
use lanstash_steam_domain::apps::{App, Depot, Target};
use lanstash_steam_domain::catalog::{offered, render};

fn depot(id: u32, os: &str, download: u64) -> Depot {
    Depot {
        id,
        manifest: Some(1),
        size: download * 2,
        download,
        empty: false,
        oslist: if os.is_empty() { vec![] } else { vec![os.into()] },
        language: None,
        arch: None,
        from_app: None,
        dlc: None,
    }
}

fn app(id: u32, name: &str, kind: &str, depots: Vec<Depot>) -> App {
    App {
        id,
        name: Some(name.into()),
        kind: Some(kind.into()),
        build: Some(9),
        depots,
        parent: None,
        dlcs: Vec::new(),
        art: Default::default(),
    }
}

#[test]
fn games_with_something_to_install_are_offered_by_name() {
    let apps = [
        app(2, "zeta", "Game", vec![depot(3, "", 100), depot(4, "windows", 50)]),
        app(5, "Alpha \"A\"", "game", vec![depot(6, "windows", 7)]),
        app(7, "A tool", "Tool", vec![depot(8, "", 1)]),
        app(9, "Mac only", "Game", vec![depot(10, "macos", 1)]),
        app(11, "No depots", "Game", vec![]),
        app(12, "Old", "Game", vec![depot(13, "", 0)]),
    ];
    let t = Target::default();
    assert!(offered(&apps[0], &t) && !offered(&apps[2], &t) && !offered(&apps[3], &t) && !offered(&apps[4], &t));
    let owners = |a: &lanstash_steam_domain::apps::App| match a.id {
        12 => vec!["player_one".to_string(), "player_two".to_string()],
        _ => vec![],
    };
    let text = render(&apps, &t, &|a| a.download(&t), &|a| (a.id == 5).then_some((3, 7)), &owners);
    let v = json::parse(&text).unwrap();
    let names: Vec<&str> =
        v.get("products").unwrap().as_array().iter().filter_map(|p| p.get("name").and_then(Value::as_str)).collect();
    assert_eq!(names, ["Alpha \"A\"", "Old", "zeta"]);
    assert!(text.contains("\"download\":7,\"lancache\":{\"have\":3,\"want\":7}}"), "{text}");
    assert!(
        text.contains(
            "{\"code\":\"12\",\"name\":\"Old\",\"game\":\"12\",\"version\":\"9\",\"known\":true,\"owners\":[\"player_one\",\"player_two\"]}"
        ),
        "no size: none given; its owners"
    );
    assert!(text.contains(
        "{\"code\":\"2\",\"name\":\"zeta\",\"game\":\"2\",\"version\":\"9\",\"known\":true,\"download\":150}"
    ));
}

#[test]
fn without_a_library_the_view_says_why() {
    use lanstash_steam_domain::catalog::render_none;
    assert_eq!(render_none("Steam: \"busy\""), "{\"products\":[],\"error\":\"Steam: \\\"busy\\\"\"}\n");
}

#[test]
fn a_game_for_several_systems_lists_them() {
    let apps = [
        app(2, "Both", "Game", vec![depot(3, "", 100), depot(4, "windows", 50), depot(5, "linux", 50)]),
        app(6, "Any", "Game", vec![depot(7, "", 100)]),
    ];
    let t = Target::default();
    assert_eq!(apps[0].systems(&t), ["windows", "linux"]);
    assert!(apps[1].systems(&t).is_empty(), "a depot naming no system says nothing");
    let text = render(&apps, &t, &|_| 0, &|_| None, &|_| Vec::new());
    assert!(text.contains("\"known\":true,\"systems\":[\"windows\",\"linux\"]}"), "{text}");
    assert!(
        text.contains("{\"code\":\"6\",\"name\":\"Any\",\"game\":\"6\",\"version\":\"9\",\"known\":true}"),
        "{text}"
    );
}
