use std::{
    fs::{self, File, OpenOptions},
    path::Path,
    sync::Arc,
};

use anyhow::{Context, Result, bail};
use fs2::FileExt;
use grammers_session::storages::SqliteSession;

pub(crate) struct StoredSession {
    pub session: Arc<SqliteSession>,
    // Keep the OS lock alive for the entire lifetime of the connection.
    _lock: File,
}

impl StoredSession {
    pub async fn open(directory: &Path) -> Result<Self> {
        if directory.is_symlink() {
            bail!("The session directory must not be a symbolic link.");
        }
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(directory)
            .context("Cannot create the private session directory.")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if fs::metadata(directory)?.permissions().mode() & 0o077 != 0 {
                bail!(
                    "The session directory must be private (Unix permissions 700). Choose a new SLOPOGRAM_DATA_DIR or fix its permissions."
                );
            }
        }
        let lock = private_file(&directory.join("session.lock"))?;
        lock.try_lock_exclusive().context(
            "This session is already open. Close the other Slopogram window, then retry.",
        )?;
        let path = directory.join("telegram.sqlite");
        private_file(&path)?;
        let session =
            Arc::new(SqliteSession::open(&path).await.context(
                "Cannot open the saved session. The existing database has been preserved.",
            )?);
        Ok(Self {
            session,
            _lock: lock,
        })
    }
}

fn private_file(path: &Path) -> Result<File> {
    if path.is_symlink() {
        bail!("Session files must not be symbolic links.");
    }
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(path)
        .context("Cannot access a session file.")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use grammers_session::{Session, types::UpdateState};

    #[tokio::test]
    async fn session_reopens_with_auth_key_and_update_cursor() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("private");
        {
            let stored = StoredSession::open(&directory).await.unwrap();
            stored.session.set_home_dc_id(2).await.unwrap();
            let mut dc = stored.session.dc_option(2).unwrap().unwrap();
            dc.auth_key = Some([7; 256]);
            stored.session.set_dc_option(&dc).await.unwrap();
            stored
                .session
                .set_update_state(UpdateState::Primary {
                    pts: 42,
                    date: 100,
                    seq: 3,
                })
                .await
                .unwrap();
        }
        let stored = StoredSession::open(&directory).await.unwrap();
        assert_eq!(stored.session.home_dc_id().unwrap(), 2);
        assert_eq!(
            stored.session.dc_option(2).unwrap().unwrap().auth_key,
            Some([7; 256])
        );
        assert_eq!(stored.session.updates_state().await.unwrap().pts, 42);
    }

    #[tokio::test]
    async fn rejects_concurrent_session_use_and_releases_lock() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("private");
        let first = StoredSession::open(&directory).await.unwrap();
        assert!(StoredSession::open(&directory).await.is_err());
        drop(first);
        assert!(StoredSession::open(&directory).await.is_ok());
    }

    #[tokio::test]
    async fn corrupt_session_is_reported_without_erasing_it() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("private");
        let stored = StoredSession::open(&directory).await.unwrap();
        drop(stored);
        let path = directory.join("telegram.sqlite");
        let contents = b"invalid session database";
        fs::write(&path, contents).unwrap();
        assert!(StoredSession::open(&directory).await.is_err());
        assert_eq!(fs::read(path).unwrap(), contents);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn session_files_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("private");
        let _stored = StoredSession::open(&directory).await.unwrap();
        assert_eq!(
            fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(directory.join("telegram.sqlite"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
