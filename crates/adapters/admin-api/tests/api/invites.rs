//! Tests for `api::invites`: an admin makes, lists and takes back invites; anyone with one makes an
//! account from it and is signed in.

use super::access::{cookie_of, send, with_accounts};
use super::*;

const PW: (&str, &str) = ("x-lanstash-password", "long+enough+pw");

/// The token of a new invite an admin made, with `query`.
fn make(a: &AdminApi, admin: &str, query: &str) -> String {
    let made = send(a, "POST", &format!("/api/v1/invites{query}"), &[("cookie", admin)], false);
    assert_eq!(made.status, 200, "{}", made.body);
    made.body.split("\"token\":\"").nth(1).unwrap().split('"').next().unwrap().to_string()
}

#[test]
fn only_an_admin_makes_lists_and_takes_back_invites() {
    let a = with_accounts();
    let admin = cookie_of(&a, "boss");
    let viewer = cookie_of(&a, "player.one");
    assert_eq!(send(&a, "POST", "/api/v1/invites?role=admin", &[], false).status, 401);
    assert_eq!(send(&a, "POST", "/api/v1/invites?role=admin", &[("cookie", &viewer)], false).status, 403);
    assert_eq!(send(&a, "GET", "/api/v1/invites", &[("cookie", &viewer)], false).status, 403);
    let token = make(&a, &admin, "?role=player&link=steam%2Fsomeone");
    let list = send(&a, "GET", "/api/v1/invites", &[("cookie", &admin)], false).body;
    assert!(list.contains("\"role\":\"player\",\"links\":[\"steam/someone\"]"), "{list}");
    assert!(!list.contains(&token), "the list never holds a token: {list}");
    let id = list.split("\"id\":\"").nth(1).unwrap().split('"').next().unwrap();
    assert_eq!(send(&a, "DELETE", &format!("/api/v1/invites/{id}"), &[("cookie", &viewer)], false).status, 403);
    assert_eq!(send(&a, "DELETE", &format!("/api/v1/invites/{id}"), &[("cookie", &admin)], false).status, 200);
    assert_eq!(send(&a, "GET", "/api/v1/invites", &[("cookie", &admin)], false).body, "{\"invites\":[]}\n");
    assert_eq!(send(&a, "DELETE", "/api/v1/invites/nothing", &[("cookie", &admin)], false).status, 404);
    let bad = send(&a, "POST", "/api/v1/invites?link=nonsense", &[("cookie", &admin)], false);
    assert_eq!(bad.status, 400, "{}", bad.body);
    assert_eq!(send(&a, "POST", "/api/v1/invites?role=boss", &[("cookie", &admin)], false).status, 400);
}

#[test]
fn an_invite_makes_an_account_once_and_signs_it_in() {
    let a = with_accounts();
    let admin = cookie_of(&a, "boss");
    let token = make(&a, &admin, "?role=player&link=steam%2Fsomeone");
    let path = format!("/api/v1/invites/{token}");
    let no_name = send(&a, "POST", &path, &[PW], false);
    assert_eq!(no_name.status, 400, "a name is needed");
    let taken = send(&a, "POST", &path, &[("x-lanstash-user", "boss"), PW], false);
    assert_eq!(taken.status, 400, "the name is taken: {}", taken.body);
    let ok = send(&a, "POST", &path, &[("x-lanstash-user", "newcomer"), PW], false);
    assert_eq!((ok.status, ok.body.as_str()), (200, "{\"name\":\"newcomer\",\"role\":\"player\"}\n"));
    let set = &ok.headers.iter().find(|(n, _)| n == "Set-Cookie").unwrap().1;
    let cookie = set.split(';').next().unwrap();
    let session = send(&a, "GET", "/api/v1/session", &[("cookie", cookie)], false).body;
    assert!(
        session.contains("\"name\":\"newcomer\"") && session.contains("\"links\":[\"steam/someone\"]"),
        "{session}"
    );
    let again = send(&a, "POST", &path, &[("x-lanstash-user", "another"), PW], false);
    assert_eq!(again.status, 404, "used once");
}

#[test]
fn an_invite_with_a_name_shows_it_to_its_holder_and_makes_only_that_account() {
    let a = with_accounts();
    let admin = cookie_of(&a, "boss");
    let token = make(&a, &admin, "?role=player&link=steam%2Fa&link=steam%2Fb&name=player.two");
    let path = format!("/api/v1/invites/{token}");
    let shown = send(&a, "GET", &path, &[], false);
    assert_eq!(
        (shown.status, shown.body.as_str()),
        (
            200,
            format!(
                "{{\"role\":\"player\",\"links\":[\"steam/a\",\"steam/b\"],\"until_ms\":{},\"name\":\"player.two\"}}\n",
                5 + 7 * 24 * 3_600_000
            )
            .as_str()
        )
    );
    let list = send(&a, "GET", "/api/v1/invites", &[("cookie", &admin)], false).body;
    assert!(list.contains("\"name\":\"player.two\""), "{list}");
    assert_eq!(send(&a, "POST", &path, &[("x-lanstash-user", "other"), PW], false).status, 400);
    assert_eq!(send(&a, "POST", &path, &[("x-lanstash-user", "player.two"), PW], false).status, 200);
    let taken = send(&a, "POST", "/api/v1/invites?name=boss", &[("cookie", &admin)], false);
    assert_eq!(taken.status, 400, "{}", taken.body);
}

#[test]
fn a_wrong_invite_makes_the_address_wait() {
    let a = with_accounts();
    let wrong = send(&a, "GET", "/api/v1/invites/0000", &[], false);
    assert_eq!(wrong.status, 404);
    let next = send(&a, "POST", "/api/v1/invites/0000", &[("x-lanstash-user", "newcomer"), PW], false);
    assert_eq!(next.status, 429);
    assert!(next.body.contains("\"retry_ms\":1000"), "{}", next.body);
    let sign_in = send(&a, "POST", "/api/v1/session", &[("x-lanstash-user", "boss"), PW], false);
    assert_eq!(sign_in.status, 429, "the same wait as a wrong sign-in");
}

#[test]
fn without_accounts_there_are_no_invites() {
    assert_eq!(get(&api(), "GET", "/api/v1/invites").status, 404);
    assert_eq!(get(&api(), "POST", "/api/v1/invites/abc").status, 404);
}
