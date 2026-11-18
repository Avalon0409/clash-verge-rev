use super::account_cache::{create_private_directory, write_private};
use crate::utils::{dirs, tmpl};
use anyhow::{Context as _, Result, ensure};
use clash_verge_logging::{Type, logging};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};
use std::{fs, path::Path};

const CONFIG_VERSION: u32 = 1;
const STATE_FILE: &str = "configuration-version.json";
const CONFIG_FILES: [&str; 4] = ["verge.yaml", "profiles.yaml", "config.yaml", "dns_config.yaml"];

#[derive(Deserialize, Serialize)]
struct MigrationState {
    schema_version: u32,
    app_version: std::string::String,
    account_cache_migrated: bool,
}

pub(crate) async fn prepare_configuration() -> Result<()> {
    let root = dirs::app_home_dir()?;
    let migration_root = root.clone();
    let legacy_accounts = tokio::task::spawn_blocking(move || prepare_at(&migration_root))
        .await
        .context("Configuration migration task failed")??;
    for id in legacy_accounts {
        if let Err(error) = crate::feat::freecat::migrate_legacy_secrets(id).await {
            logging!(warn, Type::Config, "Account migration requires one sign-in: {error}");
        }
    }
    let state = MigrationState {
        schema_version: CONFIG_VERSION,
        app_version: env!("CARGO_PKG_VERSION").into(),
        account_cache_migrated: true,
    };
    tokio::task::spawn_blocking(move || write_private(&root.join(STATE_FILE), &serde_json::to_vec(&state)?))
        .await
        .context("Unable to persist configuration migration")??;
    Ok(())
}

fn prepare_at(root: &Path) -> Result<Vec<std::string::String>> {
    let state = match fs::read(root.join(STATE_FILE)) {
        Ok(bytes) => Some(
            serde_json::from_slice::<MigrationState>(&bytes)
                .map_err(|_| anyhow::anyhow!("Invalid configuration version file"))?,
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error).context("Unable to read configuration version"),
    };
    if let Some(state) = &state {
        ensure!(
            state.schema_version <= CONFIG_VERSION,
            "Configuration requires a newer application version"
        );
    }
    let mut configs = Vec::new();
    for name in CONFIG_FILES {
        let path = root.join(name);
        let (bytes, restored) = match fs::read(&path) {
            Ok(bytes) => (Some(bytes), false),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                (restore_from_upgrade_backup(root, Path::new(name))?, true)
            }
            Err(error) => return Err(error).with_context(|| format!("Unable to inspect {name}")),
        };
        if let Some(bytes) = bytes {
            let mapping = serde_yaml_ng::from_slice::<Mapping>(&bytes).map_err(|error| {
                let location = error.location().map_or_else(std::string::String::new, |location| {
                    format!(" at line {}, column {}", location.line(), location.column())
                });
                anyhow::anyhow!("Invalid {name}{location}; existing configuration was preserved")
            })?;
            if restored {
                write_private(&path, &bytes)?;
            }
            configs.push((name, bytes, mapping));
        }
    }
    let needs_backup = state
        .as_ref()
        .is_none_or(|state| state.app_version != env!("CARGO_PKG_VERSION"));
    if needs_backup && !configs.is_empty() {
        let backup = root.join("backups/upgrades").join(env!("CARGO_PKG_VERSION"));
        create_private_directory(&backup)?;
        for (name, bytes, _) in &configs {
            let path = backup.join(name);
            if !path.try_exists()? {
                write_private(&path, bytes)?;
            }
        }
        let profiles = root.join("profiles");
        if profiles.is_dir() {
            backup_directory(&profiles, &backup.join("profiles"))?;
        }
    }
    let mut accounts = Vec::new();
    for (name, _, config) in &configs {
        if *name != "profiles.yaml" {
            continue;
        }
        for item in config.get("items").and_then(Value::as_sequence).into_iter().flatten() {
            if !state.as_ref().is_some_and(|state| state.account_cache_migrated)
                && let Some(id) = item
                    .get("account")
                    .and_then(|account| account.get("credential_id"))
                    .and_then(Value::as_str)
            {
                accounts.push(id.to_owned());
            }
            let Some(file) = item.get("file").and_then(Value::as_str) else {
                continue;
            };
            let relative = Path::new(file);
            super::IProfiles::validate_profile_file(file)?;
            let path = root.join("profiles").join(relative);
            if path.try_exists()? {
                continue;
            }
            if let Some(bytes) = restore_from_upgrade_backup(root, &Path::new("profiles").join(relative))? {
                create_private_directory(path.parent().context("Profile path has no parent")?)?;
                write_private(&path, &bytes)?;
                continue;
            }
            let template = match item.get("type").and_then(Value::as_str) {
                Some("script") => Some(tmpl::ITEM_SCRIPT),
                Some("merge") => Some(tmpl::ITEM_MERGE),
                Some("rules") => Some(tmpl::ITEM_RULES),
                Some("proxies") => Some(tmpl::ITEM_PROXIES),
                Some("groups") => Some(tmpl::ITEM_GROUPS),
                _ => None,
            };
            if let Some(template) = template {
                create_private_directory(path.parent().context("Profile path has no parent")?)?;
                write_private(&path, template.as_bytes())?;
            } else {
                logging!(
                    info,
                    Type::Config,
                    "Cached subscription {file} needs recovery during startup"
                );
            }
        }
    }
    Ok(accounts)
}

