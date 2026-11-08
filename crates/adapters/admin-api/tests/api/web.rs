//! Tests for `lanstash_adapter_admin_api::api`'s web UI: its page for every path of its own, its
//! files as themselves, the built-in page without it.

use super::*;

/// A built web UI in a directory of its own (removed when dropped).
struct Built(std::path::PathBuf);

impl Built {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("lanstash-web-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        std::fs::write(dir.join("index.html"), "<!doctype html><title>ui</title>").unwrap();
        std::fs::write(dir.join("assets/app-1a2b.js"), "console.log(1)").unwrap();
        std::fs::write(dir.join("manifest.webmanifest"), "{}").unwrap();
        Self(dir)
    }
}

impl Drop for Built {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn header<'a>(a: &'a Answer, name: &str) -> Option<&'a str> {
    a.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

#[test]
fn every_path_of_the_page_answers_its_index_and_its_files_answer_themselves() {
    let built = Built::new("paths");
    let a = api().with_web(built.0.clone());
    for path in ["/", "/platforms/steam", "/overview/storage?x=1"] {
        let page = get(&a, "GET", path);
        assert_eq!((page.status, page.body.as_str()), (200, "<!doctype html><title>ui</title>"), "{path}");
        assert_eq!(header(&page, "cache-control"), Some("no-cache"), "{path}");
    }
    let js = get(&a, "GET", "/assets/app-1a2b.js");
    assert_eq!((js.status, header(&js, "content-type")), (200, Some("text/javascript; charset=utf-8")));
    assert_eq!(header(&js, "cache-control"), Some("public, max-age=31536000, immutable"));
    let manifest = get(&a, "GET", "/manifest.webmanifest");
    assert_eq!(
        (manifest.status, header(&manifest, "content-type"), header(&manifest, "cache-control")),
        (200, Some("application/manifest+json"), Some("public, max-age=3600")),
        "a file not hashed may change under its name"
    );
    for missing in ["/assets/none.js", "/assets/../index.html", "/.hidden.txt"] {
        assert_eq!(get(&a, "GET", missing).status, 404, "{missing}");
    }
    let api_path = get(&a, "GET", "/api/v1/nothing");
    assert_eq!((api_path.status, header(&api_path, "content-type")), (404, Some("application/json")));
    assert_eq!(get(&a, "GET", "/api/v1/version").status, 200, "the API stays the API");
}

#[test]
fn without_a_built_ui_the_built_in_page_is_served() {
    let a = api().with_web(std::env::temp_dir().join("lanstash-web-none-there"));
    let page = get(&a, "GET", "/");
    assert!(page.status == 200 && page.body.contains("<html"), "the built-in page");
    assert_eq!(get(&a, "GET", "/platforms/steam").status, 404);
}
