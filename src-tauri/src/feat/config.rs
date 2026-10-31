use crate::config::{Config, IVerge};
use anyhow::Result;
use clash_verge_draft::SharedDraft;
use serde_yaml_ng::Mapping;
use tokio::sync::MutexGuard;

pub async fn update_application_proxy(change: crate::config::application_proxy::ApplicationProxyChange) -> Result<()> {
    let config_write = Config::try_lock_config_write()?;
    let verge = Config::verge().await.data_arc();
    let mut settings = verge.application_proxy.clone().unwrap_or_default();
    anyhow::ensure!(
        settings.enabled
            || matches!(
                &change,
                crate::config::application_proxy::ApplicationProxyChange::SetMode { .. }
            ),
        "enable application routing before changing application settings"
    );
    settings.apply_change(change)?;
    let enable_tun_mode = settings.enabled.then_some(true);
    let patch = IVerge {
        application_proxy: Some(settings),
        enable_tun_mode,
        ..IVerge::default()
    };
    drop(verge);
    apply_verge_patch_locked(&config_write, &patch, false).await?;
    drop(config_write);
    if enable_tun_mode.is_some() {
        super::reconcile_tun_availability().await;
    }
    Ok(())
}

pub async fn patch_clash(patch: &Mapping) -> Result<()> {
    let config_write = Config::try_lock_config_write()?;
    if let Some(mode) = patch.get("mode").and_then(serde_yaml_ng::Value::as_str) {
        ensure_application_proxy_mode(mode)?;
    }
    super::executor::apply(
        &config_write,
        super::executor::Patch::Clash(patch),
        super::effects::clash_effects(patch),
    )
    .await
}

/// Apply a patch, then reconcile TUN when its setting changes.
///
/// TUN patches do not always produce a Run State transition, so reconciliation is explicit.
pub async fn patch_verge(patch: &IVerge, not_save_file: bool) -> Result<()> {
    apply_verge_patch(patch, not_save_file).await?;
    if patch.enable_tun_mode.is_some()
        || patch
            .application_proxy
            .as_ref()
            .is_some_and(|settings| settings.enabled)
    {
        super::reconcile_tun_availability().await;
    }
    Ok(())
}

/// Apply a patch without post-update reconciliation.
pub(super) async fn apply_verge_patch(patch: &IVerge, not_save_file: bool) -> Result<()> {
    let config_write = Config::try_lock_config_write()?;
    apply_verge_patch_locked(&config_write, patch, not_save_file).await
}

/// Apply a patch with the shared configuration write lock already held.
/// Callers must pass the guard returned by [`Config::lock_config_write`].
pub(crate) async fn apply_verge_patch_locked(
    _config_write: &MutexGuard<'_, ()>,
    patch: &IVerge,
    not_save_file: bool,
) -> Result<()> {
    let current = Config::verge().await.data_arc();
    let patch = link_application_tun(patch, current.application_proxy.as_ref());
    drop(current);
    if let Some(settings) = &patch.application_proxy {
        settings.validate()?;
    }
    super::executor::apply(
        _config_write,
        super::executor::Patch::Verge {
            patch: &patch,
            persist: !not_save_file,
        },
        super::effects::verge_effects(&patch),
    )
    .await
}

fn ensure_application_proxy_mode(mode: &str) -> Result<()> {
    anyhow::ensure!(matches!(mode, "rule" | "global" | "direct"), "invalid proxy mode");
    Ok(())
}

pub async fn fetch_verge_config() -> Result<SharedDraft<IVerge>> {
    let draft = Config::verge().await;
    let data = draft.data_arc();
    Ok(data)
}

fn link_application_tun(
    patch: &IVerge,
    current: Option<&crate::config::application_proxy::ApplicationProxyConfig>,
) -> IVerge {
    let mut linked = patch.clone();
    if linked
        .application_proxy
        .as_ref()
        .is_some_and(|settings| settings.enabled)
    {
        linked.enable_tun_mode = Some(true);
    } else if linked.enable_tun_mode == Some(false)
        && linked.application_proxy.is_none()
        && let Some(settings) = current.filter(|settings| settings.enabled)
    {
        let mut settings = settings.clone();
        settings.enabled = false;
        linked.application_proxy = Some(settings);
    }
    linked
}

#[cfg(test)]
mod application_tun_tests {
    use super::*;
    use crate::config::application_proxy::ApplicationProxyConfig;

    #[test]
    fn application_switch_and_external_tun_switch_remain_consistent() {
        let enabled = ApplicationProxyConfig {
            enabled: true,
            applications: vec![],
        };
        let opening = link_application_tun(
            &IVerge {
                application_proxy: Some(enabled.clone()),
                ..IVerge::default()
            },
            None,
        );
        assert_eq!(opening.enable_tun_mode, Some(true));
        let closing = link_application_tun(
            &IVerge {
                application_proxy: Some(ApplicationProxyConfig::default()),
                ..IVerge::default()
            },
            Some(&enabled),
        );
        assert_eq!(closing.enable_tun_mode, None);
        let tun_off = link_application_tun(
            &IVerge {
                enable_tun_mode: Some(false),
                ..IVerge::default()
            },
            Some(&enabled),
        );
        assert!(tun_off.application_proxy.is_some_and(|settings| !settings.enabled));
        assert_eq!(tun_off.enable_tun_mode, Some(false));
    }
}
