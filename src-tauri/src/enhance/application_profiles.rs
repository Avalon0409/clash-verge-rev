use anyhow::{Result, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Serialize;
use serde_yaml_ng::{Mapping, Value};
use std::collections::HashMap;

pub fn prefix(uid: &str) -> std::string::String {
    crate::config::application_proxy::subscription_prefix(uid)
}

pub fn alias(uid: &str, name: &str) -> std::string::String {
    format!("{}{name}", prefix(uid))
}
pub fn root_rules(uid: &str) -> std::string::String {
    alias(uid, "__root_rules")
}

#[derive(Serialize)]
pub struct OutboundOption {
    pub name: std::string::String,
    pub value: std::string::String,
}

pub async fn catalog(uid: &str) -> Result<Vec<OutboundOption>> {
    let mut profiles = (**crate::config::Config::profiles().await.data_arc()).clone();
    let item = profiles.get_item(uid)?;
    ensure!(
        matches!(item.itype.as_deref(), Some("remote" | "local")),
        "select a subscription profile"
    );
    profiles.current = Some(uid.into());
    let config = super::enhance_profile(&profiles, false).await?.0;
    Ok(["proxy-groups", "proxies"]
        .into_iter()
        .filter_map(|key| config.get(key).and_then(Value::as_sequence))
        .flatten()
        .filter_map(|node| node.get("name").and_then(Value::as_str))
        .filter(|name| !name.contains(',') && !matches!(*name, "DIRECT" | "GLOBAL" | "REJECT" | "REJECT-DROP" | "PASS"))
        .map(|name| OutboundOption {
            name: name.into(),
            value: alias(uid, name),
        })
        .collect())
}

fn rename(value: &mut Value, names: &HashMap<std::string::String, std::string::String>) {
    if let Some(name) = value.as_str().and_then(|name| names.get(name)) {
        *value = Value::from(name.clone());
    }
}

fn rename_rule(
    line: &str,
    proxies: &HashMap<std::string::String, std::string::String>,
    providers: &HashMap<std::string::String, std::string::String>,
    subs: &HashMap<std::string::String, std::string::String>,
) -> std::string::String {
    let mut fields: Vec<_> = line.split(',').map(str::trim).map(ToOwned::to_owned).collect();
    let kind = fields.first().map(|value| value.to_uppercase()).unwrap_or_default();
    let target = if kind == "MATCH" {
        1
    } else if matches!(
        kind.as_str(),
        "AND" | "OR" | "NOT" | "SUB-RULE" | "DOMAIN-REGEX" | "PROCESS-NAME-REGEX" | "PROCESS-PATH-REGEX"
    ) {
        fields.len().saturating_sub(1)
    } else {
        2
    };
    if let Some(name) = fields.get_mut(target) {
        let names = if kind == "SUB-RULE" { subs } else { proxies };
        if let Some(alias) = names.get(name) {
            *name = alias.clone();
        }
    }
    for index in 0..fields.len().saturating_sub(1) {
        if fields[index].trim_start_matches('(').eq_ignore_ascii_case("RULE-SET") {
            let value = &fields[index + 1];
            let end = value.find(')').unwrap_or(value.len());
            if let Some(alias) = providers.get(value[..end].trim()) {
                fields[index + 1] = format!("{alias}{}", &value[end..]);
            }
        }
    }
    let result = fields.join(",");
    result
}

pub fn import_profile(target: &mut Mapping, uid: &str, source: Mapping) -> Result<()> {
    let mut proxies = HashMap::new();
    for key in ["proxies", "proxy-groups"] {
        for entry in source.get(key).and_then(Value::as_sequence).into_iter().flatten() {
            if let Some(name) = entry.get("name").and_then(Value::as_str) {
                proxies.insert(name.to_owned(), alias(uid, name));
            }
        }
    }
    // Foreign GLOBAL refers to the foreign profile's own proxy group.
    let global_name = alias(uid, "GLOBAL");
    proxies.insert("GLOBAL".into(), global_name.clone());
    let proxy_providers: HashMap<_, _> = source
        .get("proxy-providers")
        .and_then(Value::as_mapping)
        .into_iter()
        .flat_map(|entries| entries.keys())
        .filter_map(Value::as_str)
        .map(|name| (name.to_owned(), alias(uid, name)))
        .collect();
    let rule_providers: HashMap<_, _> = source
        .get("rule-providers")
        .and_then(Value::as_mapping)
        .into_iter()
        .flat_map(|entries| entries.keys())
        .filter_map(Value::as_str)
        .map(|name| (name.to_owned(), alias(uid, name)))
        .collect();
    let subs: HashMap<_, _> = source
        .get("sub-rules")
        .and_then(Value::as_mapping)
        .into_iter()
        .flat_map(|entries| entries.keys())
        .filter_map(Value::as_str)
        .map(|name| (name.to_owned(), alias(uid, name)))
        .collect();
    for key in ["proxies", "proxy-groups"] {
        let mut entries = source
            .get(key)
            .and_then(Value::as_sequence)
            .cloned()
            .unwrap_or_default();
        for entry in &mut entries {
            if let Some(map) = entry.as_mapping_mut() {
                if let Some(name) = map.get_mut(Value::from("name")) {
                    rename(name, &proxies);
                }
                if let Some(value) = map.get_mut(Value::from("dialer-proxy")) {
                    rename(value, &proxies);
                }
                if key == "proxy-groups" {
                    map.insert("hidden".into(), true.into());
                    if let Some(names) = map.get_mut(Value::from("proxies")).and_then(Value::as_sequence_mut) {
                        for name in names {
                            if let Some(value) = name.as_str().map(ToOwned::to_owned) {
                                if !matches!(value.as_str(), "DIRECT" | "REJECT" | "REJECT-DROP" | "PASS") {
                                    *name =
                                        Value::from(proxies.get(&value).cloned().unwrap_or_else(|| alias(uid, &value)));
                                }
                            }
                        }
                    }
                    if let Some(names) = map.get_mut(Value::from("use")).and_then(Value::as_sequence_mut) {
                        for name in names {
                            rename(name, &proxy_providers);
                        }
                    }
                }
            }
        }
        let dest = target
            .entry(Value::from(key))
            .or_insert_with(|| Value::Sequence(vec![]))
            .as_sequence_mut()
            .ok_or_else(|| anyhow::anyhow!("invalid outbound list"))?;
        for entry in entries {
            let name = entry.get("name");
            ensure!(
                !dest.iter().any(|old| old.get("name") == name),
                "subscription outbound namespace collision"
            );
            dest.push(entry);
        }
    }
    let all_members: Vec<_> = source
        .get("proxies")
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(|node| node.get("name").and_then(Value::as_str))
        .map(|name| Value::from(alias(uid, name)))
        .collect();
    let mut group = Mapping::new();
    group.insert("name".into(), Value::from(global_name));
    group.insert("type".into(), "select".into());
    group.insert("hidden".into(), true.into());
    group.insert(
        "proxies".into(),
        Value::Sequence(if all_members.is_empty() && proxy_providers.is_empty() {
            vec![Value::from("DIRECT")]
        } else {
            all_members
        }),
    );
    if !proxy_providers.is_empty() {
        group.insert(
            "use".into(),
            Value::Sequence(proxy_providers.values().cloned().map(Value::from).collect()),
        );
    }
    target
        .get_mut("proxy-groups")
        .and_then(Value::as_sequence_mut)
        .ok_or_else(|| anyhow::anyhow!("invalid group list"))?
        .push(Value::Mapping(group));
    for (key, names) in [
        ("proxy-providers", &proxy_providers),
        ("rule-providers", &rule_providers),
    ] {
        for (name, definition) in source
            .get(key)
            .and_then(Value::as_mapping)
            .into_iter()
            .flat_map(|entries| entries.iter())
        {
            let Some(name) = name.as_str() else { continue };
            let mut definition = definition.clone();
            if let Some(map) = definition.as_mapping_mut() {
                if key == "proxy-providers" {
                    let override_settings = map
                        .entry("override".into())
                        .or_insert_with(|| Value::Mapping(Mapping::new()))
                        .as_mapping_mut()
                        .ok_or_else(|| anyhow::anyhow!("invalid provider override"))?;
                    let old = override_settings
                        .get("additional-prefix")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned();
                    override_settings.insert("additional-prefix".into(), Value::from(format!("{}{old}", prefix(uid))));
                    if let Some(proxy) = override_settings.get_mut(Value::from("dialer-proxy")) {
                        rename(proxy, &proxies);
                    }
                }
                if map.get("type").and_then(Value::as_str) == Some("http") {
                    map.insert(
                        "path".into(),
                        Value::from(format!(
                            "./providers/app-{}/{}/{}",
                            URL_SAFE_NO_PAD.encode(uid),
                            key,
                            URL_SAFE_NO_PAD.encode(name)
                        )),
                    );
                }
                if let Some(proxy) = map.get_mut(Value::from("proxy")) {
                    rename(proxy, &proxies);
                }
            }
            let dest = target
                .entry(Value::from(key))
                .or_insert_with(|| Value::Mapping(Mapping::new()))
                .as_mapping_mut()
                .ok_or_else(|| anyhow::anyhow!("invalid provider mapping"))?;
            dest.insert(Value::from(names[name].clone()), definition);
        }
    }
    let dest = target
        .entry("sub-rules".into())
        .or_insert_with(|| Value::Mapping(Mapping::new()))
        .as_mapping_mut()
        .ok_or_else(|| anyhow::anyhow!("invalid sub-rules mapping"))?;
    for (name, lines) in source
        .get("sub-rules")
        .and_then(Value::as_mapping)
        .into_iter()
        .flat_map(|entries| entries.iter())
    {
        if let Some(name) = name.as_str() {
            dest.insert(
                Value::from(subs[name].clone()),
                Value::Sequence(
                    lines
                        .as_sequence()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(|line| Value::from(rename_rule(line, &proxies, &rule_providers, &subs)))
                        .collect(),
                ),
            );
        }
    }
    let mut rules: Vec<_> = source
        .get("rules")
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|line| Value::from(rename_rule(line, &proxies, &rule_providers, &subs)))
        .collect();
    rules.push("MATCH,DIRECT".into());
    ensure!(
        !dest.contains_key(Value::from(root_rules(uid))),
        "subscription root rule namespace collision"
    );
    dest.insert(Value::from(root_rules(uid)), Value::Sequence(rules));
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "tests assert by panicking")]
mod tests {
    use super::*;

