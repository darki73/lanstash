//! Tests for `lanstash_ports::web`: the trait works as an object; an untrusted
//! certificate's fingerprint survives being passed on as text.

use lanstash_ports::web::{Tls, Web, WebError, WebRequest, WebResponse, untrusted_in};

struct Echo;

impl Web for Echo {
    fn send(&self, req: &WebRequest<'_>) -> Result<WebResponse, WebError> {
        if req.tls.is_some_and(|t| t.pin.is_none()) {
            return Err(WebError { message: "10.0.0.1: not trusted".into(), untrusted: Some("AB:CD".into()) });
        }
        Ok(WebResponse { status: 200, body: [req.method.as_bytes(), b" ", req.body].concat() })
    }
}

#[test]
fn a_request_with_a_body_gets_a_whole_response() {
    let web: &dyn Web = &Echo;
    let mut req =
        WebRequest { method: "POST", host: "10.0.0.1", port: 3000, target: "/x", headers: &[], body: b"{}", tls: None };
    assert_eq!(web.send(&req).unwrap(), WebResponse { status: 200, body: b"POST {}".to_vec() });
    req.tls = Some(Tls { pin: Some("AB:CD"), unchecked: false });
    assert!(web.send(&req).is_ok(), "pinned");
}

#[test]
fn an_untrusted_certificate_is_named_in_the_words_and_read_back() {
    let tls = Some(Tls { pin: None, unchecked: false });
    let req = WebRequest { method: "GET", host: "h", port: 443, target: "/", headers: &[], body: b"", tls };
    let e = Echo.send(&req).unwrap_err();
    let words = format!("AdGuard Home at h: {e}");
    assert_eq!(words, "AdGuard Home at h: 10.0.0.1: not trusted (untrusted certificate, SHA-256 AB:CD)");
    assert_eq!(untrusted_in(&words), Some("AB:CD"));
    assert_eq!(untrusted_in("refused"), None);
    assert_eq!(WebError::from("x").to_string(), "x");
}
