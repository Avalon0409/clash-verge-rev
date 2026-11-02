use super::{CmdResult, CommandFailure, WithErrorCode as _, proxy_aware_coded_error, proxy_aware_error};
use crate::config::dns::{DnsOverrideScope, dns_config_path};
use crate::core::notify::NoticeStatus;
use crate::feat;
use crate::utils::{dirs, yaml_emitter};
use crate::{
    cmd::StringifyErr as _,
    config::{ClashInfo, Config, profiles::profiles_save_file_safe},
    constants,
    core::{
        CoreManager, handle,
        validate::{CoreConfigValidator, ValidationOutcome},
    },
};
use clash_verge_logging::{Type, logging, logging_error};
use serde_yaml_ng::Mapping;
use smartstring::alias::String;

async fn selected_dns_config_path(
    profile_uid: Option<String>,
    scope: Option<DnsOverrideScope>,
) -> CmdResult<(std::path::PathBuf, String, DnsOverrideScope)> {
    let profiles = Config::profiles().await.data_arc();
    let uid = profile_uid.or_else(|| profiles.current.clone()).unwrap_or_default();
    let scope = match scope {
        Some(scope) => scope,
        None => Config::verge().await.latest_arc().dns_scope(),
    };
    if scope == DnsOverrideScope::Profile {
        let item = profiles.get_item(&uid).stringify_err()?;
        if !matches!(item.itype.as_deref(), Some("remote" | "local")) {
            return Err("Select a subscription for DNS settings".into());
        }
    }
    let app_dir = dirs::app_home_dir().stringify_err()?;
    Ok((dns_config_path(&app_dir, scope, &uid).stringify_err()?, uid, scope))
}

#[tauri::command]
pub async fn copy_clash_env() -> CmdResult {
    feat::copy_clash_env().await;
    Ok(())
}

#[tauri::command]
pub async fn get_clash_info() -> CmdResult<ClashInfo> {
    Ok(Config::clash().await.data_arc().get_client_info())
}

#[tauri::command]
pub async fn patch_clash_config(payload: Mapping) -> CmdResult {
    feat::patch_clash(&payload)
        .await
        .map_err(|error| proxy_aware_coded_error(&error, "CLASH_CONFIG_UPDATE_FAILED"))
}

#[tauri::command]
pub async fn patch_clash_mode(payload: String) -> CmdResult {
    feat::change_clash_mode(payload)
        .await
        .with_error_code("CLASH_MODE_UPDATE_FAILED")
}

/// Reads the saved mode without depending on strict mihomo `/configs` deserialization.
#[tauri::command]
pub async fn get_clash_mode() -> CmdResult<Option<String>> {
    Ok(Config::clash().await.data_arc().get_mode().map(Into::into))
}

#[tauri::command]
pub async fn change_clash_core(clash_core: String) -> CmdResult<Option<CommandFailure>> {
    logging!(info, Type::Config, "changing core to {clash_core}");

    match CoreManager::global().change_core(&clash_core).await {
        Ok(_) => {
            logging_error!(Type::Core, profiles_save_file_safe().await);

            match CoreManager::global().restart_core().await {
                Ok(_) => {
                    handle::Handle::notice(NoticeStatus::ConfigCoreChangeSuccess, clash_core.as_str());
                    handle::Handle::refresh_clash();
                    Ok(None)
                }
                Err(err) => {
                    let failed = err.context("core changed but failed to restart");
                    let error_msg: String = format!("{failed:#}").into();
                    handle::Handle::notice(NoticeStatus::ConfigCoreChangeError, error_msg.as_str());
                    logging!(error, Type::Core, "core changed but failed to restart: {error_msg}");
                    Ok(Some(proxy_aware_coded_error(&failed, "CORE_CHANGE_FAILED")))
                }
            }
        }
        Err(err) => {
            let error_msg: String = format!("{err:#}").into();
            logging!(error, Type::Core, "failed to change core: {error_msg}");
            handle::Handle::notice(NoticeStatus::ConfigCoreChangeError, error_msg.as_str());
            Ok(Some(proxy_aware_coded_error(&err, "CORE_CHANGE_FAILED")))
        }
    }
}

#[tauri::command]
pub async fn restart_core() -> CmdResult {
    logging_error!(Type::Core, profiles_save_file_safe().await);
    let result = CoreManager::global()
        .restart_core()
        .await
        .map_err(|error| proxy_aware_coded_error(&error, "CORE_RESTART_FAILED"));
    if result.is_ok() {
        handle::Handle::refresh_clash();
    }
    result
}

/// Replaces the managed core binary atomically instead of letting mihomo overwrite itself.
#[tauri::command]
pub async fn upgrade_clash_core(force: bool) -> CmdResult<feat::CoreUpgradeReport> {
    let report = feat::upgrade_core(force).await.with_error_code("CORE_UPGRADE_FAILED")?;
    if report.upgraded {
        handle::Handle::refresh_clash();
    }
    Ok(report)
}

