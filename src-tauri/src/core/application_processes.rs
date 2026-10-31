use anyhow::{Context as _, Result, ensure};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

#[derive(Debug, Clone, Serialize)]
pub struct ApplicationProcess {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub start_time: u64,
    pub name: std::string::String,
    pub path: std::string::String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProcessOverride {
    pub pid: u32,
    pub start_time: u64,
    pub enabled: bool,
    #[serde(default)]
    pub routing_mode: crate::config::application_proxy::ApplicationRoutingMode,
    pub proxy: Option<smartstring::alias::String>,
    #[serde(default)]
    pub profile_uid: Option<smartstring::alias::String>,
}

#[derive(Serialize)]
pub struct ApplicationProcessSnapshot {
    pub processes: Vec<ApplicationProcess>,
    pub overrides: Vec<ProcessOverride>,
}

static OVERRIDES: LazyLock<Mutex<Vec<ProcessOverride>>> = LazyLock::new(|| Mutex::new(Vec::new()));

#[cfg(target_os = "macos")]
fn identity(pid: u32) -> Option<(u64, u32)> {
    use tauri_plugin_clash_verge_sysinfo::libc;
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    // proc_pidinfo writes exactly one proc_bsdinfo when the process is still alive.
    let size = unsafe {
        libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            std::mem::size_of::<libc::proc_bsdinfo>() as i32,
        )
    };
    if size as usize != std::mem::size_of::<libc::proc_bsdinfo>() {
        return None;
    }
    let info = unsafe { info.assume_init() };
    Some((info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec, info.pbi_ppid))
}

#[cfg(target_os = "windows")]
fn start_time(pid: u32) -> Option<u64> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME},
        System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    };
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut created: FILETIME = std::mem::zeroed();
        let mut exited = std::mem::zeroed();
        let mut kernel = std::mem::zeroed();
        let mut user = std::mem::zeroed();
        let success = GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user);
        CloseHandle(handle);
        if success == 0 {
            return None;
        }
        let ticks = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
        (ticks / 10).checked_sub(11_644_473_600_000_000)
    }
}

fn collect() -> Vec<ApplicationProcess> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .without_tasks()
            .with_exe(UpdateKind::Always),
    );
    let processes: Vec<ApplicationProcess> = system
        .processes()
        .iter()
        .filter_map(|(pid, process)| {
            let path = process.exe()?.to_str()?.to_owned();
            #[cfg(target_os = "macos")]
            let (start_time, parent) = identity(pid.as_u32())?;
            #[cfg(target_os = "windows")]
            let (start_time, parent) = (
                start_time(pid.as_u32())?,
                process.parent().map_or(0, |parent| parent.as_u32()),
            );
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            let (start_time, parent) = (
                process.start_time() * 1_000_000,
                process.parent().map_or(0, |parent| parent.as_u32()),
            );
            Some(ApplicationProcess {
                pid: pid.as_u32(),
                parent_pid: (parent > 0).then_some(parent),
                start_time,
                name: process.name().to_string_lossy().into_owned(),
                path,
            })
        })
        .collect();
    #[cfg(target_os = "windows")]
    let processes = {
        let mut processes = processes;
        let starts: std::collections::HashMap<_, _> = processes
            .iter()
            .map(|process| (process.pid, process.start_time))
            .collect();
        for process in &mut processes {
            process.parent_pid = process
                .parent_pid
                .filter(|parent| starts.get(parent).is_some_and(|start| *start <= process.start_time));
        }
        processes
    };
    processes
}

pub async fn snapshot() -> Result<ApplicationProcessSnapshot> {
    let processes = crate::process::AsyncHandler::spawn_blocking(collect)
        .await
        .context("process snapshot failed")?;
    let mut overrides = OVERRIDES.lock();
    overrides.retain(|entry| {
        processes
            .iter()
            .any(|process| process.pid == entry.pid && process.start_time == entry.start_time)
    });
    Ok(ApplicationProcessSnapshot {
        processes,
        overrides: overrides.clone(),
    })
}

pub fn overrides() -> Vec<ProcessOverride> {
    OVERRIDES.lock().clone()
}
pub fn replace_overrides(overrides: Vec<ProcessOverride>) {
    *OVERRIDES.lock() = overrides;
}

