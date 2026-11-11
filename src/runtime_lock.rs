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
    use std::os::unix::fs::OpenOptionsExt;

    match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .open(path)
    {
        Ok(file) => Ok(file),
        // A root installation can leave a 0600 lock file behind before the
        // service is switched to its hardened user. flock(2) only needs an
        // open descriptor, so read-only access is sufficient and safe here.
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => File::open(path),
        Err(error) => Err(error),
    }
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
}
