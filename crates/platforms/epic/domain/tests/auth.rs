//! Tests for `lanstash_epic::auth`.

use lanstash_epic::auth::{
    Client, LAUNCHER_CLIENT, basic, code_body, code_from, is_account_id, kept, login_url, parse_client, parse_tokens,
    percent, refresh_body, unkept,
};

const ID: &str = "0123456789abcdef0123456789abcdef";

#[test]
fn the_login_page_leads_to_epics_code_page_for_the_client() {
    assert_eq!(
        login_url("someclient"),
        "https://www.epicgames.com/id/login?redirectUrl=https%3A%2F%2Fwww.epicgames.com%2Fid%2Fapi%2Fredirect%3FclientId%3Dsomeclient%26responseType%3Dcode"
    );
    assert_eq!(percent("a b/~"), "a%20b%2F~");
}

#[test]
fn the_code_is_taken_from_what_was_pasted() {
    assert_eq!(code_from("  abc123DEF \n").as_deref(), Some("abc123DEF"));
    assert_eq!(code_from("\"abc123\"").as_deref(), Some("abc123"), "with its quotes");
    let page = r#"{"warning":"Do not share this code","redirectUrl":"https://localhost/launcher/authorized?code=abc","authorizationCode":"f00d1234","exchangeCode":null,"sid":null}"#;
    assert_eq!(code_from(page).as_deref(), Some("f00d1234"), "the whole page");
    assert_eq!(code_from(""), None);
    assert_eq!(code_from("abc&grant_type=x"), None, "only letters and digits");
    assert_eq!(code_from(r#"{"authorizationCode":null}"#), None);
}

#[test]
fn the_requests_name_the_grant_and_the_client() {
    assert_eq!(code_body("f00d"), "grant_type=authorization_code&code=f00d&token_type=eg1");
    assert_eq!(refresh_body("eg1~a.b+c"), "grant_type=refresh_token&refresh_token=eg1~a.b%2Bc&token_type=eg1");
    assert_eq!(basic(Client { id: "id", secret: "secret" }), "basic aWQ6c2VjcmV0");
}

#[test]
fn epics_answer_gives_the_tokens_and_the_account() {
    let body = format!(
        r#"{{"access_token":"eg1~A","expires_in":28800,"expires_at":"2026-11-08T00:00:00.000Z","token_type":"bearer","refresh_token":"eg1~R","refresh_expires":1987200,"account_id":"{}","client_id":"x","displayName":"Player One"}}"#,
        ID.to_uppercase()
    );
    let t = parse_tokens(&body).unwrap();
    assert_eq!((t.access_token.as_str(), t.expires_in, t.refresh_token.as_str()), ("eg1~A", 28_800, "eg1~R"));
    assert_eq!((t.refresh_expires_in, t.account_id.as_str(), t.display_name.as_str()), (1_987_200, ID, "Player One"));
    assert!(!format!("{t:?}").contains("eg1~"), "never printed");
}

#[test]
fn an_error_answer_says_why() {
    let refused = r#"{"errorCode":"errors.com.epicgames.account.oauth.authorization_code_not_found","errorMessage":"Sorry the authorization code you supplied was not found.","numericErrorCode":18059}"#;
    assert_eq!(parse_tokens(refused).unwrap_err(), "Sorry the authorization code you supplied was not found.");
    assert_eq!(parse_tokens(r#"{"errorCode":"x"}"#).unwrap_err(), "x");
    assert!(parse_tokens("<html>").is_err());
    let bad_id = r#"{"access_token":"a","expires_in":1,"refresh_token":"r","account_id":"../x"}"#;
    assert!(parse_tokens(bad_id).unwrap_err().contains("not 32 hex digits"));
    assert!(is_account_id(ID) && !is_account_id("x"));
}

#[test]
fn a_kept_sign_in_reads_back() {
    let text = kept("eg1~R", 1_234, "Player\nOne");
    assert_eq!(unkept(&text), Some(("eg1~R".into(), 1_234, "PlayerOne".into())));
    assert_eq!(unkept("eg1~R\nsoon\n"), None);
    assert_eq!(unkept(""), None);
}

#[test]
fn a_client_is_an_id_and_a_secret_and_the_launchers_is_built_in() {
    assert_eq!(parse_client(" ab12 : cd34 "), Some(("ab12".into(), "cd34".into())));
    assert_eq!(parse_client("ab12"), None);
    assert_eq!(parse_client("ab-12:cd34"), None, "letters and digits");
    assert_eq!(parse_client(":cd34"), None);
    let built_in = format!("{}:{}", LAUNCHER_CLIENT.id, LAUNCHER_CLIENT.secret);
    assert!(parse_client(&built_in).is_some(), "the built-in one is a client too");
}
