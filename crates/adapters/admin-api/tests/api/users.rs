//! Tests for `api::users`: signing in and out, and lanstash's accounts.

use super::access::{cookie_of, send, with_accounts};
use super::*;

const PW: (&str, &str) = ("x-lanstash-password", "long+enough+pw");

#[test]
fn a_sign_in_gives_a_session_cookie_and_the_session_says_who() {
    let a = with_accounts();
    assert_eq!(
        send(&a, "GET", "/api/v1/session", &[], false).body,
        "{\"name\":null,\"role\":\"guest\",\"accounts\":true,\"admin\":true,\"permissions\":[],\"links\":[]}\n"
    );
    let ok = send(&a, "POST", "/api/v1/session", &[("x-lanstash-user", "boss"), PW], false);
    assert_eq!((ok.status, ok.body.as_str()), (200, "{\"name\":\"boss\",\"role\":\"admin\"}\n"));
    let set = &ok.headers.iter().find(|(n, _)| n == "Set-Cookie").unwrap().1;
    assert!(
        set.starts_with("lanstash_session=") && set.contains("HttpOnly") && set.contains("SameSite=Strict"),
        "{set}"
    );
    let cookie = set.split(';').next().unwrap();
    assert_eq!(
        send(&a, "GET", "/api/v1/session", &[("cookie", cookie)], false).body,
        "{\"name\":\"boss\",\"role\":\"admin\",\"accounts\":true,\"admin\":true,\"permissions\":[\"prefill\",\"prefill.own\",\"settings\",\"dns\",\"delete\",\"hide\",\"clients\",\"platforms\",\"accounts\"],\"links\":[]}\n"
    );
    assert!(send(&a, "GET", "/api/v1/session", &[], true).body.contains("\"role\":\"admin\""), "this machine");
}

#[test]
fn a_wrong_sign_in_is_refused_and_the_next_waits() {
    let a = with_accounts();
    let wrong =
        send(&a, "POST", "/api/v1/session", &[("x-lanstash-user", "boss"), ("x-lanstash-password", "nope")], false);
    assert_eq!(wrong.status, 401);
    let again = send(&a, "POST", "/api/v1/session", &[("x-lanstash-user", "boss"), PW], false);
    assert_eq!(again.status, 429);
    assert!(again.body.contains("\"retry_ms\":1000"), "{}", again.body);
    assert_eq!(send(&a, "POST", "/api/v1/session", &[("x-lanstash-user", "boss")], false).status, 400);
}

#[test]
fn signing_out_ends_the_session() {
    let a = with_accounts();
    let cookie = cookie_of(&a, "player.one");
    let out = send(&a, "DELETE", "/api/v1/session", &[("cookie", &cookie)], false);
    assert!(out.headers.iter().any(|(n, v)| n == "Set-Cookie" && v.contains("Max-Age=0")));
    assert!(send(&a, "GET", "/api/v1/session", &[("cookie", &cookie)], false).body.contains("guest"));
}

#[test]
fn an_admin_lists_adds_changes_and_removes_accounts() {
    let a = with_accounts();
    let admin = cookie_of(&a, "boss");
    let c = [("cookie", admin.as_str())];
    assert_eq!(
        send(&a, "GET", "/api/v1/users", &c, false).body,
        "{\"users\":[{\"name\":\"boss\",\"role\":\"admin\",\"links\":[]},{\"name\":\"player.one\",\"role\":\"viewer\",\"links\":[]}]}\n"
    );
    let add = send(&a, "POST", "/api/v1/users/second?role=admin", &[c[0], PW], false);
    assert_eq!(add.status, 200, "{}", add.body);
    assert_eq!(send(&a, "PUT", "/api/v1/users/player.one?role=admin", &c, false).status, 200);
    assert_eq!(send(&a, "PUT", "/api/v1/users/player.one?role=boss", &c, false).status, 400);
    assert_eq!(send(&a, "DELETE", "/api/v1/users/second", &c, false).status, 200);
    assert!(!send(&a, "GET", "/api/v1/users", &c, false).body.contains("second"));
    assert_eq!(send(&a, "POST", "/api/v1/users/x", &c, false).status, 400, "no password");
}

#[test]
fn a_viewer_changes_its_own_password_with_the_old_one_and_nothing_else() {
    let a = with_accounts();
    let viewer = cookie_of(&a, "player.one");
    let v = ("cookie", viewer.as_str());
    let new = ("x-lanstash-password", "a+new+long+password");
    assert_eq!(send(&a, "PUT", "/api/v1/users/player.one?role=admin", &[v], false).status, 403);
    assert_eq!(
        send(&a, "PUT", "/api/v1/users/boss", &[v, new, ("x-lanstash-old-password", "long+enough+pw")], false).status,
        403
    );
    assert_eq!(
        send(&a, "PUT", "/api/v1/users/player.one", &[v, new, ("x-lanstash-old-password", "wrong")], false).status,
        403
    );
    let ok =
        send(&a, "PUT", "/api/v1/users/player.one", &[v, new, ("x-lanstash-old-password", "long+enough+pw")], false);
    assert_eq!(ok.status, 200, "{}", ok.body);
    assert!(send(&a, "GET", "/api/v1/session", &[v], false).body.contains("guest"), "its sessions ended");
    assert_eq!(send(&a, "GET", "/api/v1/users", &[v], false).status, 401, "the old session is gone: a guest");
}

#[test]
fn without_accounts_the_session_says_admin_and_there_is_no_sign_in() {
    assert_eq!(get(&api(), "GET", "/api/v1/session").body, "{\"name\":null,\"role\":\"admin\",\"accounts\":false}\n");
    assert_eq!(get(&api(), "POST", "/api/v1/session").status, 404);
    assert_eq!(get(&api(), "GET", "/api/v1/users").status, 404);
}
