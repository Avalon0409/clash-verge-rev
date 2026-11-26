use anyhow::{Context as _, bail, ensure};
use clash_verge_logging::{Type, logging};
use std::{
    io::Read as _,
    os::unix::process::CommandExt as _,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

const DNS_STATE_FILE: &str = ".original_dns.txt";
const DNS_BACKUP_DIR: &str = ".original_dns";
// Used only to recognize DNS written by older releases.
const PLACEHOLDER_DNS: &str = "114.114.114.114";
// Scripts run under lifecycle_lock, and networksetup waits for other preference writers without a limit.
pub(crate) const DNS_SCRIPT_TIMEOUT: Duration = Duration::from_secs(5);

static DNS_LOCK: Mutex<()> = Mutex::const_new(());

#[derive(Debug, Default, PartialEq, Eq)]
pub struct NativeDns {
    pub default: Vec<String>,
    pub domains: std::collections::BTreeMap<String, Vec<String>>,
}

fn store_value(
    store: &system_configuration::dynamic_store::SCDynamicStore,
    key: &str,
    field: &str,
) -> Option<system_configuration::core_foundation::base::CFType> {
    use system_configuration::core_foundation::{
        base::{CFType, TCFType as _},
        dictionary::CFDictionary,
        string::CFString,
    };
    let dict = store.get(key)?.downcast_into::<CFDictionary>()?;
    let value = dict.find(CFString::new(field).as_CFTypeRef())?;
    Some(unsafe { CFType::wrap_under_get_rule(*value) })
}

fn network_services() -> anyhow::Result<(Vec<String>, String, String)> {
    use system_configuration::{
        core_foundation::{
            array::CFArray,
            base::{CFType, TCFType as _},
            string::CFString,
        },
        dynamic_store::SCDynamicStoreBuilder,
    };
    let store = SCDynamicStoreBuilder::new("Clash Verge native DNS")
        .build()
        .context("open DNS dynamic store")?;
    let mut owned = Vec::new();
    let keys = store
        .get_keys("State:/Network/Service/ClashVergeTunDNS-.*/DNS")
        .context("list TUN DNS services")?;
    for key in keys.iter() {
        if let Some(addresses) =
            store_value(&store, &key.to_string(), "ServerAddresses").and_then(CFType::downcast_into::<CFArray>)
        {
            for address in &addresses {
                if let Some(ip) = unsafe { CFType::wrap_under_get_rule(*address) }.downcast_into::<CFString>() {
                    owned.push(ip.to_string());
                }
            }
        }
    }
    let primary = |field| {
        ["State:/Network/Global/IPv4", "State:/Network/Global/IPv6"]
            .iter()
            .find_map(|key| {
                store_value(&store, key, field)?
                    .downcast_into::<CFString>()
                    .map(|s| s.to_string())
            })
            .unwrap_or_default()
    };
    Ok((owned, primary("PrimaryInterface"), primary("PrimaryService")))
}

fn parse_resolvers(text: &str, owned: &[String], primary: &str) -> NativeDns {
    let owned: Vec<std::net::IpAddr> = owned
        .iter()
        .filter_map(|value| value.split('%').next()?.parse().ok())
        .collect();
    let mut dns = NativeDns::default();
    let mut scoped_default = Vec::new();
    for section in text.split("DNS configuration").skip(1) {
        let scoped = section.trim_start().starts_with("(for scoped queries)");
        if section.trim_start().starts_with("(for service-specific queries)") {
            continue;
        }
        for record in section.split("resolver #").skip(1) {
            let mut domain = "";
            let mut interface = "";
            let mut servers = Vec::new();
            for line in record.lines() {
                let Some((key, value)) = line.split_once(':') else {
                    continue;
                };
                let value = value.trim();
                match key.trim() {
                    "domain" => domain = value.trim_end_matches('.'),
                    "if_index" => {
                        interface = value
                            .split_once('(')
                            .and_then(|(_, name)| name.strip_suffix(')'))
                            .unwrap_or("")
                    }
                    key if key.starts_with("nameserver[") => {
                        let ip = value.split('%').next().unwrap_or(value);
                        if let Ok(ip) = ip.parse::<std::net::IpAddr>()
                            && !owned.contains(&ip)
                            && !servers.iter().any(|old| old == value)
                        {
                            servers.push(value.to_owned());
                        }
                    }
                    _ => {}
                }
            }
            if servers.is_empty() {
                continue;
            }
            if !interface.is_empty() {
                for server in &mut servers {
                    if let Ok(ip) = server.parse::<std::net::Ipv6Addr>()
                        && ip.is_unicast_link_local()
                    {
                        server.push('%');
                        server.push_str(interface);
                    }
                }
            }
            if !domain.is_empty() && !scoped {
                dns.domains.entry(format!("+.{domain}")).or_insert(servers);
            } else if domain.is_empty() && !scoped && dns.default.is_empty() {
                dns.default = servers;
            } else if domain.is_empty()
                && scoped
                && !primary.is_empty()
                && interface == primary
                && scoped_default.is_empty()
            {
                scoped_default = servers;
            }
        }
    }
    if dns.default.is_empty() {
        dns.default = scoped_default;
    }
    dns
}

pub async fn system_dns_upstreams() -> anyhow::Result<NativeDns> {
    let (mut owned, _, _) = network_services()?;
    let output = tokio::process::Command::new("/usr/sbin/scutil")
        .arg("--dns")
        .output()
        .await?;
    ensure!(
        output.status.success(),
        "read macOS DNS configuration: {}",
        output.status
    );
    let (after, primary, service) = network_services()?;
    owned.extend(after);
    let mut dns = parse_resolvers(&String::from_utf8_lossy(&output.stdout), &owned, &primary);
    if dns.default == [PLACEHOLDER_DNS] && !service.is_empty() {
        let dir = restore_dns_state_dir(&crate::utils::dirs::app_resources_dir()?, dns_state_dir()?);
        let backup = dir.join(DNS_BACKUP_DIR).join(service.trim());
        if backup.exists() {
            let original = tokio::fs::read_to_string(backup).await?;
            let original: Vec<_> = original
                .lines()
                .filter(|line| line.parse::<std::net::IpAddr>().is_ok())
                .map(ToOwned::to_owned)
                .collect();
            if !original.is_empty() {
                dns.default = original.into_iter().filter(|ip| !owned.contains(ip)).collect();
            }
        }
    }
    Ok(dns)
}

fn dns_state_dir() -> anyhow::Result<PathBuf> {
    // The DNS scripts persist their backups relative to their working directory.
    let dir = crate::utils::dirs::app_home_dir()?;
    std::fs::create_dir_all(&dir).with_context(|| format!("failed to create DNS state directory {}", dir.display()))?;
    Ok(dir)
}

fn restore_dns_state_dir(resource_dir: &Path, state_dir: PathBuf) -> PathBuf {
    if resource_dir.join(DNS_STATE_FILE).exists() || resource_dir.join(DNS_BACKUP_DIR).exists() {
        resource_dir.to_path_buf()
    } else {
        state_dir
    }
}

/// Restores DNS left by older versions without installing a replacement resolver.
pub async fn sync_public_dns() {
    if crate::core::handle::Handle::global().is_exiting() {
        return;
    }
    if let Err(err) = restore_public_dns().await {
        logging!(error, Type::Config, "restore legacy system DNS failed: {err:#}");
    }
}

pub async fn restore_public_dns() -> anyhow::Result<()> {
    let _guard = DNS_LOCK.lock().await;
    unset_public_dns().await
}

async fn unset_public_dns() -> anyhow::Result<()> {
    logging!(debug, Type::Config, "try to unset system dns");
    let result = async {
        let resource_dir = crate::utils::dirs::app_resources_dir()?;
        let state_dir = restore_dns_state_dir(&resource_dir, dns_state_dir()?);
        if !state_dir.join(DNS_STATE_FILE).exists() && !state_dir.join(DNS_BACKUP_DIR).exists() {
            return Ok(());
        }
        run_dns_script(&resource_dir.join("unset_dns.sh"), state_dir).await
    }
    .await;
    match &result {
        Ok(()) => logging!(info, Type::Config, "unset system dns successfully"),
        Err(err) => logging!(error, Type::Config, "unset system dns failed: {err:#}"),
    }
    result
}

/// On timeout the script's whole process group is killed, so no networksetup it started writes late.
async fn run_dns_script(script: &Path, dir: PathBuf) -> anyhow::Result<()> {
    ensure!(script.exists(), "DNS script not found: {}", script.display());
    let mut child = Command::new("bash")
        .arg(script)
        .arg(PLACEHOLDER_DNS)
        .current_dir(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .with_context(|| format!("failed to run {}", script.display()))?;
    crate::process::AsyncHandler::spawn_blocking(move || {
        let deadline = Instant::now() + DNS_SCRIPT_TIMEOUT;
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if Instant::now() >= deadline {
                unsafe { libc::killpg(libc::pid_t::try_from(child.id())?, libc::SIGKILL) };
                child.wait()?;
                bail!("timed out after {} s", DNS_SCRIPT_TIMEOUT.as_secs());
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let mut stderr = String::new();
        if let Some(mut pipe) = child.stderr.take() {
            pipe.read_to_string(&mut stderr)?;
        }
        ensure!(
            status.success(),
            "exit code {}: {}",
            status.code().unwrap_or(-1),
            stderr.trim()
        );
        Ok(())
    })
    .await
    .context("DNS script task failed")?
}

#[cfg(test)]
mod tests {

    #[test]
    fn native_defaults_and_vpn_domains_exclude_owned_addresses_across_records() {
        let output = "DNS configuration\nresolver #1\nnameserver[0] : 198.18.0.2\nresolver #2\ndomain : corp.test\nnameserver[0] : 10.0.0.53\nif_index : 14 (utun4)\nDNS configuration (for scoped queries)\nresolver #1\nnameserver[0] : 198.18.0.2\nnameserver[1] : 192.0.2.53\nnameserver[2] : fe80::1\nif_index : 3 (en1)";
        let native = super::parse_resolvers(output, &["198.18.0.2".into()], "en1");
        assert_eq!(native.default, ["192.0.2.53", "fe80::1%en1"]);
        assert_eq!(native.domains["+.corp.test"], ["10.0.0.53"]);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn restore_dns_state_dir_defaults_to_app_data_but_honors_legacy_file() {
        use super::restore_dns_state_dir;

        let root = std::env::temp_dir().join(format!("clash-verge-dns-{}", nanoid::nanoid!()));
        let resource_dir = root.join("resources");
        let state_dir = root.join("app-data");
        std::fs::create_dir_all(&resource_dir).expect("create test resource directory");

        assert_eq!(restore_dns_state_dir(&resource_dir, state_dir.clone()), state_dir);

        std::fs::write(resource_dir.join(".original_dns.txt"), "empty").expect("create legacy DNS state");
        assert_eq!(restore_dns_state_dir(&resource_dir, state_dir.clone()), resource_dir);

        // A migrated legacy backup whose restore failed must still be retried there.
        std::fs::remove_file(resource_dir.join(".original_dns.txt")).expect("remove legacy DNS state");
        std::fs::create_dir(resource_dir.join(".original_dns")).expect("create migrated DNS state");
        assert_eq!(restore_dns_state_dir(&resource_dir, state_dir), resource_dir);

        std::fs::remove_dir_all(root).expect("remove test directory");
    }
}
