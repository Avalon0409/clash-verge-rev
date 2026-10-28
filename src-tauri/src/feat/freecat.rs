use crate::{
    config::{PrfAccount, PrfOption},
    utils::network::{HttpResponse, NetworkManager, ProxyType},
};
use anyhow::{Context as _, Result, anyhow, bail, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use tauri::Url;

pub const DEFAULT_USER_AGENT: &str = "freecatencua/v3.1.0 clash-verge Platform/macos";
const KEYCHAIN_SERVICE: &str = "io.github.clash-verge-rev.subscription-account";
const VERIFIED_CORE_HASH: &str = "5c715dd468ea8e978de95fcbd852d31525e51e1299be525505ed90729e9d52f6";

#[derive(Deserialize)]
pub struct AccountCredentials {
    pub password: std::string::String,
}

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct AccountSecrets {
    password: std::string::String,
    auth_data: std::string::String,
    key: Vec<u8>,
    iv: Vec<u8>,
}

pub fn validate_account(account: &PrfAccount) -> Result<()> {
    ensure!(
        account.provider == "freecat",
        "Unsupported subscription account provider"
    );
    ensure!(!account.email.trim().is_empty(), "Account email is required");
    let url = Url::parse(account.server_url.trim()).map_err(|_| anyhow!("Invalid account server address"))?;
    ensure!(
        url.scheme() == "https"
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "Account server must be an HTTPS address without credentials, query or fragment"
    );
    ensure!(
        account.client_user_agent.starts_with(
            DEFAULT_USER_AGENT
                .split_once(' ')
                .context("Invalid default client user agent")?
                .0
        ) && !account.client_user_agent.contains(['\r', '\n']),
        "Only the verified Freecat 3.1.0 subscription format is supported"
    );
    Ok(())
}

fn endpoint(account: &PrfAccount, action: &str) -> std::string::String {
    let base = account.server_url.trim().trim_end_matches('/');
    let base = base.strip_suffix("/api/v1").unwrap_or(base);
    format!("{base}/api/v1/{action}")
}

pub(crate) async fn read_secrets(id: &str) -> Result<AccountSecrets> {
    let id = id.to_owned();
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        {
            let bytes = security_framework::passwords::get_generic_password(KEYCHAIN_SERVICE, &id)
                .map_err(|_| anyhow!("Account credentials are unavailable in Keychain; sign in again"))?;
            serde_json::from_slice(&bytes).map_err(|_| anyhow!("Stored account credentials are invalid"))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = id;
            bail!("Account subscription credential storage currently requires macOS")
        }
    })
    .await
    .map_err(|_| anyhow!("Credential storage task failed"))?
}

pub(crate) async fn write_secrets(id: &str, secrets: &AccountSecrets) -> Result<()> {
    let id = id.to_owned();
    let bytes = serde_json::to_vec(secrets).map_err(|_| anyhow!("Unable to encode account credentials"))?;
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        {
            security_framework::passwords::set_generic_password(KEYCHAIN_SERVICE, &id, &bytes)
                .map_err(|_| anyhow!("Unable to save account credentials in Keychain"))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (id, bytes);
            bail!("Account subscription credential storage currently requires macOS")
        }
    })
    .await
    .map_err(|_| anyhow!("Credential storage task failed"))?
}

pub(crate) async fn delete_secrets(id: &str) -> Result<()> {
    let id = id.to_owned();
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        {
            match security_framework::passwords::delete_generic_password(KEYCHAIN_SERVICE, &id) {
                Ok(()) => Ok(()),
                Err(error) if error.code() == -25300 => Ok(()),
                Err(_) => bail!("Unable to remove account credentials from Keychain"),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = id;
            Ok(())
        }
    })
    .await
    .map_err(|_| anyhow!("Credential storage task failed"))?
}

pub(crate) fn prepare_secrets(credentials: AccountCredentials) -> Result<AccountSecrets> {
    ensure!(!credentials.password.is_empty(), "Account password is required");
    Ok(AccountSecrets {
        password: credentials.password,
        ..Default::default()
    })
}

pub(crate) fn update_credentials(
    mut secrets: AccountSecrets,
    credentials: AccountCredentials,
) -> Result<AccountSecrets> {
    ensure!(!credentials.password.is_empty(), "Account password is required");
    secrets.password = credentials.password;
    secrets.auth_data.clear();
    Ok(secrets)
}