fn backup_directory(source: &Path, target: &Path) -> Result<()> {
    create_private_directory(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let destination = target.join(entry.file_name());
        if kind.is_dir() {
            backup_directory(&entry.path(), &destination)?;
        } else if kind.is_file() && !destination.try_exists()? {
            write_private(&destination, &fs::read(entry.path())?)?;
        }
    }
    Ok(())
}

fn restore_from_upgrade_backup(root: &Path, relative: &Path) -> Result<Option<Vec<u8>>> {
    let directory = root.join("backups/upgrades");
    if !directory.is_dir() {
        return Ok(None);
    }
    let mut candidates = fs::read_dir(directory)?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join(relative))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    candidates.sort_by_key(|path| fs::metadata(path).and_then(|metadata| metadata.modified()).ok());
    candidates
        .last()
        .map(fs::read)
        .transpose()
        .context("Unable to restore cached configuration")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_preserves_configuration_and_recovers_missing_profile_files() -> Result<()> {
        let root = std::env::temp_dir().join(crate::utils::help::get_uid("migration-"));
        fs::create_dir_all(root.join("profiles"))?;
        let original = b"clash_core: verge-mihomo\ncustom_option: keep-me\n";
        fs::write(root.join("verge.yaml"), original)?;
        fs::write(
            root.join("profiles.yaml"),
            b"items:\n  - type: remote\n    file: subscription.yaml\n  - type: script\n    file: custom.js\n",
        )?;
        fs::write(root.join("profiles/subscription.yaml"), b"proxies: []\n")?;
        prepare_at(&root)?;
        fs::remove_file(root.join("profiles/subscription.yaml"))?;
        fs::remove_file(root.join("verge.yaml"))?;
        prepare_at(&root)?;
        assert_eq!(fs::read(root.join("profiles/subscription.yaml"))?, b"proxies: []\n");
        assert_eq!(fs::read(root.join("verge.yaml"))?, original);
        assert_eq!(fs::read(root.join("profiles/custom.js"))?, tmpl::ITEM_SCRIPT.as_bytes());
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn invalid_legacy_configuration_is_not_replaced_with_defaults() -> Result<()> {
        let root = std::env::temp_dir().join(crate::utils::help::get_uid("migration-"));
        fs::create_dir_all(&root)?;
        fs::write(root.join("verge.yaml"), b"invalid: [\n")?;
        assert!(prepare_at(&root).is_err());
        assert_eq!(fs::read(root.join("verge.yaml"))?, b"invalid: [\n");
        assert!(!root.join(STATE_FILE).exists());
        fs::remove_dir_all(root)?;
        Ok(())
    }
}
