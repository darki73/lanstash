//! Tests for `lanstash_adapter_http::github`: the newest release's tag, through a fake web.

use std::sync::{Arc, Mutex};

use lanstash_adapter_http::github::{API_HOST, GitHubReleases};
use lanstash_ports::releases::Releases;
use lanstash_ports::web::{Web, WebError, WebRequest, WebResponse};

/// One request as the fake saw it.
struct Asked {
    host: String,
    port: u16,
    target: String,
    headers: Vec<(String, String)>,
    /// Over HTTPS, trusted by the public roots.
    public_tls: bool,
}

/// Answers every request with one response, and keeps what was asked.
struct Canned {
    answer: Result<WebResponse, WebError>,
    asked: Mutex<Vec<Asked>>,
}

impl Canned {
    fn new(status: u16, body: &str) -> Arc<Self> {
        Arc::new(Self {
            answer: Ok(WebResponse { status, body: body.as_bytes().to_vec() }),
            asked: Mutex::new(Vec::new()),
        })
    }
}

impl Web for Canned {
    fn send(&self, req: &WebRequest<'_>) -> Result<WebResponse, WebError> {
        self.asked.lock().unwrap().push(Asked {
            host: req.host.into(),
            port: req.port,
            target: req.target.into(),
            headers: req.headers.iter().map(|(n, v)| ((*n).to_owned(), (*v).to_owned())).collect(),
            public_tls: req.tls.is_some_and(|t| t.pin.is_none() && !t.unchecked),
        });
        self.answer.clone()
    }
}

fn releases(web: &Arc<Canned>) -> GitHubReleases {
    GitHubReleases::new(Arc::clone(web) as Arc<dyn Web>, "owner/lanstash", "lanstash/0.1.0")
}

#[test]
fn the_newest_release_is_its_tag_without_the_v() {
    let web = Canned::new(200, r#"{"tag_name":"v0.2.0","name":"lanstash 0.2.0","draft":false}"#);
    assert_eq!(releases(&web).latest(), Ok(Some("0.2.0".into())));
    let asked = web.asked.lock().unwrap();
    let a = &asked[0];
    assert_eq!((a.host.as_str(), a.port, a.target.as_str()), (API_HOST, 443, "/repos/owner/lanstash/releases/latest"));
    assert!(a.public_tls, "HTTPS, trusted by the public roots");
    assert!(a.headers.contains(&("User-Agent".into(), "lanstash/0.1.0".into())));
}

#[test]
fn no_release_yet_is_none_and_anything_else_is_an_error() {
    assert_eq!(releases(&Canned::new(404, r#"{"message":"Not Found"}"#)).latest(), Ok(None));
    assert!(releases(&Canned::new(403, "rate limited")).latest().unwrap_err().contains("403"));
    assert!(releases(&Canned::new(200, r#"{"name":"x"}"#)).latest().unwrap_err().contains("tag_name"));
    assert!(releases(&Canned::new(200, "not json")).latest().is_err());
    let down = Arc::new(Canned { answer: Err(WebError::from("connect: refused")), asked: Mutex::new(Vec::new()) });
    assert!(releases(&down).latest().unwrap_err().contains("refused"));
}
