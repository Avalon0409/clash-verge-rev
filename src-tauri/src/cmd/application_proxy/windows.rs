use crate::config::application_proxy::{ApplicationMatchKind, ProxyApplication, path_key};
use anyhow::{Context as _, Result};
use std::path::{Path, PathBuf};

struct ComApartment(bool);

impl ComApartment {
    fn initialize() -> windows::core::Result<Self> {
        use windows::Win32::{
            Foundation::RPC_E_CHANGED_MODE,
            System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx},
        };
        let status = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        // Tokio may reuse a blocking thread whose COM apartment is already initialized.
        if status == RPC_E_CHANGED_MODE {
            return Ok(Self(false));
        }
        status.ok()?;
        Ok(Self(true))
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe {
                windows::Win32::System::Com::CoUninitialize();
            }
        }
    }
}

pub(super) fn list_applications() -> Result<Vec<ProxyApplication>> {
    use windows::{
        Win32::{
            System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, IPersistFile, STGM_READ},
            UI::Shell::{IShellLinkW, ShellLink},
        },
        core::{Interface as _, PCWSTR},
    };
    let _apartment = ComApartment::initialize().context("Unable to read Windows application shortcuts")?;
    let mut directories = Vec::new();
    if let Some(data) = dirs::data_dir() {
        directories.push(data.join("Microsoft/Windows/Start Menu/Programs"));
    }
    if let Some(data) = std::env::var_os("ProgramData") {
        directories.push(PathBuf::from(data).join("Microsoft/Windows/Start Menu/Programs"));
    }
    let mut applications = Vec::new();
    while let Some(directory) = directories.pop() {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_type().is_ok_and(|kind| kind.is_dir() && !kind.is_symlink()) {
                directories.push(path);
                continue;
            }
            if !path.extension().is_some_and(|extension| {
                extension
                    .to_str()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("lnk"))
            }) {
                continue;
            }
            let target = (|| -> windows::core::Result<std::string::String> {
                let link: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }?;
                let file: IPersistFile = link.cast()?;
                use std::os::windows::ffi::OsStrExt as _;
                let path: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
                unsafe {
                    file.Load(PCWSTR(path.as_ptr()), STGM_READ)?;
                }
                let mut target = vec![0; 32768];
                unsafe {
                    link.GetPath(&mut target, std::ptr::null_mut(), 0)?;
                }
                let length = target
                    .iter()
                    .position(|character| *character == 0)
                    .unwrap_or(target.len());
                Ok(std::string::String::from_utf16_lossy(&target[..length]))
            })();
            let Ok(target) = target else {
                continue;
            };
            let target = Path::new(&target);
            if !matches!(target.components().next(), Some(std::path::Component::Prefix(prefix)) if matches!(prefix.kind(), std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)))
                || !target.is_absolute()
                || !target.is_file()
                || !target.extension().is_some_and(|extension| {
                    extension
                        .to_str()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
                })
            {
                continue;
            }
            let Ok(target) = std::fs::canonicalize(target) else {
                continue;
            };
            let Some(name) = path.file_stem().and_then(|name| name.to_str()) else {
                continue;
            };
            let Some(target) = target.to_str() else {
                continue;
            };
            applications.push(ProxyApplication {
                name: name.into(),
                path: display_path(target).into(),
                kind: ApplicationMatchKind::Executable,
            });
        }
    }
    applications.sort_by_key(|application| path_key(&application.path));
    applications.dedup_by(|left, right| path_key(&left.path) == path_key(&right.path));
    applications.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(applications)
}

pub(super) fn display_path(path: &str) -> std::string::String {
    if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{unc}");
    }
    path.strip_prefix(r"\\?\").unwrap_or(path).to_owned()
}

pub(super) fn application_icon(path: &str) -> Option<std::string::String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use windows_sys::Win32::{
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC,
            DeleteObject, GdiFlush, SelectObject,
        },
        UI::{
            Shell::{SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGetFileInfoW},
            WindowsAndMessaging::{DI_NORMAL, DestroyIcon, DrawIconEx},
        },
    };
    let _apartment = ComApartment::initialize().ok()?;
    let path: Vec<_> = path.encode_utf16().chain(Some(0)).collect();
    let mut info: SHFILEINFOW = unsafe { std::mem::zeroed() };
    let found = unsafe {
        SHGetFileInfoW(
            path.as_ptr(),
            0,
            &mut info,
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        )
    };
    if found == 0 || info.hIcon.is_null() {
        return None;
    }
    scopeguard::defer! { unsafe { DestroyIcon(info.hIcon); } }
    unsafe {
        let dc = CreateCompatibleDC(std::ptr::null_mut());
        if dc.is_null() {
            return None;
        }
        scopeguard::defer! { DeleteDC(dc); }
        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: 32,
                biHeight: -32,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            },
            ..std::mem::zeroed()
        };
        let mut bits = std::ptr::null_mut();
        let bitmap = CreateDIBSection(dc, &bitmap_info, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
        if bitmap.is_null() {
            return None;
        }
        scopeguard::defer! { DeleteObject(bitmap); }
        let previous = SelectObject(dc, bitmap);
        scopeguard::defer! { SelectObject(dc, previous); }
        std::ptr::write_bytes(bits.cast::<u8>(), 0, 32 * 32 * 4);
        if DrawIconEx(dc, 0, 0, info.hIcon, 32, 32, 0, std::ptr::null_mut(), DI_NORMAL) == 0 {
            return None;
        }
        GdiFlush();
        let mut pixels = std::slice::from_raw_parts(bits.cast::<u8>(), 32 * 32 * 4).to_vec();
        let has_alpha = pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0);
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
            if !has_alpha {
                pixel[3] = 255;
            } else if pixel[3] > 0 {
                for channel in 0..3 {
                    pixel[channel] = (u32::from(pixel[channel]) * 255 / u32::from(pixel[3])).min(255) as u8;
                }
            }
        }
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 32, 32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().ok()?.write_image_data(&pixels).ok()?;
        Some(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
    }
}
