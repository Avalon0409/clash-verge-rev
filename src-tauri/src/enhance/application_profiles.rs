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

#[cfg(target_os = "macos")]
fn replace_system_servers(dns: &mut Value, servers: &[Value]) {
    match dns {
        Value::Sequence(values) => {
            let old = std::mem::take(values);
            for mut value in old {
                if value.as_str() == Some("system") {
                    values.extend_from_slice(servers);
                } else {
                    replace_system_servers(&mut value, servers);
                    values.push(value);
                }
            }
        }
        Value::Mapping(map) => {
            for value in map.values_mut() {
                replace_system_servers(value, servers);
            }
        }
        Value::String(value) if value == "system" => *dns = Value::Sequence(servers.to_vec()),
        _ => {}
    }
}

#[cfg(target_os = "macos")]
pub fn preserve_system_dns(dns: &mut Mapping, servers: &[Value]) {
    for key in [
        "nameserver",
        "default-nameserver",
        "fallback",
        "proxy-server-nameserver",
        "direct-nameserver",
        "nameserver-policy",
        "proxy-server-nameserver-policy",
    ] {
        if let Some(value) = dns.get_mut(key) {
            replace_system_servers(value, servers);
        }
    }
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
    fields.join(",")
}

fn rename_group_members(
    group: &mut Mapping,
    uid: &str,
    proxies: &HashMap<std::string::String, std::string::String>,
    providers: &HashMap<std::string::String, std::string::String>,
) {
    group.insert("hidden".into(), true.into());
    if let Some(names) = group.get_mut("proxies").and_then(Value::as_sequence_mut) {
        for name in names {
            if let Some(value) = name.as_str().map(ToOwned::to_owned)
                && !matches!(value.as_str(), "DIRECT" | "REJECT" | "REJECT-DROP" | "PASS")
            {
                *name = Value::from(proxies.get(&value).cloned().unwrap_or_else(|| alias(uid, &value)));
            }
        }
    }
    if let Some(names) = group.get_mut("use").and_then(Value::as_sequence_mut) {
        for name in names {
            rename(name, providers);
        }
    }
}

fn rename_dns_nameservers(value: &mut Value, proxies: &HashMap<std::string::String, std::string::String>) {
    if let Some(servers) = value.as_sequence_mut() {
        for server in servers {
            rename_dns_nameservers(server, proxies);
        }
    } else if let Some(value_str) = value.as_str()
        && let Some((address, fragment)) = value_str.split_once('#')
    {
        let fragments: Vec<_> = fragment
            .split('&')
            .map(|part| proxies.get(part).map_or_else(|| part.to_owned(), Clone::clone))
            .collect();
        *value = Value::from(format!("{address}#{}", fragments.join("&")));
    }
}

pub fn namespace_dns(
    dns: &mut Mapping,
    proxies: &HashMap<std::string::String, std::string::String>,
    rule_providers: &HashMap<std::string::String, std::string::String>,
) {
    for key in [
        "default-nameserver",
        "nameserver",
        "fallback",
        "proxy-server-nameserver",
        "direct-nameserver",
    ] {
        if let Some(servers) = dns.get_mut(key) {
            rename_dns_nameservers(servers, proxies);
        }
    }
    for key in ["nameserver-policy", "proxy-server-nameserver-policy"] {
        if let Some(policies) = dns.get_mut(key).and_then(Value::as_mapping_mut) {
            let old = std::mem::take(policies);
            for (key, mut value) in old {
                rename_dns_nameservers(&mut value, proxies);
                if let Some(names) = key.as_str().and_then(|key| key.strip_prefix("rule-set:")) {
                    for name in names.split(',').map(str::trim) {
                        let name = rule_providers.get(name).map_or(name, std::string::String::as_str);
                        policies.insert(Value::from(format!("rule-set:{name}")), value.clone());
                    }
                } else {
                    policies.insert(key, value);
                }
            }
        }
    }
}

fn import_dns(
    target: &mut Mapping,
    uid: &str,
    source: &Mapping,
    proxies: &HashMap<std::string::String, std::string::String>,
    rule_providers: &HashMap<std::string::String, std::string::String>,
) -> Result<()> {
    if let Some(mut dns) = source.get("dns").and_then(Value::as_mapping).cloned() {
        namespace_dns(&mut dns, proxies, rule_providers);
        dns.insert("enhanced-mode".into(), "redir-host".into());
        dns.remove("listen");
        target
            .entry("application-dns".into())
            .or_insert_with(|| Value::Mapping(Mapping::new()))
            .as_mapping_mut()
            .ok_or_else(|| anyhow::anyhow!("invalid application DNS mapping"))?
            .insert(Value::from(root_rules(uid)), Value::Mapping(dns));
    }
    Ok(())
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
    import_dns(target, uid, &source, &proxies, &rule_providers)?;
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
                    rename_group_members(map, uid, &proxies, &proxy_providers);
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

    #[test]
    fn subscription_dns_keeps_its_proxy_and_policy_namespace() -> Result<()> {
        let mut global = Mapping::new();
        let source: Mapping = serde_yaml_ng::from_str(
            "proxies: [{name: Exit, type: http, server: 127.0.0.1, port: 1002}]\nproxy-groups: []\nrule-providers: {Company: {type: inline, behavior: domain, payload: [example.test]}}\ndns: {enable: true, nameserver: ['https://1.1.1.1/dns-query#Exit&ecs=1.2.3.4'], nameserver-policy: {'rule-set:Company': ['https://8.8.8.8/dns-query#Exit']}}",
        )?;
        import_profile(&mut global, "one", source)?;
        let dns = &global["application-dns"][root_rules("one")];
        assert_eq!(
            dns["nameserver"][0],
            format!("https://1.1.1.1/dns-query#{}&ecs=1.2.3.4", alias("one", "Exit"))
        );
        assert_eq!(
            dns["nameserver-policy"][format!("rule-set:{}", alias("one", "Company"))][0],
            format!("https://8.8.8.8/dns-query#{}", alias("one", "Exit"))
        );
        assert_eq!(dns["enhanced-mode"], "redir-host");
        Ok(())
    }
}
