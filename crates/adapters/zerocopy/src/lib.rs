//! lanstash adapter: sending a stored file's bytes to a client's socket without
//! copying them through lanstash (`sendfile(2)` on Linux): the kernel moves them
//! from the file system's cache to the socket. The only `unsafe` in lanstash is
//! here, in one function ([`sendfile::send_file`]), behind a safe signature;
//! the architecture rules keep it in that one file.

#![deny(unsafe_code)]

pub mod sendfile;
