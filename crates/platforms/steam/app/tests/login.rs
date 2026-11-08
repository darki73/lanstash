//! Tests for `lanstash_steam_app::login`, against a scripted Steam.

mod support;

use lanstash_ports::secrets::Secrets;
use lanstash_steam_app::connection::Connection;
use lanstash_steam_app::login::{ACCOUNT, ACCOUNTS, Login, REFRESH_TOKEN, accounts, forget, login, token, token_name};
use lanstash_steam_domain::cm::auth::method;
use lanstash_steam_domain::cm::message::Message;
use lanstash_steam_domain::cm::proto::{Writer, fields};
use support::{Answers, Fixed, Memory, Script, answer, steam};

/// A 512-bit odd modulus: any will do, the scripted Steam does not decrypt.
fn modulus_hex() -> String {
    "c3".repeat(64)
}

fn key(m: &Message) -> Vec<u8> {
    answer(m, 1, Writer::new().string(1, &modulus_hex()).string(2, "010001").varint(3, 777).finish())
}

fn session(confirmations: &[u64]) -> Vec<u8> {
    let mut w = Writer::new().varint(1, 5).bytes(2, b"req");
    for c in confirmations {
        w = w.bytes(4, &Writer::new().varint(1, *c).finish());
    }
    w.varint(5, 76_561_197_960_287_930).finish()
}

fn tokens() -> Vec<u8> {
    Writer::new().string(3, "REFRESH").string(4, "ACCESS").string(6, "someone").finish()
}

fn field(body: &[u8], n: u32) -> Option<Vec<u8>> {
    fields(body).filter_map(Result::ok).find(|(f, _)| *f == n).and_then(|(_, v)| v.bytes()).map(<[u8]>::to_vec)
}

/// A Steam that wants `confirmations`, refuses the first `bad_codes` codes,
/// and approves after `pending_polls` empty polls.
fn script(confirmations: &'static [u64], mut bad_codes: u32, mut pending_polls: u32) -> Script {
    Box::new(move |m| {
        let name = m.header.target_job_name.clone().unwrap_or_default();
        match name.as_str() {
            method::GET_PASSWORD_RSA_PUBLIC_KEY => vec![key(m)],
            method::BEGIN_AUTH_SESSION_VIA_CREDENTIALS => vec![answer(m, 1, session(confirmations))],
            method::UPDATE_AUTH_SESSION_WITH_STEAM_GUARD_CODE if bad_codes > 0 => {
                bad_codes -= 1;
                vec![answer(m, 65, vec![])]
            }
            method::UPDATE_AUTH_SESSION_WITH_STEAM_GUARD_CODE => vec![answer(m, 1, vec![])],
            method::POLL_AUTH_SESSION_STATUS if pending_polls > 0 => {
                pending_polls -= 1;
                vec![answer(m, 1, vec![])]
            }
            method::POLL_AUTH_SESSION_STATUS => vec![answer(m, 1, tokens())],
            _ => Vec::new(),
        }
    })
}

#[test]
fn an_email_code_logs_in_and_keeps_the_token() {
    let (link, sent) = steam(script(&[2], 1, 0));
    let mut c = Connection::over(link);
    let prompt = Answers::new(&["  someone ", "hunter2", "wrong", "ab12c"]);
    let secrets = Memory::default();
    let with = Login { prompt: &prompt, random: &Fixed, secrets: &secrets, device: "lanstash" };
    assert_eq!(login(&mut c, &with).unwrap(), "someone");
    assert_eq!(token(&secrets, "someone").unwrap().as_deref(), Some("REFRESH"));
    assert_eq!(accounts(&secrets).unwrap(), ["someone"]);
    let asked = prompt.asked.lock().unwrap();
    assert_eq!(asked[1], ("Password".to_string(), true), "the password is not echoed");
    assert!(asked[2].0.contains("email"));
    assert_eq!(*prompt.told.lock().unwrap(), ["Steam did not accept that code; try again."]);
    let sent = sent.lock().unwrap();
    let begin = sent
        .iter()
        .find(|m| m.header.target_job_name.as_deref() == Some(method::BEGIN_AUTH_SESSION_VIA_CREDENTIALS))
        .unwrap();
    assert_eq!(field(&begin.body, 2).unwrap(), b"someone");
    let encrypted = field(&begin.body, 3).unwrap();
    assert_eq!(encrypted.len(), 88, "base64 of a 64-byte ciphertext");
    assert!(!String::from_utf8_lossy(&encrypted).contains("hunter2"));
    let code = sent
        .iter()
        .rev()
        .find(|m| m.header.target_job_name.as_deref() == Some(method::UPDATE_AUTH_SESSION_WITH_STEAM_GUARD_CODE))
        .unwrap();
    assert_eq!(field(&code.body, 3).unwrap(), b"AB12C", "codes are sent upper case");
}