    #[test]
    fn subscriptions_keep_global_defaults_and_isolate_same_names() -> Result<()> {
        let mut global: Mapping = serde_yaml_ng::from_str(
            "mixed-port: 1234\ndns: {enable: false}\nproxies: [{name: Exit, type: http, server: 127.0.0.1, port: 1001}]\nproxy-groups: [{name: Main, type: select, proxies: [Exit]}]\nrules: [\"DOMAIN-KEYWORD,baidu,DIRECT\", \"MATCH,Main\"]",
        )?;
        let original = global.clone();
        let source: Mapping = serde_yaml_ng::from_str(
            "mixed-port: 9999\ndns: {enable: true}\nproxies: [{name: Exit, type: http, server: 127.0.0.1, port: 1002}]\nproxy-groups: [{name: Main, type: select, proxies: [Exit,DIRECT]}]\nrule-providers: {Same: {type: inline, behavior: domain, payload: [example.test]}}\nrules: [\"DOMAIN-KEYWORD,baidu,DIRECT\", \"DOMAIN-KEYWORD,blocked,REJECT\", \"AND,((RULE-SET,Same),(NETWORK,TCP)),Main\", \"MATCH,Main\"]",
        )?;
        import_profile(&mut global, "B", source.clone())?;
        import_profile(&mut global, "C", source)?;
        assert_eq!(global["mixed-port"], original["mixed-port"]);
        assert_eq!(global["dns"], original["dns"]);
        assert_eq!(global["rules"], original["rules"]);
        let nodes = global["proxies"].as_sequence().expect("nodes");
        assert_eq!(nodes[0]["port"], 1001);
        assert!(nodes.iter().any(|node| node["name"] == alias("B", "Exit")));
        assert!(nodes.iter().any(|node| node["name"] == alias("C", "Exit")));
        let rules = global["sub-rules"][root_rules("B")]
            .as_sequence()
            .expect("subscription rules");
        assert_eq!(rules[0], "DOMAIN-KEYWORD,baidu,DIRECT");
        assert_eq!(rules[1], "DOMAIN-KEYWORD,blocked,REJECT");
        assert_eq!(
            rules[2],
            format!(
                "AND,((RULE-SET,{}),(NETWORK,TCP)),{}",
                alias("B", "Same"),
                alias("B", "Main")
            )
        );
        assert_eq!(rules[3], format!("MATCH,{}", alias("B", "Main")));
        assert!(global["rule-providers"].get(alias("B", "Same")).is_some());
        Ok(())
    }
}
