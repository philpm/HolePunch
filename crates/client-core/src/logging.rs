use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tokio::io::AsyncReadExt;
const LIMIT: u64 = 2 * 1024 * 1024;
pub struct RollingLog {
    path: PathBuf,
    file: File,
    size: u64,
}
fn open(path: &Path) -> std::io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    opts.open(path)
}
impl RollingLog {
    pub fn new(path: PathBuf) -> std::io::Result<Self> {
        let file = open(&path)?;
        // Older clients did not cap logs. Retain only the tail before rotating them.
        for candidate in [&path, &path.with_extension("log.1")] {
            if candidate.exists() && candidate.metadata()?.len() > LIMIT {
                let mut old = File::open(candidate)?;
                old.seek(SeekFrom::End(-(LIMIT as i64)))?;
                let mut tail = Vec::with_capacity(LIMIT as usize);
                old.read_to_end(&mut tail)?;
                drop(old);
                let mut capped = OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(candidate)?;
                capped.write_all(&tail)?;
            }
        }
        let size = file.metadata()?.len();
        Ok(Self { path, file, size })
    }
    pub fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        if self.size + bytes.len() as u64 > LIMIT {
            // Truncate in place after copying: portable on Windows where an open handle cannot be renamed.
            self.file.flush()?;
            std::fs::copy(&self.path, self.path.with_extension("log.1"))?;
            // Windows append-only handles cannot truncate a file. Keep append
            // semantics for writes and use a separate write handle for rotation.
            OpenOptions::new()
                .write(true)
                .open(&self.path)?
                .set_len(0)?;
            self.size = 0;
        }
        self.file.write_all(bytes)?;
        self.size += bytes.len() as u64;
        Ok(())
    }
}
pub fn pump(
    mut reader: impl tokio::io::AsyncRead + Unpin + Send + 'static,
    log: Arc<Mutex<RollingLog>>,
) {
    tokio::spawn(async move {
        let mut buf = [0; 4096];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let _ = log.lock().unwrap().write(&buf[..n]);
                }
            }
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_rotation() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("rathole.log");
        std::fs::write(&path, vec![b'o'; (LIMIT * 2) as usize]).unwrap();
        std::fs::write(
            path.with_extension("log.1"),
            vec![b'o'; (LIMIT * 2) as usize],
        )
        .unwrap();
        let mut l = RollingLog::new(path.clone()).unwrap();
        assert_eq!(path.metadata().unwrap().len(), LIMIT);
        assert_eq!(
            path.with_extension("log.1").metadata().unwrap().len(),
            LIMIT
        );
        for _ in 0..1200 {
            l.write(&[b'x'; 4096]).unwrap();
        }
        assert!(path.metadata().unwrap().len() <= LIMIT);
        assert!(path.with_extension("log.1").metadata().unwrap().len() <= LIMIT);
        assert!(std::fs::read(&path)
            .unwrap()
            .iter()
            .all(|byte| *byte == b'x'));
        assert!(std::fs::read(path.with_extension("log.1"))
            .unwrap()
            .iter()
            .all(|byte| *byte == b'x'));
    }
}
