//! Tests for `lanstash_adapter_zerocopy::sendfile`: a file's bytes to a socket on
//! loopback (on Linux by `sendfile(2)`; elsewhere it says it cannot).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use lanstash_adapter_zerocopy::sendfile::{Sent, send_file};

/// A file of `n` bytes (each its position's low byte), and its path's directory to remove.
fn file_of(name: &str, n: usize) -> (std::fs::File, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("lanstash-zerocopy-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("data");
    let bytes: Vec<u8> = (0..n).map(|i| i as u8).collect();
    std::fs::File::create(&path).unwrap().write_all(&bytes).unwrap();
    (std::fs::File::open(&path).unwrap(), dir)
}

/// A connected pair: what is sent on the first is read on the second.
fn pair() -> (TcpStream, TcpStream) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let out = TcpStream::connect(l.local_addr().unwrap()).unwrap();
    let (back, _) = l.accept().unwrap();
    (out, back)
}

#[test]
fn a_range_of_the_file_reaches_the_socket_and_the_files_position_stays() {
    let (file, dir) = file_of("range", 300_000);
    let (out, mut back) = pair();
    let reader = std::thread::spawn(move || {
        let mut got = Vec::new();
        back.read_to_end(&mut got).unwrap();
        got
    });
    let sent = send_file(&out, &file, 1_000, 200_000).unwrap();
    drop(out);
    let got = reader.join().unwrap();
    if cfg!(target_os = "linux") {
        assert_eq!(sent, Sent::All);
        let want: Vec<u8> = (1_000..201_000).map(|i: usize| i as u8).collect();
        assert_eq!(got, want);
        use std::io::Seek;
        assert_eq!((&file).stream_position().unwrap(), 0, "the offset is passed, the position untouched");
    } else {
        assert_eq!((sent, got.len()), (Sent::Unsupported, 0), "elsewhere: nothing sent, send it another way");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn past_the_end_of_the_file_is_an_error() {
    let (file, dir) = file_of("short", 100);
    let (out, _back) = pair();
    let r = send_file(&out, &file, 50, 100);
    if cfg!(target_os = "linux") {
        assert_eq!(r.unwrap_err().kind(), std::io::ErrorKind::UnexpectedEof);
    } else {
        assert_eq!(r.unwrap(), Sent::Unsupported);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
