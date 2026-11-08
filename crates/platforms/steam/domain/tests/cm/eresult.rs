//! Tests for `lanstash_steam_domain::cm::eresult`.

use lanstash_steam_domain::cm::eresult::{
    EXPIRED, INVALID_PASSWORD, OK, RATE_LIMIT_EXCEEDED, REVOKED, SERVICE_UNAVAILABLE, TRY_ANOTHER_CM, describe,
    refuses_token,
};

#[test]
fn codes_read_as_words() {
    assert_eq!(describe(OK), "OK");
    assert_eq!(describe(INVALID_PASSWORD), "wrong account name or password");
    assert_eq!(describe(RATE_LIMIT_EXCEEDED), "too many attempts; wait a while");
    assert_eq!(describe(TRY_ANOTHER_CM), "try another server");
    assert_eq!(describe(12345), "an unexpected result");
}

#[test]
fn only_what_says_the_token_will_not_do_refuses_it() {
    for code in [INVALID_PASSWORD, REVOKED, EXPIRED, RATE_LIMIT_EXCEEDED] {
        assert!(refuses_token(code), "{code}");
    }
    for code in [OK, 2, 3, SERVICE_UNAVAILABLE, TRY_ANOTHER_CM, 12345] {
        assert!(!refuses_token(code), "{code}");
    }
}
