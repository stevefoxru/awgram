use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Keeps the OS-level lock alive for the full lifetime of the bot process.
pub struct BotInstanceLock {
    _file: File,
    path: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum BotInstanceLockError {
    #[error("другой экземпляр Telegram-бота уже использует этот токен (lock: {path})")]
    AlreadyRunning { path: PathBuf },
    #[error("не удалось открыть блокировку основного бота {path}: {source}")]
    Open {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("не удалось заблокировать Telegram-бот {path}: {source}")]
    Lock {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl BotInstanceLock {
    pub fn acquire(bot_token: &str) -> Result<Self, BotInstanceLockError> {
        Self::acquire_in(&runtime_directory(), bot_token)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn acquire_in(directory: &Path, bot_token: &str) -> Result<Self, BotInstanceLockError> {
        let token_hash = format!("{:x}", Sha256::digest(bot_token.as_bytes()));
        let path = directory.join(format!("awgram-bot-{}.lock", &token_hash[..24]));
        let file = open_private_lock_file(&path).map_err(|source| BotInstanceLockError::Open {
            path: path.clone(),
            source,
        })?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file, path }),
            Err(std::fs::TryLockError::WouldBlock) => {
                Err(BotInstanceLockError::AlreadyRunning { path })
            }
            Err(std::fs::TryLockError::Error(source)) => {
                Err(BotInstanceLockError::Lock { path, source })
            }
        }
    }
}

#[cfg(unix)]
fn runtime_directory() -> PathBuf {
    PathBuf::from("/tmp")
}

#[cfg(not(unix))]
fn runtime_directory() -> PathBuf {
    std::env::temp_dir()
}

#[cfg(unix)]
fn open_private_lock_file(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

    let opened = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        // The file is in the shared /tmp so root and the hardened service user
        // must both be able to open it. No token or application data is stored.
        .mode(0o644)
        // A privileged manual start must never follow an attacker-controlled
        // symlink in /tmp.
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path);
    let (file, writable) = match opened {
        Ok(file) => (file, true),
        // flock(2) works on a read-only descriptor. This path is expected when
        // root created the 0644 file before the hardened service starts.
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => (
            OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(path)?,
            false,
        ),
        Err(error) => return Err(error),
    };
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file() || metadata.nlink() != 1 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "lock path is not a single regular file",
        ));
    }
    if writable {
        use std::os::unix::fs::PermissionsExt as _;
        file.set_permissions(std::fs::Permissions::from_mode(0o644))?;
    }
    Ok(file)
}

#[cfg(not(unix))]
fn open_private_lock_file(path: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_token_cannot_be_locked_twice() {
        let directory = tempfile::tempdir().unwrap();
        let first = BotInstanceLock::acquire_in(directory.path(), "same-token").unwrap();
        let second = BotInstanceLock::acquire_in(directory.path(), "same-token");

        assert!(matches!(
            second,
            Err(BotInstanceLockError::AlreadyRunning { .. })
        ));
        drop(first);
        assert!(BotInstanceLock::acquire_in(directory.path(), "same-token").is_ok());
    }

    #[test]
    fn different_tokens_use_independent_locks_without_leaking_tokens() {
        let directory = tempfile::tempdir().unwrap();
        let first = BotInstanceLock::acquire_in(directory.path(), "secret-token-one").unwrap();
        let second = BotInstanceLock::acquire_in(directory.path(), "secret-token-two").unwrap();

        assert_ne!(first.path(), second.path());
        assert!(!first.path().to_string_lossy().contains("secret-token-one"));
        assert!(!second.path().to_string_lossy().contains("secret-token-two"));
    }

    #[cfg(unix)]
    #[test]
    fn lock_file_is_regular_and_readable_by_hardened_user() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let lock = BotInstanceLock::acquire_in(directory.path(), "permission-token").unwrap();
        let metadata = std::fs::symlink_metadata(lock.path()).unwrap();

        assert!(metadata.file_type().is_file());
        assert_eq!(metadata.permissions().mode() & 0o777, 0o644);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_cannot_be_used_as_lock_file() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let token = "symlink-token";
        let token_hash = format!("{:x}", Sha256::digest(token.as_bytes()));
        let path = directory
            .path()
            .join(format!("awgram-bot-{}.lock", &token_hash[..24]));
        let target = directory.path().join("target");
        std::fs::write(&target, b"must not be opened").unwrap();
        symlink(&target, &path).unwrap();

        assert!(BotInstanceLock::acquire_in(directory.path(), token).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"must not be opened");
    }
}