pub(crate) fn require_login(secrets: &mut AccountSecrets) {
    secrets.auth_data.clear();
}

fn transport(option: Option<&PrfOption>) -> ProxyType {
    if option.is_some_and(|o| o.self_proxy.unwrap_or(false)) {
        ProxyType::Localhost
    } else if option.is_some_and(|o| o.with_proxy.unwrap_or(false)) {
        ProxyType::System
    } else {
        ProxyType::None
    }
}

async fn login(account: &PrfAccount, secrets: &mut AccountSecrets, option: Option<&PrfOption>) -> Result<()> {
    let (status, data) = NetworkManager::new()
        .account_json(
            &endpoint(account, "passport/auth/login"),
            Some(&json!({"email": account.email, "password": secrets.password})),
            None,
            transport(option),
            option.and_then(|o| o.timeout_seconds),
            account.client_user_agent.clone(),
        )
        .await?;
    ensure!(
        status.is_success(),
        "Account login failed (HTTP {status}); check the server and credentials"
    );
    secrets.auth_data = data
        .get("data")
        .and_then(|d| d.get("auth_data"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            anyhow!("Account login returned no session; check the credentials or additional login requirements")
        })?
        .to_owned();
    Ok(())
}

pub(crate) async fn fetch(
    account: &PrfAccount,
    option: Option<&PrfOption>,
    mut secrets: AccountSecrets,
) -> Result<(HttpResponse, AccountSecrets)> {
    validate_account(account)?;
    if secrets.key.is_empty() || secrets.iv.is_empty() {
        let (key, iv) = match read_secrets("freecat.protocol.v3.1.0").await {
            Ok(material) if material.key.len() == 16 && material.iv.len() == 16 => (material.key, material.iv),
            _ => {
                let (key, iv) = tokio::task::spawn_blocking(import_protocol_material)
                    .await
                    .map_err(|_| anyhow!("Protocol import task failed"))??;
                let material = AccountSecrets {
                    key: key.clone(),
                    iv: iv.clone(),
                    ..Default::default()
                };
                write_secrets("freecat.protocol.v3.1.0", &material).await?;
                (key, iv)
            }
        };
        secrets.key = key;
        secrets.iv = iv;
    }
    if secrets.auth_data.is_empty() {
        login(account, &mut secrets, option).await?;
    }
    let manager = NetworkManager::new();
    let timeout = option.and_then(|o| o.timeout_seconds);
    let query = endpoint(account, "user/getSubscribe");
    let (mut status, mut data) = manager
        .account_json(
            &query,
            None,
            Some(&secrets.auth_data),
            transport(option),
            timeout,
            account.client_user_agent.clone(),
        )
        .await?;
    if matches!(status.as_u16(), 401 | 403) {
        login(account, &mut secrets, option).await?;
        (status, data) = manager
            .account_json(
                &query,
                None,
                Some(&secrets.auth_data),
                transport(option),
                timeout,
                account.client_user_agent.clone(),
            )
            .await?;
    }
    ensure!(
        status.is_success(),
        "Account subscription query failed (HTTP {status}); sign in again if the session expired"
    );
    let url = data
        .get("data")
        .and_then(|d| d.get("subscribe_url"))
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Account response contains no subscription URL"))?;
    let parsed = Url::parse(url).map_err(|_| anyhow!("Account returned an invalid subscription URL"))?;
    ensure!(
        parsed.scheme() == "https"
            && parsed.host_str().is_some()
            && parsed.username().is_empty()
            && parsed.password().is_none(),
        "Account returned an unsafe subscription URL"
    );
    let response = manager
        .get(
            url,
            transport(option),
            Some(timeout.unwrap_or(20)),
            Some(account.client_user_agent.clone()),
            false,
        )
        .await
        .map_err(|_| anyhow!("Freecat subscription download failed"))?;
    ensure!(
        response.status().is_success(),
        "Freecat subscription download failed (HTTP {})",
        response.status()
    );
    let yaml = decode_subscription(response.text(), &secrets.key, &secrets.iv)?;
    Ok((
        HttpResponse::new(response.status(), response.headers().clone(), yaml.into()),
        secrets,
    ))
}

