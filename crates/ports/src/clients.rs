//! The clients that download through the cache: what their addresses are called
//! on the network (a reverse DNS lookup), and the names given to them.

use alloc::string::String;
use alloc::vec::Vec;
use core::net::IpAddr;

/// What an address is called on the network.
pub trait ReverseNames: Send + Sync {
    /// The host names of `ip` (none: the network does not know it), and how long (seconds)
    /// the answer may be kept.
    ///
    /// # Errors
    /// No server answered, or the answer was not understood.
    fn names_of(&self, ip: IpAddr) -> Result<(Vec<String>, u32), String>;
}

/// The names given to clients, kept beyond a restart.
pub trait ClientLabels: Send + Sync {
    /// Every client given a name: (address, name).
    ///
    /// # Errors
    /// They could not be read.
    fn labels(&self) -> Result<Vec<(String, String)>, String>;

    /// Names the client at `address` `label`; `None` takes its name away.
    ///
    /// # Errors
    /// It could not be kept.
    fn set_label(&self, address: &str, label: Option<&str>) -> Result<(), String>;
}
