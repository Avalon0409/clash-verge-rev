use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};
use smartstring::alias::String;
use std::{collections::HashSet, path::Path};

pub fn subscription_prefix(uid: &str) -> std::string::String {
    use base64::Engine as _;
    format!("__ap_{}:", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(uid))
}

pub fn path_key(path: &str) -> std::string::String {
    #[cfg(target_os = "windows")]
    {
        let path = path.replace('\\', "/").to_lowercase();
        if let Some(unc) = path.strip_prefix("//?/unc/") {
            return format!("//{unc}");
        }
        path.strip_prefix("//?/").unwrap_or(&path).to_owned()
    }
    #[cfg(not(target_os = "windows"))]
    path.to_owned()
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct ApplicationProxyConfig {
    pub enabled: bool,
    pub applications: Vec<ApplicationProxyEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApplicationMatchKind {
    Bundle,
    Executable,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApplicationRoutingMode {
    #[default]
    Rule,
    Proxy,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProxyApplication {
    pub name: String,
    pub path: String,
    pub kind: ApplicationMatchKind,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApplicationProxyEntry {
    pub name: String,
    pub path: String,
    pub kind: ApplicationMatchKind,
    pub enabled: bool,
    #[serde(default)]
    pub routing_mode: ApplicationRoutingMode,
    pub proxy: Option<String>,
    #[serde(default)]
    pub profile_uid: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ApplicationProxyChange {
    SetMode {
        enabled: bool,
    },
    Add {
        application: ProxyApplication,
    },
    Configure {
        application: ProxyApplication,
        proxy: Option<String>,
        enabled: bool,
        #[serde(default)]
        routing_mode: ApplicationRoutingMode,
        #[serde(default)]
        profile_uid: Option<String>,
    },
    SetEnabled {
        path: String,
        enabled: bool,
    },
    SetProfile {
        path: String,
        profile_uid: Option<String>,
    },
    SetRoutingMode {
        path: String,
        routing_mode: ApplicationRoutingMode,
    },
    SetProxy {
        path: String,
        proxy: Option<String>,
    },
    Remove {
        path: String,
    },
}

impl ApplicationProxyConfig {
    pub fn apply_change(&mut self, change: ApplicationProxyChange) -> Result<()> {
        match change {
            ApplicationProxyChange::SetMode { enabled } => self.enabled = enabled,
            ApplicationProxyChange::Add { application } => {
                ensure!(
                    !self
                        .applications
                        .iter()
                        .any(|entry| path_key(&entry.path) == path_key(&application.path)),
                    "application path is already configured"
                );
                self.applications.push(ApplicationProxyEntry {
                    name: application.name,
                    path: application.path,
                    kind: application.kind,
                    enabled: false,
                    routing_mode: ApplicationRoutingMode::Rule,
                    proxy: None,
                    profile_uid: None,
                });
            }
            ApplicationProxyChange::Configure {
                application,
                proxy,
                enabled,
                routing_mode,
                profile_uid,
            } => {
                let entry = ApplicationProxyEntry {
                    name: application.name,
                    path: application.path,
                    kind: application.kind,
                    proxy,
                    enabled,
                    routing_mode,
                    profile_uid,
                };
                if let Some(current) = self
                    .applications
                    .iter_mut()
                    .find(|current| path_key(&current.path) == path_key(&entry.path))
                {
                    *current = entry;
                } else {
                    self.applications.push(entry);
                }
            }
            ApplicationProxyChange::SetEnabled { path, enabled } => self.entry_mut(&path)?.enabled = enabled,
            ApplicationProxyChange::SetProfile { path, profile_uid } => {
                let entry = self.entry_mut(&path)?;
                entry.profile_uid = profile_uid;
                entry.proxy = None;
            }
            ApplicationProxyChange::SetRoutingMode { path, routing_mode } => {
                self.entry_mut(&path)?.routing_mode = routing_mode
            }
            ApplicationProxyChange::SetProxy { path, proxy } => self.entry_mut(&path)?.proxy = proxy,
            ApplicationProxyChange::Remove { path } => {
                let index = self
                    .applications
                    .iter()
                    .position(|entry| path_key(&entry.path) == path_key(&path))
                    .context("application is no longer configured")?;
                self.applications.remove(index);
            }
        }
        self.validate()
    }

    fn entry_mut(&mut self, path: &str) -> Result<&mut ApplicationProxyEntry> {
        self.applications
            .iter_mut()
            .find(|entry| path_key(&entry.path) == path_key(path))
            .context("application is no longer configured")
    }

    pub fn validate(&self) -> Result<()> {
        let mut paths = HashSet::new();
        for application in &self.applications {
            ensure!(!application.name.trim().is_empty(), "application name is required");
            ensure!(
                Path::new(application.path.as_str()).is_absolute(),
                "application path must be absolute"
            );
            ensure!(
                !application.path.chars().any(char::is_control),
                "application path contains control characters"
            );
            ensure!(
                paths.insert(path_key(&application.path)),
                "application path is already configured"
            );
            if application.kind == ApplicationMatchKind::Bundle {
                ensure!(
                    cfg!(target_os = "macos"),
                    "application bundles are supported only on macOS"
                );
                ensure!(
                    application.path.ends_with(".app"),
                    "application bundle path must end with .app"
                );
            }
            if application.enabled && application.routing_mode == ApplicationRoutingMode::Proxy {
                ensure!(
                    application.proxy.as_ref().is_some_and(|proxy| !proxy.trim().is_empty()),
                    "proxy selection is required"
                );
            }
            if let Some(proxy) = &application.proxy {
                if let Some(uid) = application.profile_uid.as_deref() {
                    ensure!(
                        proxy.starts_with(&subscription_prefix(uid)),
                        "proxy does not belong to the selected subscription"
                    );
                } else {
                    ensure!(
                        !proxy.starts_with("__ap_"),
                        "select the subscription that owns this proxy"
                    );
                }

                ensure!(
                    !proxy.contains(',') && !proxy.chars().any(char::is_control),
                    "proxy name cannot contain commas or control characters"
                );
                ensure!(
                    !matches!(proxy.as_str(), "DIRECT" | "GLOBAL" | "REJECT" | "REJECT-DROP" | "PASS"),
                    "select a proxy node or group"
                );
            }
        }
        Ok(())
    }
}

impl ApplicationProxyEntry {
    pub fn rule_prefix(&self) -> std::string::String {
        let escaped = regex::escape(self.path.as_str()).replace(',', "\\x2c");
        match self.kind {
            ApplicationMatchKind::Bundle => format!("PROCESS-PATH-REGEX,^{escaped}/"),
            ApplicationMatchKind::Executable => format!("PROCESS-PATH-REGEX,^{escaped}$"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BROWSER_PATH: &str = if cfg!(target_os = "windows") {
        "C:/Apps/browser.exe"
    } else {
        "/usr/local/bin/browser"
    };
    const EDITOR_PATH: &str = if cfg!(target_os = "windows") {
        "C:/Apps/editor.exe"
    } else {
        "/usr/local/bin/editor"
    };

    #[test]
    fn node_binding_must_belong_to_its_subscription() -> Result<()> {
        let mut config = ApplicationProxyConfig {
            enabled: true,
            applications: vec![ApplicationProxyEntry {
                name: "Browser".into(),
                path: BROWSER_PATH.into(),
                kind: ApplicationMatchKind::Executable,
                enabled: true,
                routing_mode: ApplicationRoutingMode::Rule,
                profile_uid: Some("B".into()),
                proxy: Some(format!("{}Exit", subscription_prefix("C")).into()),
            }],
        };
        assert!(config.validate().is_err());
        config.applications[0].proxy = Some(format!("{}Exit", subscription_prefix("B")).into());
        config.validate()?;
        config.applications[0].profile_uid = None;
        assert!(config.validate().is_err());
        Ok(())
    }

    #[test]
    fn existing_entries_default_to_rules_and_keep_the_saved_node() -> Result<()> {
        let config: ApplicationProxyConfig = serde_yaml_ng::from_str(&format!(
            "enabled: true\napplications: [{{name: Browser, path: {BROWSER_PATH}, kind: executable, enabled: true, proxy: Exit}}]"
        ))?;
        assert_eq!(config.applications[0].routing_mode, ApplicationRoutingMode::Rule);
        assert_eq!(config.applications[0].proxy.as_deref(), Some("Exit"));
        config.validate()?;
        let mut defaults = ApplicationProxyConfig::default();
        defaults.apply_change(ApplicationProxyChange::Configure {
            application: ProxyApplication {
                name: "Browser".into(),
                path: BROWSER_PATH.into(),
                kind: ApplicationMatchKind::Executable,
            },
            proxy: None,
            profile_uid: None,
            enabled: true,
            routing_mode: ApplicationRoutingMode::Rule,
        })?;
        assert!(defaults.applications[0].enabled);
        Ok(())
    }

    #[test]
    fn incremental_changes_keep_other_applications_and_survive_serialization() -> Result<()> {
        let mut config = ApplicationProxyConfig::default();
        for path in [BROWSER_PATH, EDITOR_PATH] {
            config.apply_change(ApplicationProxyChange::Add {
                application: ProxyApplication {
                    name: "Application".into(),
                    path: path.into(),
                    kind: ApplicationMatchKind::Executable,
                },
            })?;
        }
        let mut invalid = config.clone();
        invalid.applications[0].enabled = true;
        assert!(
            invalid
                .apply_change(ApplicationProxyChange::SetRoutingMode {
                    path: BROWSER_PATH.into(),
                    routing_mode: ApplicationRoutingMode::Proxy
                })
                .is_err()
        );
        config.apply_change(ApplicationProxyChange::SetProxy {
            path: BROWSER_PATH.into(),
            proxy: Some("Exit".into()),
        })?;
        config.apply_change(ApplicationProxyChange::SetEnabled {
            path: BROWSER_PATH.into(),
            enabled: true,
        })?;
        config.apply_change(ApplicationProxyChange::SetMode { enabled: true })?;
        let restarted: ApplicationProxyConfig = serde_yaml_ng::from_str(&serde_yaml_ng::to_string(&config)?)?;
        assert!(restarted.enabled && restarted.applications[0].enabled);
        assert!(!restarted.applications[1].enabled);
        assert_eq!(restarted.applications[0].proxy.as_deref(), Some("Exit"));
        assert_eq!(restarted.applications[1].proxy, None);
        config.apply_change(ApplicationProxyChange::Configure {
            application: ProxyApplication {
                name: "Browser".into(),
                path: BROWSER_PATH.into(),
                kind: ApplicationMatchKind::Executable,
            },
            proxy: Some("AnotherExit".into()),
            profile_uid: None,
            enabled: true,
            routing_mode: ApplicationRoutingMode::Proxy,
        })?;
        assert_eq!(config.applications.len(), 2);
        assert_eq!(config.applications[0].proxy.as_deref(), Some("AnotherExit"));
        assert!(!config.applications[1].enabled);
        #[cfg(target_os = "windows")]
        {
            config.apply_change(ApplicationProxyChange::SetEnabled {
                path: r"\\?\c:\APPS\BROWSER.EXE".into(),
                enabled: false,
            })?;
            assert!(!config.applications[0].enabled);
            assert!(
                config
                    .apply_change(ApplicationProxyChange::Add {
                        application: ProxyApplication {
                            name: "Alias".into(),
                            path: r"C:\apps\Browser.exe".into(),
                            kind: ApplicationMatchKind::Executable
                        }
                    })
                    .is_err()
            );
        }
        Ok(())
    }
}
