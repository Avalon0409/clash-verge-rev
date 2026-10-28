use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};
use smartstring::alias::String;
use std::{collections::HashSet, path::Path};

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
    },
    SetEnabled {
        path: String,
        enabled: bool,
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
                    !self.applications.iter().any(|entry| entry.path == application.path),
                    "application path is already configured"
                );
                self.applications.push(ApplicationProxyEntry {
                    name: application.name,
                    path: application.path,
                    kind: application.kind,
                    enabled: false,
                    routing_mode: ApplicationRoutingMode::Rule,
                    proxy: None,
                });
            }
            ApplicationProxyChange::Configure {
                application,
                proxy,
                enabled,
                routing_mode,
            } => {
                let entry = ApplicationProxyEntry {
                    name: application.name,
                    path: application.path,
                    kind: application.kind,
                    proxy,
                    enabled,
                    routing_mode,
                };
                if let Some(current) = self.applications.iter_mut().find(|current| current.path == entry.path) {
                    *current = entry;
                } else {
                    self.applications.push(entry);
                }
            }
            ApplicationProxyChange::SetEnabled { path, enabled } => self.entry_mut(&path)?.enabled = enabled,
            ApplicationProxyChange::SetRoutingMode { path, routing_mode } => {
                self.entry_mut(&path)?.routing_mode = routing_mode
            }
            ApplicationProxyChange::SetProxy { path, proxy } => self.entry_mut(&path)?.proxy = proxy,
            ApplicationProxyChange::Remove { path } => {
                let index = self
                    .applications
                    .iter()
                    .position(|entry| entry.path == path)
                    .context("application is no longer configured")?;
                self.applications.remove(index);
            }
        }
        self.validate()
    }

    fn entry_mut(&mut self, path: &str) -> Result<&mut ApplicationProxyEntry> {
        self.applications
            .iter_mut()
            .find(|entry| entry.path == path)
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
                paths.insert(&application.path),
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

    #[test]
    fn existing_entries_default_to_rules_and_keep_the_saved_node() -> Result<()> {
        let config: ApplicationProxyConfig = serde_yaml_ng::from_str(
            "enabled: true\napplications: [{name: Browser, path: /usr/local/bin/browser, kind: executable, enabled: true, proxy: Exit}]",
        )?;
        assert_eq!(config.applications[0].routing_mode, ApplicationRoutingMode::Rule);
        assert_eq!(config.applications[0].proxy.as_deref(), Some("Exit"));
        config.validate()?;
        let mut defaults = ApplicationProxyConfig::default();
        defaults.apply_change(ApplicationProxyChange::Configure {
            application: ProxyApplication {
                name: "Browser".into(),
                path: "/usr/local/bin/browser".into(),
                kind: ApplicationMatchKind::Executable,
            },
            proxy: None,
            enabled: true,
            routing_mode: ApplicationRoutingMode::Rule,
        })?;
        assert!(defaults.applications[0].enabled);
        Ok(())
    }

    #[test]
    fn incremental_changes_keep_other_applications_and_survive_serialization() -> Result<()> {
        let mut config = ApplicationProxyConfig::default();
        for path in ["/usr/local/bin/browser", "/usr/local/bin/editor"] {
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
                    path: "/usr/local/bin/browser".into(),
                    routing_mode: ApplicationRoutingMode::Proxy
                })
                .is_err()
        );
        config.apply_change(ApplicationProxyChange::SetProxy {
            path: "/usr/local/bin/browser".into(),
            proxy: Some("Exit".into()),
        })?;
        config.apply_change(ApplicationProxyChange::SetEnabled {
            path: "/usr/local/bin/browser".into(),
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
                path: "/usr/local/bin/browser".into(),
                kind: ApplicationMatchKind::Executable,
            },
            proxy: Some("AnotherExit".into()),
            enabled: true,
            routing_mode: ApplicationRoutingMode::Proxy,
        })?;
        assert_eq!(config.applications.len(), 2);
        assert_eq!(config.applications[0].proxy.as_deref(), Some("AnotherExit"));
        assert!(!config.applications[1].enabled);
        Ok(())
    }
}
