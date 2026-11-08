use std::sync::{Arc, Mutex};

use lanstash_ports::storage::{DirEntry, FileRead, FileWrite, StoreError, StoreErrorKind, StoreFs};

#[derive(Default)]
struct Bytes(Mutex<Vec<u8>>);

impl FileRead for Bytes {
    fn size(&self) -> Result<u64, StoreError> {
        Ok(self.0.lock().unwrap().len() as u64)
    }
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), StoreError> {
        let d = self.0.lock().unwrap();
        let at = usize::try_from(offset).unwrap();
        let src = d.get(at..at + buf.len()).ok_or_else(|| StoreError::new(StoreErrorKind::UnexpectedEof, "eof"))?;
        buf.copy_from_slice(src);
        Ok(())
    }
}

impl FileWrite for Bytes {
    fn write_all_at(&self, offset: u64, data: &[u8]) -> Result<(), StoreError> {
        let mut d = self.0.lock().unwrap();
        let at = usize::try_from(offset).unwrap();
        if d.len() < at + data.len() {
            d.resize(at + data.len(), 0);
        }
        d[at..at + data.len()].copy_from_slice(data);
        Ok(())
    }
    fn set_len(&self, len: u64) -> Result<(), StoreError> {
        self.0.lock().unwrap().resize(usize::try_from(len).unwrap(), 0);
        Ok(())
    }
    fn sync(&self) -> Result<(), StoreError> {
        Ok(())
    }
}

struct OneFile(Arc<Bytes>);

impl StoreFs for OneFile {
    fn list(&self, _dir: &str) -> Result<Vec<DirEntry>, StoreError> {
        Ok(vec![DirEntry { name: "f".into(), is_dir: false }])
    }
    fn open_read(&self, _path: &str) -> Result<Arc<dyn FileRead>, StoreError> {
        Ok(self.0.clone())
    }
    fn create_new(&self, _path: &str) -> Result<Arc<dyn FileWrite>, StoreError> {
        Err(StoreError::new(StoreErrorKind::AlreadyExists, "f"))
    }
    fn open_write(&self, _path: &str) -> Result<Arc<dyn FileWrite>, StoreError> {
        Ok(self.0.clone())
    }
    fn rename(&self, _from: &str, _to: &str) -> Result<(), StoreError> {
        Ok(())
    }
    fn remove_file(&self, _path: &str) -> Result<(), StoreError> {
        Err(StoreError::other("read-only"))
    }
}

#[test]
fn errors_show_their_kind_and_message() {
    assert_eq!(StoreError::other("disk on fire").to_string(), "Other: disk on fire");
    assert_eq!(StoreError::new(StoreErrorKind::NotFound, "x").kind, StoreErrorKind::NotFound);
}

#[test]
fn shared_handles_read_and_write_through() {
    let file = Arc::new(Bytes::default());
    file.write_all_at(2, b"ab").unwrap();
    assert_eq!(FileRead::size(&file).unwrap(), 4);
    let mut buf = [0u8; 2];
    FileRead::read_exact_at(&file, 2, &mut buf).unwrap();
    assert_eq!(&buf, b"ab");
    file.set_len(1).unwrap();
    FileWrite::sync(&file).unwrap();
    assert_eq!(file.read_exact_at(0, &mut buf).unwrap_err().kind, StoreErrorKind::UnexpectedEof);
}

#[test]
fn defaults_read_immutable_files_like_others_and_know_no_space() {
    let fs = OneFile(Arc::new(Bytes::default()));
    fs.open_write("f").unwrap().write_all_at(0, b"xyz").unwrap();
    assert_eq!(fs.open_read_immutable("f").unwrap().size().unwrap(), 3);
    assert_eq!(fs.space(), Ok(None));
    assert!(fs.open_read("f").unwrap().os_file().is_none(), "no OS file under it by default");
}
