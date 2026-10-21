//! Local administrative operations. Backups use SQLite's online backup API, including WAL.
use anyhow::{Context, Result};
use rusqlite::{backup::Backup, Connection, OpenFlags};
use std::{
    fs::{File, OpenOptions as FileOptions},
    path::{Path, PathBuf},
};
pub fn lock_directory(dir: &Path) -> Result<File> {
    std::fs::create_dir_all(dir)?;
    let file = FileOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join("center.lock"))?;
    fs2::FileExt::try_lock_exclusive(&file)
        .context("Center is already running in this data directory; stop it before restore")?;
    Ok(file)
}
fn checked(path: &Path) -> Result<Connection> {
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let integrity: String = db.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    anyhow::ensure!(integrity == "ok", "Database integrity check failed");
    let version: u32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    anyhow::ensure!(version <= 4, "Unsupported future schema {version}");
    for table in ["settings", "clients", "tunnels"] {
        db.prepare(&format!("SELECT * FROM {table} LIMIT 0"))?;
    }
    Ok(db)
}
pub fn backup(source: &Path, dest: &Path) -> Result<()> {
    let db = checked(source)?;
    let mut options = FileOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    drop(
        options
            .open(dest)
            .context("Backup destination must not already exist")?,
    );
    let result = (|| -> Result<()> {
        let mut target = Connection::open(dest)?;
        Backup::new(&db, &mut target)?.run_to_completion(
            128,
            std::time::Duration::from_millis(10),
            None,
        )?;
        drop(target);
        checked(dest)?;
        FileOptions::new()
            .write(true)
            .open(dest)?
            .sync_all()
            .context("Syncing completed backup")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(dest);
    }
    result
}
pub fn restore(dir: &Path, source: &Path) -> Result<PathBuf> {
    let _lock = lock_directory(dir)?;
    checked(source)?;
    let target = dir.join("holepunch.db");
    // Always preserve existing state; restore cannot silently discard an installation.
    if target.exists() {
        backup(
            &target,
            &dir.join(format!(
                "before-restore-{}.db",
                uuid::Uuid::new_v4().simple()
            )),
        )?;
    }
    let temp = dir.join(format!("restore-{}.db", uuid::Uuid::new_v4().simple()));
    backup(source, &temp)?;
    for suffix in ["-wal", "-shm"] {
        let p = dir.join(format!("holepunch.db{suffix}"));
        if p.exists() {
            std::fs::remove_file(p)?;
        }
    }
    std::fs::rename(&temp, &target)?;
    Ok(target)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backup_wal_restore_and_lock() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("source.db");
        let s = crate::store::Store::open(&src).unwrap();
        s.set("hello", "persisted").unwrap();
        let dest = d.path().join("backup.db");
        backup(&src, &dest).unwrap();
        assert!(backup(&src, &dest).is_err());
        let restore_dir = d.path().join("restored");
        let lock = lock_directory(&restore_dir).unwrap();
        assert!(restore(&restore_dir, &dest).is_err());
        drop(lock);
        restore(&restore_dir, &dest).unwrap();
        assert_eq!(
            crate::store::Store::open(&restore_dir.join("holepunch.db"))
                .unwrap()
                .get("hello")
                .unwrap()
                .as_deref(),
            Some("persisted")
        );
        let restored = crate::store::Store::open(&restore_dir.join("holepunch.db")).unwrap();
        restored.set("hello", "changed").unwrap();
        drop(restored);
        restore(&restore_dir, &dest).unwrap();
        let previous = std::fs::read_dir(&restore_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("before-restore-")
            })
            .unwrap();
        assert_eq!(
            crate::store::Store::open(&previous)
                .unwrap()
                .get("hello")
                .unwrap()
                .as_deref(),
            Some("changed")
        );
        assert_eq!(
            crate::store::Store::open(&restore_dir.join("holepunch.db"))
                .unwrap()
                .get("hello")
                .unwrap()
                .as_deref(),
            Some("persisted")
        );
    }
}
