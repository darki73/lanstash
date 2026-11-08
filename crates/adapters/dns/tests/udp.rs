//! Tests for `lanstash_adapter_dns::udp`: one exchange with a server on loopback.

use std::net::UdpSocket;
use std::time::Duration;

use lanstash_adapter_dns::udp::exchange;

#[test]
fn datagrams_that_are_not_the_answer_are_skipped() {
    let server = UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();
    std::thread::spawn(move || {
        let mut buf = [0u8; 64];
        if let Ok((_, from)) = server.recv_from(&mut buf) {
            let _ = server.send_to(b"noise", from);
            let _ = server.send_to(b"answer", from);
        }
    });
    let got = exchange(addr, b"q", Duration::from_secs(2), |b| (b == b"answer").then_some(Ok(b.len())));
    assert_eq!(got, Ok(6));
}

#[test]
fn no_answer_in_time_is_an_error() {
    let silent = UdpSocket::bind("127.0.0.1:0").unwrap();
    let got: Result<(), String> =
        exchange(silent.local_addr().unwrap(), b"q", Duration::from_millis(100), |_| Some(Ok(())));
    let e = got.unwrap_err();
    assert!(e.ends_with("no answer within 100 ms"), "said in words, not as an OS error: {e}");
}