pub async fn validate_identity(pid: u32, start_time: u64) -> Result<ApplicationProcess> {
    snapshot()
        .await?
        .processes
        .into_iter()
        .find(|process| process.pid == pid && process.start_time == start_time)
        .context("process has exited or its identity has changed")
}

pub async fn update(override_setting: ProcessOverride) -> Result<()> {
    let config_write = crate::config::Config::try_lock_config_write()?;
    ensure!(
        crate::config::Config::verge()
            .await
            .data_arc()
            .application_proxy
            .as_ref()
            .is_some_and(|settings| settings.enabled),
        "enable application routing before changing process settings"
    );
    let process = validate_identity(override_setting.pid, override_setting.start_time).await?;
    ensure!(
        cfg!(any(target_os = "macos", target_os = "windows")),
        "per-process routing requires macOS or Windows"
    );
    let validation = crate::config::application_proxy::ApplicationProxyConfig {
        enabled: true,
        applications: vec![crate::config::application_proxy::ApplicationProxyEntry {
            name: process.name.into(),
            path: process.path.into(),
            kind: crate::config::application_proxy::ApplicationMatchKind::Executable,
            enabled: override_setting.enabled,
            routing_mode: override_setting.routing_mode,
            proxy: override_setting.proxy.clone(),
            profile_uid: override_setting.profile_uid.clone(),
        }],
    };
    validation.validate()?;
    if let Some(uid) = override_setting.profile_uid.as_deref() {
        let profiles = crate::config::Config::profiles().await.data_arc();
        let item = profiles.get_item(uid)?;
        ensure!(
            matches!(item.itype.as_deref(), Some("remote" | "local")),
            "select a subscription profile"
        );
    }
    let before = overrides();
    let mut candidate = before.clone();
    candidate.retain(|entry| entry.pid != override_setting.pid);
    candidate.push(override_setting);
    replace_overrides(candidate);
    reconcile(&config_write, before).await
}

pub async fn reset(pid: u32, start_time: u64) -> Result<()> {
    let config_write = crate::config::Config::try_lock_config_write()?;
    ensure!(
        crate::config::Config::verge()
            .await
            .data_arc()
            .application_proxy
            .as_ref()
            .is_some_and(|settings| settings.enabled),
        "enable application routing before changing process settings"
    );
    let before = overrides();
    let mut candidate = before.clone();
    candidate.retain(|entry| entry.pid != pid || entry.start_time != start_time);
    replace_overrides(candidate);
    reconcile(&config_write, before).await
}

async fn reconcile(config_write: &tokio::sync::MutexGuard<'_, ()>, before: Vec<ProcessOverride>) -> Result<()> {
    let runtime_before = crate::config::Config::runtime().await.data_arc().config.clone();
    let current = crate::config::Config::verge()
        .await
        .data_arc()
        .application_proxy
        .clone()
        .unwrap_or_default();
    let result = crate::feat::apply_verge_patch_locked(
        config_write,
        &crate::config::IVerge {
            application_proxy: Some(current),
            ..crate::config::IVerge::default()
        },
        true,
    )
    .await;
    if result.is_err() && crate::config::Config::runtime().await.data_arc().config == runtime_before {
        replace_overrides(before);
    }
    result
}

pub(crate) fn start_cleanup() {
    crate::process::AsyncHandler::spawn(|| async {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if crate::core::handle::Handle::global().is_exiting() {
                break;
            }
            if !overrides().is_empty() {
                let _ = snapshot().await;
            }
        }
    });
}

#[cfg(all(test, any(target_os = "macos", target_os = "windows")))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn process_identity_rejects_a_stale_start_time() -> Result<()> {
        let pid = std::process::id();
        #[cfg(target_os = "macos")]
        let (start_time, _) = identity(pid).context("test process identity")?;
        #[cfg(target_os = "windows")]
        let start_time = start_time(pid).context("test process identity")?;
        ensure!(start_time > 0, "process start time must be available");
        let process = validate_identity(pid, start_time).await?;
        ensure!(!process.path.is_empty(), "executable path must be available");
        ensure!(
            validate_identity(pid, start_time + 1).await.is_err(),
            "stale instance must be rejected"
        );
        Ok(())
    }
}