#[tauri::command]
pub async fn use_bundled_clash_core() -> CmdResult {
    let core = Config::verge().await.latest_arc().get_valid_clash_core();
    let path = tauri::utils::platform::current_exe()
        .stringify_err()?
        .with_file_name(format!("{core}{}", std::env::consts::EXE_SUFFIX));
    crate::core::service::stage_approved_core(&path).with_error_code("BUNDLED_CORE_INSTALL_FAILED")?;
    if Config::verge().await.latest_arc().get_valid_clash_core() != core {
        return Err("The selected core changed; retry applying the bundled core".into());
    }
    CoreManager::global()
        .restart_core()
        .await
        .with_error_code("BUNDLED_CORE_RESTART_FAILED")?;
    handle::Handle::refresh_clash();
    Ok(())
}

#[tauri::command]
pub async fn test_delay(url: String) -> CmdResult<u32> {
    let result = match feat::test_delay(url).await {
        Ok(delay) => delay,
        Err(e) => {
            logging!(error, Type::Cmd, "get clash delay failed: {e:#}");
            10000u32
        }
    };
    Ok(result)
}

#[tauri::command]
pub async fn save_dns_config(
    mut dns_config: Mapping,
    profile_uid: Option<String>,
    scope: Option<DnsOverrideScope>,
) -> CmdResult {
    use tokio::fs;

    let _config_write = Config::lock_config_write().await;
    let (dns_path, uid, scope) = selected_dns_config_path(profile_uid, scope).await?;
    crate::config::dns::migrate_profile_dns_configs()
        .await
        .stringify_err()?;

    if scope == DnsOverrideScope::Global {
        dns_config = crate::feat::qualify_global_dns(dns_config, &uid)
            .await
            .stringify_err()?;
    }
    let yaml_str = yaml_emitter::to_mihomo_config_string(&dns_config).stringify_err()?;
    fs::write(&dns_path, yaml_str).await.stringify_err_log(|error| {
        logging!(error, Type::Config, "Failed to save DNS settings: {error:#}");
    })?;
    logging!(info, Type::Config, "DNS config saved to {dns_path:?}");

    Ok(())
}

#[tauri::command]
pub fn take_dns_override_notice() -> bool {
    crate::config::dns::take_dns_override_notice()
}

#[tauri::command]
pub async fn set_dns_override(
    profile_uid: String,
    enabled: bool,
    confirmation: Option<String>,
) -> CmdResult<feat::DnsOverrideOutcome> {
    feat::set_dns_override(profile_uid, enabled, confirmation)
        .await
        .map_err(|error| proxy_aware_coded_error(&error, "DNS_OVERRIDE_UPDATE_FAILED"))
}

#[tauri::command]
pub async fn apply_dns_config(apply: bool) -> CmdResult {
    logging!(
        info,
        Type::Config,
        "Regenerating scoped DNS configuration, enabled={apply}"
    );
    CoreManager::global()
        .update_config_checked()
        .await
        .map_err(|error| proxy_aware_error(&error))?;

    logging_error!(Type::Config, Config::sync_dns_override().await);
    handle::Handle::refresh_clash();
    Ok(())
}

#[tauri::command]
pub async fn get_dns_config_content(
    profile_uid: Option<String>,
    scope: Option<DnsOverrideScope>,
) -> CmdResult<Option<String>> {
    use crate::utils::dirs;
    use tokio::fs;

    let (mut dns_path, uid, scope) = selected_dns_config_path(profile_uid, scope).await?;
    if scope == DnsOverrideScope::Profile && !dns_path.exists() {
        if Config::verge()
            .await
            .latest_arc()
            .profile_dns_settings
            .get(&uid)
            .is_some_and(|setting| setting.enabled)
        {
            dns_path = dirs::app_home_dir().stringify_err()?.join(constants::files::DNS_CONFIG);
        } else {
            let mut profiles = (**Config::profiles().await.data_arc()).clone();
            profiles.current = Some(uid);
            let mut source = profiles.current_mapping().await.stringify_err()?;
            source.retain(|key, _| matches!(key.as_str(), Some("dns" | "hosts")));
            return Ok(Some(
                yaml_emitter::to_mihomo_config_string(&source).stringify_err()?.into(),
            ));
        }
    }

    match fs::read_to_string(&dns_path).await {
        Ok(content) => Ok(Some(content.into())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(CommandFailure::plain(error)),
    }
}

#[tauri::command]
pub async fn validate_dns_config(
    profile_uid: Option<String>,
    scope: Option<DnsOverrideScope>,
) -> CmdResult<ValidationOutcome> {
    let (dns_path, _, _) = selected_dns_config_path(profile_uid, scope).await?;
    let dns_path_str = dns_path.to_str().unwrap_or_default();

    if !dns_path.exists() {
        return Ok(ValidationOutcome::invalid_from_message("DNS config file not found"));
    }

    // A fragment, not a runnable config; the merged result is validated on apply.
    CoreConfigValidator::validate_config_file_outcome(dns_path_str, Some(true))
        .await
        .stringify_err()
}

#[tauri::command]
pub async fn get_clash_logs() -> CmdResult<Vec<std::string::String>> {
    let logs = CoreManager::global().get_clash_logs().await.unwrap_or_default();
    Ok(logs)
}
