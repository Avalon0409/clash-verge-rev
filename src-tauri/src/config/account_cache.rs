use crate::utils::{dirs, help};
use anyhow::{Context as _, Result, ensure};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest as _, Sha256};
use std::{
    fs::{self, File},
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
};

static CACHE_LOCK: Mutex<()> = Mutex::new(());
const FORMAT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct CachedAccount<T> {
    version: u32,
    data: T,
}

fn cache_path(root: &Path, id: &str) -> PathBuf {
    let digest = Sha256::digest(id.as_bytes());
    let digest = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    root.join(format!("{digest}.json"))
}

pub(crate) fn read<T: DeserializeOwned>(id: &str) -> Result<Option<T>> {
    let _cache = CACHE_LOCK.lock();
    read_at(&dirs::app_home_dir()?.join("account-cache"), id)
}

fn read_at<T: DeserializeOwned>(root: &Path, id: &str) -> Result<Option<T>> {
    let path = cache_path(root, id);
    if !path.try_exists()? {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    open_private_file(&path)?.read_to_end(&mut bytes)?;
    let account: CachedAccount<T> =
        serde_json::from_slice(&bytes).map_err(|_| anyhow::anyhow!("Local account cache is invalid; sign in again"))?;
    ensure!(
        account.version == FORMAT_VERSION,
        "Unsupported local account cache version"
    );
    Ok(Some(account.data))
}

pub(crate) fn write<T: Serialize>(id: &str, data: &T) -> Result<()> {
    let _cache = CACHE_LOCK.lock();
    write_at(&dirs::app_home_dir()?.join("account-cache"), id, data)
}

fn write_at<T: Serialize>(root: &Path, id: &str, data: &T) -> Result<()> {
    create_private_directory(root)?;
    let bytes = serde_json::to_vec(&CachedAccount {
        version: FORMAT_VERSION,
        data,
    })
    .context("Unable to encode local account cache")?;
    write_private(&cache_path(root, id), &bytes)
}

pub(crate) fn delete(id: &str) -> Result<()> {
    let _cache = CACHE_LOCK.lock();
    let path = cache_path(&dirs::app_home_dir()?.join("account-cache"), id);
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("Unable to remove local account cache"),
    }
}

pub(super) fn create_private_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    ensure!(
        fs::symlink_metadata(path)?.is_dir(),
        "Local cache directory is not an ordinary directory"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[cfg(windows)]
fn open_private_file(path: &Path) -> Result<File> {
    crate::core::owner_identity::open_private_current_user_file(path)
}

#[cfg(unix)]
fn open_private_file(path: &Path) -> Result<File> {
    use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.uid() == unsafe { libc::geteuid() } && metadata.nlink() == 1,
        "Local account cache is not an ordinary current-user file"
    );
    ensure!(
        metadata.mode() & 0o077 == 0,
        "Local account cache permissions must be private"
    );
    Ok(file)
}

pub(super) fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = path.with_extension(format!("{}.tmp", help::get_uid("cache")));
    #[cfg(windows)]
    let mut file = crate::core::owner_identity::create_private_current_user_file(&temporary)?;
    #[cfg(unix)]
    let mut file = {
        use std::os::unix::fs::OpenOptionsExt as _;
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?
    };
    let temporary = scopeguard::guard(temporary, |path| {
        let _ = fs::remove_file(path);
    });
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temporary.as_path(), path).context("Unable to publish local cache file")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_cache_survives_reopening_and_replaces_complete_records() -> Result<()> {
        let root = std::env::temp_dir().join(help::get_uid("account-cache-"));
        write_at(&root, "account/../../outside", &serde_json::json!({"session": "first"}))?;
        write_at(
            &root,
            "account/../../outside",
            &serde_json::json!({"session": "second"}),
        )?;
        let record: serde_json::Value = read_at(&root, "account/../../outside")?.context("missing cache")?;
        assert_eq!(record["session"], "second");
        assert_eq!(fs::read_dir(&root)?.count(), 1);
        assert_eq!(
            cache_path(&root, "account/../../outside").parent(),
            Some(root.as_path())
        );
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn invalid_cache_is_preserved_and_missing_cache_is_explicit() -> Result<()> {
        let root = std::env::temp_dir().join(help::get_uid("account-cache-"));
        create_private_directory(&root)?;
        assert!(read_at::<serde_json::Value>(&root, "missing")?.is_none());
        let path = cache_path(&root, "invalid");
        write_private(&path, b"{\"version\":99,\"data\":{}}")?;
        assert!(read_at::<serde_json::Value>(&root, "invalid").is_err());
        assert_eq!(fs::read(&path)?, b"{\"version\":99,\"data\":{}}");
        fs::remove_dir_all(root)?;
        Ok(())
    }
}