#[test]
fn an_approval_in_the_app_is_waited_for() {
    let (link, _) = steam(script(&[4], 0, 1));
    let mut c = Connection::over(link);
    let prompt = Answers::new(&["someone", "pw", ""]);
    let secrets = Memory::default();
    let with = Login { prompt: &prompt, random: &Fixed, secrets: &secrets, device: "lanstash" };
    login(&mut c, &with).unwrap();
    assert_eq!(
        prompt.told.lock().unwrap()[0],
        "Approve this sign-in in the Steam mobile app (or the email from Steam)."
    );
    assert!(prompt.asked.lock().unwrap()[2].0.starts_with("Not approved yet"));
    assert!(token(&secrets, "someone").unwrap().is_some());
}

#[test]
fn enter_instead_of_a_code_waits_for_the_tap() {
    let (link, sent) = steam(script(&[3, 4], 0, 0));
    let mut c = Connection::over(link);
    let prompt = Answers::new(&["someone", "pw", ""]);
    let secrets = Memory::default();
    let with = Login { prompt: &prompt, random: &Fixed, secrets: &secrets, device: "lanstash" };
    login(&mut c, &with).unwrap();
    assert!(prompt.asked.lock().unwrap()[2].0.contains("or approve the sign-in there"));
    let no_code = sent
        .lock()
        .unwrap()
        .iter()
        .all(|m| m.header.target_job_name.as_deref() != Some(method::UPDATE_AUTH_SESSION_WITH_STEAM_GUARD_CODE));
    assert!(no_code);
}

#[test]
fn a_wrong_password_is_said_in_words_and_nothing_is_kept() {
    let (link, _) = steam(Box::new(|m| match m.header.target_job_name.as_deref() {
        Some(method::GET_PASSWORD_RSA_PUBLIC_KEY) => vec![key(m)],
        _ => vec![answer(m, 5, vec![])],
    }));
    let mut c = Connection::over(link);
    let prompt = Answers::new(&["someone", "nope"]);
    let secrets = Memory::default();
    let with = Login { prompt: &prompt, random: &Fixed, secrets: &secrets, device: "lanstash" };
    assert_eq!(login(&mut c, &with).unwrap_err(), "Steam: wrong account name or password (5)");
    assert!(secrets.0.lock().unwrap().is_empty());
}

#[test]
fn the_one_account_kept_before_moves_to_the_list() {
    let secrets = Memory::default();
    secrets.save(REFRESH_TOKEN, "x\n").unwrap();
    secrets.save(ACCOUNT, "Some_One\n").unwrap();
    assert_eq!(accounts(&secrets).unwrap(), ["Some_One"]);
    assert_eq!(token(&secrets, "some_one").unwrap().as_deref(), Some("x"), "any case");
    assert!(secrets.load(REFRESH_TOKEN).unwrap().is_none() && secrets.load(ACCOUNT).unwrap().is_none());
    assert_eq!(accounts(&secrets).unwrap(), ["Some_One"], "once");
}

#[test]
fn several_accounts_are_kept_and_forgotten_one_by_one() {
    let secrets = Memory::default();
    let tokens = |name: &str, token: &str| lanstash_steam_domain::cm::auth::Tokens {
        refresh_token: token.into(),
        access_token: String::new(),
        account_name: name.into(),
    };
    lanstash_steam_app::login::keep(&secrets, &tokens("player_one", "A"), "").unwrap();
    lanstash_steam_app::login::keep(&secrets, &tokens("player_two", "B"), "").unwrap();
    lanstash_steam_app::login::keep(&secrets, &tokens("PLAYER_ONE", "C"), "").unwrap();
    assert_eq!(accounts(&secrets).unwrap(), ["player_one", "player_two"], "a sign-in again renews, never doubles");
    assert_eq!(token(&secrets, "player_one").unwrap().as_deref(), Some("C"));
    assert_eq!(token(&secrets, "player_two").unwrap().as_deref(), Some("B"));
    assert!(forget(&secrets, "nobody").is_err());
    forget(&secrets, "Player_One").unwrap();
    assert_eq!(accounts(&secrets).unwrap(), ["player_two"]);
    assert!(token(&secrets, "player_one").unwrap().is_none());
    forget(&secrets, "player_two").unwrap();
    assert!(secrets.0.lock().unwrap().is_empty(), "nothing left behind");
    assert!(lanstash_steam_app::login::keep(&secrets, &tokens("", "D"), "").is_err(), "no name, no account");
}

#[test]
fn a_tokens_secret_is_named_by_its_account() {
    assert_eq!(token_name("Some_One"), "steam-token-some-one");
    assert_eq!(token_name("a"), "steam-token-a");
    assert_eq!(token_name("a.b"), "steam-token-x612e62", "anything else in hex");
    assert_eq!(ACCOUNTS, "steam-accounts");
}
