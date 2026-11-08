//! Tests for `lanstash_adapter_admin_api::api`'s setup paths: the file that
//! sets up a PC for a platform, to save.

use super::*;

#[derive(Debug)]
struct Epicish;

impl Platform for Epicish {
    fn id(&self) -> &str {
        "epicgames"
    }
    fn setup(&self, os: ClientOs) -> Option<SetupFile> {
        (os == ClientOs::Windows).then(|| SetupFile {
            name: "Epic Games (lanstash).cmd\"\r\nX: y".into(),
            content_type: "application/octet-stream",
            body: b"@echo off\r\n".to_vec(),
        })
    }
}

fn header<'a>(a: &'a Answer, name: &str) -> Option<&'a str> {
    a.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

#[test]
fn a_setup_file_is_served_to_save_under_its_name() {
    let a = api().with_platforms(vec![Arc::new(Epicish)]);
    let got = get(&a, "GET", "/api/v1/setup/epicgames/windows");
    assert_eq!(got.status, 200);
    assert_eq!(got.body, "@echo off\r\n");
    assert_eq!(header(&got, "Content-Type"), Some("application/octet-stream"));
    // Only safe characters of the name reach the header.
    assert_eq!(header(&got, "Content-Disposition"), Some("attachment; filename=\"Epic Games (lanstash).cmdX y\""));
    assert_eq!(header(&got, "X-Content-Type-Options"), Some("nosniff"));
}

#[test]
fn no_file_for_another_system_platform_or_path() {
    let a = api().with_platforms(vec![Arc::new(Epicish)]);
    for target in [
        "/api/v1/setup/epicgames/macos",
        "/api/v1/setup/epicgames/Windows",
        "/api/v1/setup/steam/windows",
        "/api/v1/setup/epicgames",
        "/api/v1/setup/epicgames/windows/x",
    ] {
        let got = get(&a, "GET", target);
        assert_eq!(got.status, 404, "{target}");
        assert!(got.body.contains("\"kind\":\"not-found\""), "{target}");
    }
}
