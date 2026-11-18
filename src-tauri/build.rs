fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_file = "../patches/mihomo-application-process.json";
    println!("cargo:rerun-if-changed={source_file}");
    let source: serde_json::Value = serde_json::from_slice(&std::fs::read(source_file)?)?;
    let core_version = source["app_version"]
        .as_str()
        .ok_or("Patched core version is missing")?;
    println!("cargo:rustc-env=APP_PROXY_CORE_VERSION={core_version}");
    if std::env::var_os("CARGO_FEATURE_CLIPPY").is_none() {
        validate_bundled_assets(core_version)?;
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
        && std::env::var_os("CARGO_FEATURE_CLIPPY").is_some()
    {
        // Tauri's resource is linked to the app binary, but not to Rust unit-test harnesses.
        // Give mock-context builds their own manifest so tests can resolve TaskDialogIndirect.
        let out_dir = std::env::var_os("OUT_DIR").ok_or_else(|| std::io::Error::other("OUT_DIR is not set"))?;
        let manifest_path = std::path::PathBuf::from(out_dir).join("windows-test-manifest.xml");
        std::fs::write(
            &manifest_path,
            r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
</assembly>
"#,
        )?;
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest_path.display());
    }

    #[cfg(feature = "clippy")]
    {
        println!("cargo:warning=Skipping tauri_build during Clippy");
    }

    #[cfg(not(feature = "clippy"))]
    tauri_build::build();

    Ok(())
}

fn validate_bundled_assets(core_version: &str) -> Result<(), Box<dyn std::error::Error>> {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS")?;
    if !matches!(target_os.as_str(), "macos" | "windows") {
        return Ok(());
    }
    let target = std::env::var("TARGET")?;
    let extension = if target_os == "windows" { ".exe" } else { "" };
    for core in ["verge-mihomo", "verge-mihomo-alpha"] {
        let path = format!("sidecar/{core}-{target}{extension}");
        println!("cargo:rerun-if-changed={path}");
        let bytes = std::fs::read(&path).map_err(|error| {
            format!("Missing bundled core {path}: {error}; run pnpm prebuild and pnpm build:process-core")
        })?;
        if !bytes
            .windows(core_version.len())
            .any(|bytes| bytes == core_version.as_bytes())
        {
            return Err(
                format!("Bundled core {path} does not contain {core_version}; rebuild the patched core").into(),
            );
        }
    }
    for name in [
        "clash-verge-service",
        "clash-verge-service-install",
        "clash-verge-service-uninstall",
    ] {
        let path = format!("resources/{name}{extension}");
        println!("cargo:rerun-if-changed={path}");
        if !std::fs::metadata(&path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0) {
            return Err(format!("Missing bundled service resource {path}; run pnpm prebuild").into());
        }
    }
    if target_os == "macos" {
        let source_file = "../patches/service-install-owner-release.json";
        println!("cargo:rerun-if-changed={source_file}");
        let source: serde_json::Value = serde_json::from_slice(&std::fs::read(source_file)?)?;
        let marker = source["marker"]
            .as_str()
            .ok_or("Service installer patch marker is missing")?;
        let installer = std::fs::read("resources/clash-verge-service-install")?;
        if !installer.windows(marker.len()).any(|bytes| bytes == marker.as_bytes()) {
            return Err("The macOS service installer is missing its upgrade fix; run pnpm prebuild".into());
        }
    }
    for name in ["Country.mmdb", "ASN.mmdb", "geoip.dat", "geosite.dat"] {
        let path = format!("resources/{name}");
        println!("cargo:rerun-if-changed={path}");
        if !std::fs::metadata(&path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0) {
            return Err(format!("Missing bundled resource {path}; run pnpm prebuild").into());
        }
    }
    Ok(())
}
