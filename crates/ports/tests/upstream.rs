use lanstash_domain::http::ResponseHeadIn;
use lanstash_ports::upstream::{Source, Upstream, UpstreamError, UpstreamRequest, UpstreamResponse};

struct Empty(ResponseHeadIn);

impl UpstreamResponse for Empty {
    fn head(&self) -> &ResponseHeadIn {
        &self.0
    }
    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, UpstreamError> {
        Ok(0)
    }
}

struct NotFound;

impl Upstream for NotFound {
    fn fetch(&self, req: &UpstreamRequest<'_>) -> Result<Box<dyn UpstreamResponse>, UpstreamError> {
        if req.host.is_empty() {
            return Err(UpstreamError::new("no host"));
        }
        Ok(Box::new(Empty(ResponseHeadIn { status: 404, minor_version: 1, headers: Vec::new() })))
    }
}

#[test]
fn a_request_gets_a_response_or_an_error() {
    let src = Source { host: "example.com".into(), port: None, target: "/x".into() };
    let req = UpstreamRequest { method: "GET", host: &src.host, port: src.port, target: &src.target, headers: &[] };
    let mut resp = NotFound.fetch(&req).unwrap();
    assert_eq!(resp.head().status, 404);
    assert_eq!(resp.read(&mut [0u8; 4]).unwrap(), 0);
    let e = NotFound.fetch(&UpstreamRequest { host: "", ..req }).err().unwrap();
    assert_eq!(e.to_string(), "upstream: no host");
}
