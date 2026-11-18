use crate::{
    config::Config,
    core::{CoreManager, handle::Handle},
};
use anyhow::{Context as _, Result, ensure};
use std::{path::Path, process::Command};

pub async fn use_bundled_core() -> Result<()> {
    let core = Config::verge().await.latest_arc().get_valid_clash_core();
    let path = std::env::current_exe()?.with_file_name(format!("{core}{}", std::env::consts::EXE_SUFFIX));
    let to = tokio::task::spawn_blocking(move || verify_bundled_core(&path)).await??;
    CoreManager::global()
        .restart_core()
        .await
        .context("Unable to start the bundled core")?;
    let running = Handle::mihomo()
        .get_version()
        .await
        .context("Bundled core did not become ready")?;
    ensure!(
        running.version.as_str() == to,
        "The service is still running an older core; repair the service in Settings"
    );
    Ok(())
}

pub(crate) fn verify_bundled_core(path: &Path) -> Result<std::string::String> {
    let mut command = Command::new(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        command.creation_flags(0x0800_0000);
    }
    let output = command
        .arg("-v")
        .output()
        .context("Unable to read bundled core version")?;
    ensure!(output.status.success(), "Bundled core version check failed");
    let version = String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .nth(2)
        .map(str::to_owned)
        .context("Bundled core returned an invalid version")?;
    ensure!(
        version == env!("APP_PROXY_CORE_VERSION"),
        "The bundled core is incompatible; reinstall this project's application package"
    );
    Ok(version)
}
