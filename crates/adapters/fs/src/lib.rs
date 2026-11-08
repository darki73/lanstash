//! lanstash adapter for the file system: the storage ports over directories
//! and files ([`dir_fs`], [`fs_file`]), free space ([`disk_space`]),
//! lancache's nginx cache directory, read-only ([`nginx_dir`]), and secrets
//! and state kept as private files ([`private_dir`]).

#![forbid(unsafe_code)]

pub mod dir_fs;
pub mod disk_space;
pub mod fs_file;
pub mod nginx_dir;
pub mod private_dir;