fn decode_subscription(raw: &str, key: &[u8], iv: &[u8]) -> Result<std::string::String> {
    ensure!(
        raw.len() <= 16 * 1024 * 1024,
        "Freecat subscription exceeds the size limit"
    );
    let ciphertext = STANDARD
        .decode(raw.trim())
        .map_err(|_| anyhow!("Expected an encrypted Freecat subscription; check the client User-Agent"))?;
    let plaintext = decrypt_cbc(&ciphertext, key, iv)?;
    let yaml = STANDARD
        .decode(plaintext)
        .map_err(|_| anyhow!("Invalid Freecat inner subscription encoding"))?;
    let yaml = std::string::String::from_utf8(yaml).map_err(|_| anyhow!("Freecat subscription is not UTF-8"))?;
    let mapping = serde_yaml_ng::from_str::<serde_yaml_ng::Mapping>(&yaml)
        .map_err(|_| anyhow!("Decoded Freecat subscription is invalid YAML"))?;
    ensure!(
        mapping.contains_key("proxies") || mapping.contains_key("proxy-providers"),
        "Decoded subscription has no proxy configuration"
    );
    Ok(yaml)
}

#[cfg(target_os = "macos")]
fn decrypt_cbc(ciphertext: &[u8], key: &[u8], iv: &[u8]) -> Result<Vec<u8>> {
    use std::ffi::c_void;
    #[link(name = "System")]
    unsafe extern "C" {
        fn CCCrypt(
            op: u32,
            algorithm: u32,
            options: u32,
            key: *const c_void,
            key_len: usize,
            iv: *const c_void,
            input: *const c_void,
            input_len: usize,
            output: *mut c_void,
            output_len: usize,
            moved: *mut usize,
        ) -> i32;
    }
    ensure!(
        key.len() == 16 && iv.len() == 16 && !ciphertext.is_empty() && ciphertext.len().is_multiple_of(16),
        "Invalid Freecat encrypted subscription or protocol material"
    );
    let mut output = vec![0u8; ciphertext.len() + 16];
    let mut moved = 0;
    // CommonCrypto validates PKCS7; all buffers remain owned for the duration of this call.
    let status = unsafe {
        CCCrypt(
            1,
            0,
            1,
            key.as_ptr().cast(),
            key.len(),
            iv.as_ptr().cast(),
            ciphertext.as_ptr().cast(),
            ciphertext.len(),
            output.as_mut_ptr().cast(),
            output.len(),
            &mut moved,
        )
    };
    ensure!(
        status == 0 && moved <= output.len(),
        "Freecat subscription decryption failed; the server may use a different protocol version"
    );
    output.truncate(moved);
    Ok(output)
}

#[cfg(not(target_os = "macos"))]
fn decrypt_cbc(_ciphertext: &[u8], _key: &[u8], _iv: &[u8]) -> Result<Vec<u8>> {
    bail!("Freecat encrypted subscriptions currently require macOS")
}

fn import_protocol_material() -> Result<(Vec<u8>, Vec<u8>)> {
    let binary = std::fs::read("/Applications/自由猫.app/Contents/MacOS/ziyoumaoCore")
        .map_err(|_| anyhow!("First-time Freecat setup requires the verified Freecat 3.1.0 app installed in Applications to import its protocol material"))?;
    extract_protocol_material(&binary)
}

