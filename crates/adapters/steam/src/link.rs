//! The ports this adapter serves: a [`Wss`] is a [`Link`], and [`WssDialer`]
//! opens them (and fetches over HTTPS) for the Steam app.

use std::time::Duration;

use lanstash_ports::link::{Dialer, Link};

use crate::wss::{Wss, https_get};

impl Link for Wss {
    fn send(&mut self, message: &[u8]) -> Result<(), String> {
        Wss::send(self, message)
    }

    fn recv(&mut self) -> Result<Option<Vec<u8>>, String> {
        Wss::recv(self)
    }
}

/// Opens WebSockets over TLS; every connect and read waits at most `timeout`.
#[derive(Debug, Clone, Copy)]
pub struct WssDialer {
    pub timeout: Duration,
}

impl Dialer for WssDialer {
    fn dial(&self, host: &str, port: u16, path: &str) -> Result<Box<dyn Link>, String> {
        Ok(Box::new(Wss::connect(host, port, path, self.timeout)?))
    }

    fn get(&self, host: &str, path: &str) -> Result<Vec<u8>, String> {
        https_get(host, path, self.timeout)
    }
}
