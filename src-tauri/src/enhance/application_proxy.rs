use super::application_profiles;
use crate::config::application_proxy::ApplicationProxyConfig;
use anyhow::Result;
use serde_yaml_ng::{Mapping, Value};
use std::collections::HashSet;

pub fn use_application_proxy(mut config: Mapping, settings: Option<&ApplicationProxyConfig>) -> Result<Mapping> {
    let Some(settings) = settings.filter(|settings| settings.enabled) else {
        return Ok(config);
    };
    settings.validate()?;
    let global_mode = config.get("mode").and_then(Value::as_str).unwrap_or("rule").to_owned();
    let sub_rules = config.get("sub-rules").and_then(Value::as_mapping);
    let outbounds: HashSet<&str> = ["proxies", "proxy-groups"]
        .into_iter()
        .filter_map(|key| config.get(key).and_then(Value::as_sequence))
        .flatten()
        .filter_map(|entry| entry.get("name").and_then(Value::as_str))
        .collect();
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    let applications: Vec<_> = settings.applications.iter().map(|application| {
        let proxy = application.proxy.as_deref().map(|proxy| if outbounds.contains(proxy) { proxy } else { "REJECT" }).unwrap_or(if application.routing_mode == crate::config::application_proxy::ApplicationRoutingMode::Rule { "" } else { "REJECT" });
        serde_json::json!({"path": application.path, "kind": application.kind, "enabled": application.enabled, "mode": application.routing_mode, "proxy": proxy, "rules": application.profile_uid.as_ref().map(|uid| application_profiles::root_rules(uid)), "profile_missing": application.profile_uid.as_ref().is_some_and(|uid| !sub_rules.is_some_and(|rules| rules.contains_key(Value::from(application_profiles::root_rules(uid)))))})
    }).collect();
    let overrides: Vec<_> = crate::core::application_processes::overrides().into_iter().map(|setting| {
        let proxy = setting.proxy.as_deref().map(|proxy| if outbounds.contains(proxy) { proxy } else { "REJECT" }).unwrap_or(if setting.routing_mode == crate::config::application_proxy::ApplicationRoutingMode::Rule { "" } else { "REJECT" });
        serde_json::json!({"pid": setting.pid, "start_time": setting.start_time, "enabled": setting.enabled, "mode": setting.routing_mode, "proxy": proxy, "rules": setting.profile_uid.as_ref().map(|uid| application_profiles::root_rules(uid)), "profile_missing": setting.profile_uid.as_ref().is_some_and(|uid| !sub_rules.is_some_and(|rules| rules.contains_key(Value::from(application_profiles::root_rules(uid)))))})
    }).collect();
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(
        &serde_json::json!({"applications": applications, "overrides": overrides, "fallback": "global", "global_mode": global_mode, "tun_only": true}),
    )?);
    let mut rules = vec![
        Value::from(format!("APPLICATION-PROCESS,{payload},DIRECT")),
        // A forced proxy that cannot carry UDP must not fall through to subscription rules.
        Value::from(format!("APPLICATION-PROCESS,{payload},REJECT")),
    ];
    rules.extend(
        config
            .get("rules")
            .and_then(Value::as_sequence)
            .cloned()
            .unwrap_or_default(),
    );
    rules.push(Value::from("MATCH,DIRECT"));
    config.insert("rules".into(), Value::Sequence(rules));
    config.insert("mode".into(), "rule".into());
    config.insert("find-process-mode".into(), "always".into());
    Ok(config)
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "tests assert by panicking")]
mod tests {
    use super::*;
    use crate::config::application_proxy::{ApplicationMatchKind, ApplicationProxyEntry};

    #[test]
    fn app_gate_preserves_subscription_rules_and_blocks_missing_outbounds() -> Result<()> {
        let config: Mapping = serde_yaml_ng::from_str(
            "mode: global\nfind-process-mode: off\nrules: [\"DOMAIN-KEYWORD,baidu,DIRECT\", \"MATCH,Exit\"]\nproxy-groups: [{name: Exit, type: select, proxies: [DIRECT]}]\ndns: {nameserver: [192.0.2.53]}",
        )?;
        let settings = ApplicationProxyConfig {
            enabled: true,
            applications: vec![ApplicationProxyEntry {
                name: "Browser".into(),
                path: "/usr/local/bin/browser".into(),
                kind: ApplicationMatchKind::Executable,
                enabled: true,
                routing_mode: crate::config::application_proxy::ApplicationRoutingMode::Rule,
                proxy: None,
                profile_uid: None,
            }],
        };
        assert_eq!(use_application_proxy(config.clone(), None)?, config);
        let result = use_application_proxy(config.clone(), Some(&settings))?;
        assert_eq!(result["mode"], "rule");
        assert_eq!(result["dns"], config["dns"]);
        let rules = result["rules"].as_sequence().expect("generated rules");
        assert!(rules[0].as_str().expect("gate").ends_with(",DIRECT"));
        assert!(rules[1].as_str().expect("UDP guard").ends_with(",REJECT"));
        assert_eq!(rules[2], "DOMAIN-KEYWORD,baidu,DIRECT");
        assert_eq!(rules[3], "MATCH,Exit");
        let mut missing = settings;
        missing.applications[0].routing_mode = crate::config::application_proxy::ApplicationRoutingMode::Proxy;
        missing.applications[0].proxy = Some("Removed".into());
        let result = use_application_proxy(config, Some(&missing))?;
        use base64::Engine as _;
        let payload = result["rules"][0]
            .as_str()
            .expect("gate")
            .split(',')
            .nth(1)
            .expect("payload");
        let payload: serde_json::Value =
            serde_json::from_slice(&base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload)?)?;
        assert_eq!(payload["applications"][0]["proxy"], "REJECT");
        assert_eq!(payload["applications"][0]["mode"], "proxy");
        Ok(())
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn executable_override_and_literal_bundle_boundaries_survive_restart() -> Result<()> {
        let settings = ApplicationProxyConfig {
            enabled: true,
            applications: vec![
                ApplicationProxyEntry {
                    name: "Browser".into(),
                    path: "/Applications/Browser[work],1.app".into(),
                    kind: ApplicationMatchKind::Bundle,
                    enabled: false,
                    routing_mode: crate::config::application_proxy::ApplicationRoutingMode::Rule,
                    proxy: None,
                    profile_uid: None,
                },
                ApplicationProxyEntry {
                    name: "Helper".into(),
                    path: "/Applications/Browser[work],1.app/Contents/MacOS/Helper".into(),
                    kind: ApplicationMatchKind::Executable,
                    enabled: false,
                    routing_mode: crate::config::application_proxy::ApplicationRoutingMode::Rule,
                    proxy: None,
                    profile_uid: None,
                },
            ],
        };
        let restarted = serde_yaml_ng::from_str(&serde_yaml_ng::to_string(&settings)?)?;
        let result = use_application_proxy(Mapping::new(), Some(&restarted))?;
        assert!(
            result["rules"][0]
                .as_str()
                .expect("gate")
                .starts_with("APPLICATION-PROCESS,")
        );
        let expression = settings.applications[0].rule_prefix();
        let regex = regex::Regex::new(expression.strip_prefix("PROCESS-PATH-REGEX,").expect("prefix"))?;
        assert!(regex.is_match(settings.applications[1].path.as_str()));
        assert!(!regex.is_match("/Applications/Browserw,1.app/Contents/MacOS/Helper"));
        assert!(!regex.is_match("/Applications/Browser[work],1.app.other/Helper"));
        Ok(())
    }
}
