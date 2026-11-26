//! Re-apply the system proxy when macOS gains or switches its primary network service.
//!
//! Proxy settings live per network service. A write that found no service wrote nothing, and a
//! switch to another service leaves the proxy on the old one; both need the write repeated.

use crate::{config::Config, core::CoreManager, core::proxy_control, process::AsyncHandler};
use clash_verge_logging::{Type, logging};
use parking_lot::Mutex;
use sysproxy::NetworkServiceMonitor;

static MONITOR: Mutex<Option<NetworkServiceMonitor>> = Mutex::new(None);

pub fn is_armed() -> bool {
    MONITOR.lock().is_some()
}

/// Subscribe to the primary IPv4 service on the main run loop. Armed on return, so a service
/// appearing from here on is never missed.
pub fn start() {
    if let Err(err) = watch_dns() {
        logging!(warn, Type::Config, "could not watch DNS changes: {err:#}");
    }
    match NetworkServiceMonitor::start(|| {
        AsyncHandler::spawn(reapply);
    }) {
        Ok(monitor) => {
            *MONITOR.lock() = Some(monitor);
            logging!(
                debug,
                Type::Core,
                "network watch armed; the system proxy is re-applied on network changes"
            );
        }
        Err(error) => logging!(
            warn,
            Type::Core,
            "could not watch the network; the proxy is not re-applied on changes: {error}"
        ),
    }
}

async fn reapply() {
    // Config lock first, then lifecycle: the repo's order. Holding both also means the config
    // read below is committed state, not a patch that may still roll back.
    let _config_write = Config::lock_config_write().await;
    let manager = CoreManager::global();
    let _life = manager.lifecycle_lock.lock().await;
    // Checked under the locks: a task that queued behind a restart may be stale by now.
    if !Config::verge()
        .await
        .latest_arc()
        .enable_system_proxy
        .unwrap_or_default()
    {
        return;
    }
    if !proxy_control::has_network_service().await {
        logging!(
            debug,
            Type::Core,
            "network changed without a service to write the proxy on; waiting for the next change"
        );
        return;
    }
    if let Err(error) = manager.apply_proxy_after_start().await {
        logging!(
            warn,
            Type::Core,
            "failed to re-apply the system proxy after a network change: {error:#}"
        );
    }
}

fn watch_dns() -> anyhow::Result<()> {
    use anyhow::{Context as _, ensure};
    use system_configuration::{
        core_foundation::{
            array::CFArray,
            runloop::{CFRunLoop, kCFRunLoopCommonModes},
            string::CFString,
        },
        dynamic_store::{SCDynamicStoreBuilder, SCDynamicStoreCallBackContext},
    };
    let store = SCDynamicStoreBuilder::new("Clash Verge DNS monitor")
        .callback_context(SCDynamicStoreCallBackContext {
            info: (),
            callout: |_, keys, _| {
                if keys
                    .iter()
                    .any(|key| !key.to_string().starts_with("State:/Network/Service/ClashVergeTunDNS-"))
                {
                    AsyncHandler::spawn(|| async {
                        if let Err(err) = refresh_dns().await {
                            logging!(warn, Type::Config, "refresh native DNS: {err:#}");
                        }
                    });
                }
            },
        })
        .build()
        .context("open DNS monitor")?;
    let keys = CFArray::from_CFTypes(
        &[
            "State:/Network/Global/DNS",
            "State:/Network/Global/IPv4",
            "State:/Network/Global/IPv6",
        ]
        .map(CFString::from_static_string),
    );
    let patterns = CFArray::from_CFTypes(&[CFString::from_static_string("(State|Setup):/Network/Service/.*/DNS")]);
    ensure!(
        store.set_notification_keys(&keys, &patterns),
        "subscribe to DNS changes"
    );
    let source = store.create_run_loop_source().context("create DNS run loop source")?;
    CFRunLoop::get_main().add_source(&source, unsafe { kCFRunLoopCommonModes });
    Ok(())
}

async fn refresh_dns() -> anyhow::Result<()> {
    let _config_write = Config::lock_config_write().await;
    let runtime = Config::runtime().await.data_arc();
    let Some(config) = runtime.config.as_ref() else {
        return Ok(());
    };
    if config
        .get("application-dns")
        .and_then(|profiles| profiles.get("__global_dns"))
        .is_none()
        || config
            .get("tun")
            .and_then(|tun| tun.get("enable"))
            .and_then(serde_yaml_ng::Value::as_bool)
            != Some(true)
    {
        return Ok(());
    }
    let fields = crate::enhance::application_profiles::native_upstream_fields(
        &crate::utils::resolve::dns::system_dns_upstreams().await?,
    );
    if let Some(dns) = config.get("dns").and_then(serde_yaml_ng::Value::as_mapping)
        && fields.iter().all(|(key, value)| dns.get(key) == Some(value))
    {
        return Ok(());
    }
    CoreManager::global().update_config_checked().await
}
