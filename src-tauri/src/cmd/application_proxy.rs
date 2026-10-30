use super::{CmdResult, WithErrorCode as _};
use crate::config::application_proxy::{ApplicationMatchKind, ApplicationProxyChange, ProxyApplication};
use anyhow::{Context as _, Result, ensure};
use std::path::{Path, PathBuf};
use tokio::fs;

#[tauri::command]
pub async fn update_application_proxy(mut change: ApplicationProxyChange) -> CmdResult<()> {
    if let ApplicationProxyChange::Add { application } | ApplicationProxyChange::Configure { application, .. } =
        &mut change
    {
        *application = resolve_application(Path::new(application.path.as_str()))
            .await
            .with_error_code("APPLICATION_PATH_INVALID")?;
    }
    let uid = match &change {
        ApplicationProxyChange::Configure { profile_uid, .. }
        | ApplicationProxyChange::SetProfile { profile_uid, .. } => profile_uid.as_deref(),
        _ => None,
    };
    if let Some(uid) = uid {
        let profiles = crate::config::Config::profiles().await.data_arc();
        let item = profiles
            .get_item(uid)
            .with_error_code("APPLICATION_SUBSCRIPTION_INVALID")?;
        matches!(item.itype.as_deref(), Some("remote" | "local"))
            .then_some(())
            .ok_or_else(|| anyhow::anyhow!("select a subscription profile"))
            .with_error_code("APPLICATION_SUBSCRIPTION_INVALID")?;
    }
    crate::feat::update_application_proxy(change)
        .await
        .with_error_code("APPLICATION_PROXY_UPDATE_FAILED")
}

#[tauri::command]
pub async fn get_proxy_application_icon(path: String) -> CmdResult<Option<String>> {
    #[cfg(target_os = "macos")]
    {
        ensure_icon_path(&path).with_error_code("APPLICATION_PATH_INVALID")?;
        tokio::task::spawn_blocking(move || objc2::rc::autoreleasepool(|_| application_icon(&path)))
            .await
            .with_error_code("APPLICATION_ICON_FAILED")
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        Ok(None)
    }
}

#[cfg(target_os = "macos")]
fn ensure_icon_path(path: &str) -> Result<()> {
    ensure!(Path::new(path).is_absolute(), "application path must be absolute");
    Ok(())
}

#[cfg(target_os = "macos")]
fn application_icon(path: &str) -> Option<String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use objc2::AnyThread as _;
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};
    use objc2_foundation::{NSDictionary, NSString};
    if !Path::new(path).exists() {
        return None;
    }
    let icon_path = path.find(".app/Contents/").map_or(path, |index| &path[..index + 4]);
    let image = NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(icon_path));
    let tiff = image.TIFFRepresentation()?;
    let bitmap = NSBitmapImageRep::initWithData(NSBitmapImageRep::alloc(), &tiff)?;
    // No image properties are supplied, so the dictionary has no typed values to validate.
    let png = unsafe { bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new()) }?;
    Some(format!("data:image/png;base64,{}", STANDARD.encode(png.to_vec())))
}

#[tauri::command]
pub async fn resolve_proxy_application(path: String) -> CmdResult<ProxyApplication> {
    resolve_application(Path::new(&path))
        .await
        .with_error_code("APPLICATION_PATH_INVALID")
}

async fn resolve_application(path: &Path) -> Result<ProxyApplication> {
    ensure!(path.is_absolute(), "application path must be absolute");
    let path = fs::canonicalize(path)
        .await
        .context("application path does not exist")?;
    let metadata = fs::metadata(&path).await?;
    let kind = if cfg!(target_os = "macos") && path.extension().is_some_and(|extension| extension == "app") {
        ensure!(
            metadata.is_dir() && path.join("Contents/MacOS").is_dir(),
            "select a macOS application bundle"
        );
        ApplicationMatchKind::Bundle
    } else {
        ensure!(metadata.is_file(), "select an application bundle or executable file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            ensure!(
                metadata.permissions().mode() & 0o111 != 0,
                "selected file is not executable"
            );
        }
        ApplicationMatchKind::Executable
    };
    let name = if kind == ApplicationMatchKind::Bundle {
        path.file_stem()
    } else {
        path.file_name()
    }
    .and_then(|name| name.to_str())
    .context("application name is not valid UTF-8")?;
    let application = ProxyApplication {
        name: name.into(),
        path: path.to_str().context("application path is not valid UTF-8")?.into(),
        kind,
    };
    Ok(application)
}

#[tauri::command]
pub async fn list_proxy_applications() -> CmdResult<Vec<ProxyApplication>> {
    list_applications().await.with_error_code("APPLICATION_LIST_FAILED")
}

async fn list_applications() -> Result<Vec<ProxyApplication>> {
    if !cfg!(target_os = "macos") {
        return Ok(Vec::new());
    }
    let mut directories = vec![
        (PathBuf::from("/Applications"), 0),
        (PathBuf::from("/System/Applications"), 0),
    ];
    if let Some(home) = dirs::home_dir() {
        directories.push((home.join("Applications"), 0));
    }
    let mut applications = Vec::new();
    while let Some((directory, depth)) = directories.pop() {
        let mut entries = match fs::read_dir(&directory).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error).context("failed to read application directory"),
        };
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|extension| extension == "app") {
                if path.join("Contents/MacOS").is_dir() {
                    applications.push(resolve_application(&path).await?);
                }
            } else if depth < 2 && entry.file_type().await?.is_dir() {
                directories.push((path, depth + 1));
            }
        }
    }
    applications.sort_by(|left, right| left.name.cmp(&right.name).then_with(|| left.path.cmp(&right.path)));
    applications.dedup_by(|left, right| left.path == right.path);
    Ok(applications)
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    #[tokio::test]
    async fn native_icon_is_a_png_from_the_background_command() -> Result<()> {
        let path = std::env::current_exe()?;
        let icon = get_proxy_application_icon(path.to_str().context("executable path")?.to_owned())
            .await
            .map_err(anyhow::Error::msg)?;
        let icon = icon.context("native executable icon")?;
        let bytes = STANDARD.decode(icon.strip_prefix("data:image/png;base64,").context("PNG data URL")?)?;
        ensure!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "native icon must be a PNG");
        Ok(())
    }
}

#[tauri::command]
pub async fn get_application_processes() -> CmdResult<crate::core::application_processes::ApplicationProcessSnapshot> {
    crate::core::application_processes::snapshot()
        .await
        .with_error_code("APPLICATION_PROCESS_LIST_FAILED")
}

#[tauri::command]
pub async fn update_application_process(setting: crate::core::application_processes::ProcessOverride) -> CmdResult<()> {
    crate::core::application_processes::update(setting)
        .await
        .with_error_code("APPLICATION_PROCESS_UPDATE_FAILED")
}

#[tauri::command]
pub async fn reset_application_process(pid: u32, start_time: u64) -> CmdResult<()> {
    crate::core::application_processes::reset(pid, start_time)
        .await
        .with_error_code("APPLICATION_PROCESS_UPDATE_FAILED")
}

#[tauri::command]
pub async fn list_application_profile_outbounds(
    uid: String,
) -> CmdResult<Vec<crate::enhance::application_profiles::OutboundOption>> {
    crate::enhance::application_profiles::catalog(&uid)
        .await
        .with_error_code("APPLICATION_SUBSCRIPTION_READ_FAILED")
}