fn extract_protocol_material(binary: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    fn u32_be(bytes: &[u8], at: usize) -> Result<u32> {
        Ok(u32::from_be_bytes(
            bytes
                .get(at..at + 4)
                .ok_or_else(|| anyhow!("Invalid Core archive"))?
                .try_into()?,
        ))
    }
    let binary = if binary.starts_with(&[0xca, 0xfe, 0xba, 0xbe]) {
        let count = u32_be(binary, 4)? as usize;
        ensure!(count <= 16, "Invalid Core architecture count");
        let mut selected = None;
        for index in 0..count {
            let at = 8 + index * 20;
            if u32_be(binary, at)? == 0x0100000c {
                let offset = u32_be(binary, at + 8)? as usize;
                let size = u32_be(binary, at + 12)? as usize;
                selected = binary.get(offset..offset.checked_add(size).ok_or_else(|| anyhow!("Invalid Core slice"))?);
            }
        }
        selected.ok_or_else(|| anyhow!("Freecat Core has no verified arm64 slice"))?
    } else {
        binary
    };
    ensure!(
        Sha256::digest(binary)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<std::string::String>()
            == VERIFIED_CORE_HASH,
        "Freecat Core version is not verified; protocol material must be reanalyzed before importing"
    );
    fn read_u64(binary: &[u8], offset: usize) -> Result<u64> {
        Ok(u64::from_le_bytes(
            binary
                .get(offset..offset + 8)
                .ok_or_else(|| anyhow!("Invalid Core material location"))?
                .try_into()?,
        ))
    }
    fn read_u32(binary: &[u8], offset: usize) -> Result<u32> {
        Ok(u32::from_le_bytes(
            binary
                .get(offset..offset + 4)
                .ok_or_else(|| anyhow!("Invalid Core load command"))?
                .try_into()?,
        ))
    }
    let mut segments = Vec::new();
    let mut position = 32;
    for _ in 0..read_u32(binary, 16)? {
        if read_u32(binary, position)? == 0x19 {
            segments.push((
                read_u64(binary, position + 24)?,
                read_u64(binary, position + 40)?,
                read_u64(binary, position + 48)?,
            ));
        }
        position += read_u32(binary, position + 4)? as usize;
    }
    let bytes_at = |address: u64, length: usize| -> Result<&[u8]> {
        for &(start, offset, size) in &segments {
            if address >= start && address - start + length as u64 <= size {
                let begin = (offset + address - start) as usize;
                return binary
                    .get(begin..begin + length)
                    .ok_or_else(|| anyhow!("Invalid Core material range"));
            }
        }
        bail!("Core protocol material is not mapped")
    };
    let hex_at = |address| -> Result<Vec<u8>> {
        let descriptor = bytes_at(address, 16)?;
        let pointer = read_u64(descriptor, 0)?;
        let length = read_u64(descriptor, 8)? as usize;
        ensure!(
            length <= 128 && length.is_multiple_of(2),
            "Invalid protocol material length"
        );
        let source = bytes_at(pointer, length)?;
        source
            .as_chunks::<2>()
            .0
            .iter()
            .map(|chunk| {
                let hex = std::str::from_utf8(chunk).map_err(|_| anyhow!("Invalid protocol material encoding"))?;
                u8::from_str_radix(hex, 16).map_err(|_| anyhow!("Invalid protocol material encoding"))
            })
            .collect()
    };
    let mask = hex_at(0x101d19280)?;
    ensure!(mask.len() == 4, "Invalid protocol material mask");
    let assemble = |address: u64| -> Result<Vec<u8>> {
        let mut result = Vec::with_capacity(16);
        for part in 0u64..4 {
            let bytes = hex_at(address + part * 16)?;
            ensure!(bytes.len() == 4, "Invalid protocol material part");
            result.extend(bytes.iter().zip(&mask).map(|(value, mask)| value ^ mask));
        }
        Ok(result)
    };
    Ok((assemble(0x101d19200)?, assemble(0x101d19240)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_server_rejects_credentials_and_insecure_urls() {
        for server in [
            "http://example.invalid",
            "https://user:pass@example.invalid",
            "https://example.invalid?token=test",
        ] {
            let account = PrfAccount {
                server_url: server.into(),
                email: "test@example.invalid".into(),
                client_user_agent: DEFAULT_USER_AGENT.into(),
                provider: "freecat".into(),
                credential_id: "test".into(),
            };
            assert!(validate_account(&account).is_err());
        }
    }

    #[tokio::test]
    #[ignore = "requires an explicitly supplied private test account file"]
    async fn freecat_live_login_and_decode() -> Result<()> {
        let file = std::env::var("FREECAT_TEST_ACCOUNT_FILE")?;
        let data: Value = serde_json::from_slice(&std::fs::read(file)?)?;
        let account = PrfAccount {
            provider: "freecat".into(),
            server_url: data["server"]
                .as_str()
                .context("Private test account is missing server")?
                .into(),
            email: data["email"]
                .as_str()
                .context("Private test account is missing email")?
                .into(),
            credential_id: "test".into(),
            client_user_agent: DEFAULT_USER_AGENT.into(),
        };
        let secrets = prepare_secrets(AccountCredentials {
            password: data["password"]
                .as_str()
                .context("Private test account is missing password")?
                .to_owned(),
        })?;
        let (response, _) = fetch(&account, None, secrets).await?;
        let mapping: serde_yaml_ng::Mapping = serde_yaml_ng::from_str(response.text())?;
        ensure!(
            !mapping["proxies"]
                .as_sequence()
                .context("Decoded subscription is missing proxies")?
                .is_empty(),
            "No proxies decoded"
        );
        Ok(())
    }
}
