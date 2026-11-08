//! `NginxDir` against a real temporary directory.

mod support;

use std::fs;

use lanstash_adapter_fs::nginx_dir::NginxDir;
use lanstash_ports::nginx::NginxCacheSource;
use lanstash_ports::storage::FileRead;
use support::TempDir;

#[test]
fn nginx_dir_lists_level_files_only() {
    let d = TempDir::new("nginx");
    let name = "009e7e74e5f962fcbd151381e957007a";
    fs::create_dir_all(d.0.join("7a").join("00")).unwrap();
    fs::write(d.0.join("7a").join("00").join(name), b"data").unwrap();
    fs::write(d.0.join("7a").join("00").join("tmp.123"), b"temp file").unwrap();
    let src = NginxDir::new(&d.0);
    assert_eq!(src.list_files(&["7a".into(), "7b".into()]).unwrap(), vec![format!("7a/00/{name}")]);
    assert_eq!(src.open(&format!("7a/00/{name}")).unwrap().size().unwrap(), 4);
    assert!(src.open("../x").is_err());
    assert!(src.open("7a/00/../../etc").is_err());
    assert!(src.list_files(&["zz".into()]).is_err());
    assert!(src.exists(&format!("7a/00/{name}")).unwrap());
    assert!(!src.exists("7a/00/00000000000000000000000000000000").unwrap());
    assert!(!src.exists("ff/ff/00000000000000000000000000000000").unwrap());
    assert!(src.exists("7a/00/tmp.123").is_err());
    assert!(src.exists("7a/00/../../etc").is_err());
}
