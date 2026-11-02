use crate::core::notify::{Refresh, announce};
use anyhow::{Result, ensure};
use clash_verge_draft::DraftTransaction;
use serde::Serialize;
use smartstring::alias::String;

use crate::{
    config::{
        Config,
        dns::{DnsOverrideScope, ProfileDnsSettings, dns_config_path, dns_override_source},
        profiles::PROFILE_WRITE_LOCK,
    },
    constants::files::DNS_CONFIG,
    core::CoreManager,
    utils::dirs,
};

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DnsOverrideOutcome {
    Applied,
    ConfirmationRequired { source: String },
}

pub async fn qualify_global_dns(
    mut config: serde_yaml_ng::Mapping,
    profile_uid: &str,
) -> Result<serde_yaml_ng::Mapping> {
    use serde_yaml_ng::Value;
    let catalog = crate::enhance::application_profiles::catalog(profile_uid).await?;
    let proxies = catalog.into_iter().map(|item| (item.name, item.value)).collect();
    let mut profiles = (**Config::profiles().await.data_arc()).clone();
    profiles.current = Some(profile_uid.into());
    let source = profiles.current_mapping().await?;
    let providers = source
        .get("rule-providers")
        .and_then(Value::as_mapping)
        .into_iter()
        .flat_map(|entries| entries.keys())
        .filter_map(Value::as_str)
        .map(|name| {
            (
                name.to_owned(),
                crate::enhance::application_profiles::alias(profile_uid, name),
            )
        })
        .collect();
    let dns = if config.contains_key("dns") {
        config.get_mut("dns").and_then(Value::as_mapping_mut)
    } else {
        Some(&mut config)
    };
    if let Some(dns) = dns {
        crate::enhance::application_profiles::namespace_dns(dns, &proxies, &providers);
    }
    Ok(config)
}

pub async fn set_dns_override(
    profile_uid: String,
    enabled: bool,
    confirmation: Option<String>,
) -> Result<DnsOverrideOutcome> {
    let _profile_write = PROFILE_WRITE_LOCK.lock().await;
    let _config_write = Config::lock_config_write().await;
    let profiles = Config::profiles().await.data_arc();
    ensure!(
        profiles.current.as_ref() == Some(&profile_uid),
        "The active profile changed; retry the DNS setting"
    );
    let scope = Config::verge().await.latest_arc().dns_scope();
    crate::config::dns::migrate_profile_dns_configs().await?;
    let source = if scope == DnsOverrideScope::Global {
        None
    } else {
        dns_override_source(
            profiles.current.as_deref().unwrap_or_default(),
            &profiles.current_mapping().await?,
        )?
    };
    if enabled && let Some(source) = source.as_ref().filter(|source| Some(*source) != confirmation.as_ref()) {
        return Ok(DnsOverrideOutcome::ConfirmationRequired { source: source.clone() });
    }

    if enabled {
        let app_dir = dirs::app_home_dir()?;
        let path = dns_config_path(&app_dir, scope, &profile_uid)?;
        let mut content = if path.exists() {
            tokio::fs::read_to_string(&path).await?
        } else {
            let content = tokio::fs::read_to_string(app_dir.join(DNS_CONFIG)).await?;
            tokio::fs::write(&path, &content).await?;
            content
        };
        let document = serde_yaml_ng::from_str::<serde_yaml_ng::Mapping>(&content)?;
        if scope == DnsOverrideScope::Global {
            content = crate::utils::yaml_emitter::to_mihomo_config_string(
                &qualify_global_dns(document, &profile_uid).await?,
            )?;
            tokio::fs::write(&path, &content).await?;
        }
    }

    let verge = Config::verge().await;
    let transaction = DraftTransaction::begin(vec![&verge])?;
    verge.edit_draft(|draft| {
        if scope == DnsOverrideScope::Global {
            draft.global_dns_settings = Some(enabled);
        } else {
            draft.profile_dns_settings.insert(
                profile_uid.clone(),
                ProfileDnsSettings {
                    enabled,
                    confirmation: if enabled { source } else { None },
                },
            );
        }
    });
    CoreManager::global().update_config_checked().await?;

    let state = Config::runtime().await.data_arc().dns_override.clone();
    let mut outcome = DnsOverrideOutcome::Applied;
    // A subscription update can replace the file while Mihomo is validating it.
    if scope == DnsOverrideScope::Profile
        && enabled
        && let Some(state) = state.filter(|state| !state.enabled)
    {
        verge.edit_draft(|draft| {
            draft
                .profile_dns_settings
                .insert(profile_uid, ProfileDnsSettings::default());
        });
        if let Some(source) = state.source {
            outcome = DnsOverrideOutcome::ConfirmationRequired { source };
        }
    }

    transaction.commit();
    // Core and memory state remain applied even when persistence fails.
    let result = verge.data_arc().save_file().await;
    announce(Refresh::Verge);
    announce(Refresh::Clash);
    result?;
    Ok(outcome)
}
