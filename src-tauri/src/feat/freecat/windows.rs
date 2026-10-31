use super::AccountSecrets;
use anyhow::{Result, anyhow, ensure};
use windows_sys::Win32::{
    Foundation::{ERROR_NOT_FOUND, GetLastError},
    Security::{
        Credentials::{
            CRED_MAX_CREDENTIAL_BLOB_SIZE, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW,
            CredFree, CredReadW, CredWriteW,
        },
        Cryptography::{
            BCRYPT_AES_CBC_ALG_HANDLE, BCRYPT_BLOCK_PADDING, BCRYPT_KEY_HANDLE, BCryptDecrypt, BCryptDestroyKey,
            BCryptGenerateSymmetricKey,
        },
    },
};

fn target(id: &str) -> Result<Vec<u16>> {
    let target = format!("{}/{}/{id}", crate::utils::dirs::APP_ID, super::KEYCHAIN_SERVICE);
    ensure!(!target.contains('\0'), "Invalid credential identifier");
    Ok(target.encode_utf16().chain(Some(0)).collect())
}

pub(super) fn read(id: &str) -> Result<AccountSecrets> {
    let target = target(id)?;
    let mut credential = std::ptr::null_mut();
    unsafe {
        ensure!(
            CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential) != 0,
            "Account credentials are unavailable in Windows Credential Manager; sign in again"
        );
        let result = if (*credential).CredentialBlobSize == 0 {
            Err(anyhow!("Stored account credentials are invalid"))
        } else {
            let bytes =
                std::slice::from_raw_parts((*credential).CredentialBlob, (*credential).CredentialBlobSize as usize);
            serde_json::from_slice(bytes).map_err(|_| anyhow!("Stored account credentials are invalid"))
        };
        CredFree(credential.cast());
        result
    }
}

pub(super) fn write(id: &str, bytes: &[u8]) -> Result<()> {
    let mut target = target(id)?;
    ensure!(
        bytes.len() <= CRED_MAX_CREDENTIAL_BLOB_SIZE as usize,
        "Account credentials exceed the Windows Credential Manager size limit"
    );
    let credential = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: target.as_mut_ptr(),
        CredentialBlobSize: bytes.len() as u32,
        CredentialBlob: bytes.as_ptr().cast_mut(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        ..unsafe { std::mem::zeroed() }
    };
    ensure!(
        unsafe { CredWriteW(&credential, 0) } != 0,
        "Unable to save account credentials in Windows Credential Manager"
    );
    Ok(())
}

pub(super) fn delete(id: &str) -> Result<()> {
    let target = target(id)?;
    unsafe {
        ensure!(
            CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) != 0 || GetLastError() == ERROR_NOT_FOUND,
            "Unable to remove account credentials from Windows Credential Manager"
        );
    }
    Ok(())
}

struct Key(BCRYPT_KEY_HANDLE);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            BCryptDestroyKey(self.0);
        }
    }
}

pub(super) fn decrypt(ciphertext: &[u8], key: &[u8], iv: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        key.len() == 16
            && iv.len() == 16
            && !ciphertext.is_empty()
            && ciphertext.len().is_multiple_of(16)
            && ciphertext.len() <= u32::MAX as usize,
        "Invalid Freecat encrypted subscription or protocol material"
    );
    let mut handle = std::ptr::null_mut();
    // CNG owns the key buffer when no object buffer is supplied.
    ensure!(
        unsafe {
            BCryptGenerateSymmetricKey(
                BCRYPT_AES_CBC_ALG_HANDLE,
                &mut handle,
                std::ptr::null_mut(),
                0,
                key.as_ptr(),
                key.len() as u32,
                0,
            )
        } >= 0,
        "Unable to initialize Freecat subscription decryption"
    );
    let handle = Key(handle);
    let mut output = vec![0; ciphertext.len()];
    let mut iv = iv.to_vec();
    let mut moved = 0;
    let status = unsafe {
        BCryptDecrypt(
            handle.0,
            ciphertext.as_ptr(),
            ciphertext.len() as u32,
            std::ptr::null(),
            iv.as_mut_ptr(),
            iv.len() as u32,
            output.as_mut_ptr(),
            output.len() as u32,
            &mut moved,
            BCRYPT_BLOCK_PADDING,
        )
    };
    ensure!(
        status >= 0 && moved as usize <= output.len(),
        "Freecat subscription decryption failed; the server may use a different protocol version"
    );
    output.truncate(moved as usize);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    #[test]
    fn windows_cbc_matches_the_subscription_format_and_rejects_bad_padding() -> Result<()> {
        let mut ciphertext = STANDARD.decode("kk/WadJgKSauDuN0AhsaXb6zwer2qG/YEMoayv5FPlA=")?;
        let key = [0; 16];
        let iv = [0; 16];
        ensure!(
            decrypt(&ciphertext, &key, &iv)? == b"Windows subscription test",
            "CNG must decode AES-128-CBC with PKCS7 padding"
        );
        ciphertext[31] ^= 1;
        ensure!(
            decrypt(&ciphertext, &key, &iv).is_err(),
            "Invalid padding must be rejected"
        );
        Ok(())
    }

    #[test]
    #[ignore = "requires an interactive Windows desktop logon"]
    fn windows_credentials_round_trip_in_the_user_vault() -> Result<()> {
        let id = format!("test-{}", nanoid::nanoid!());
        scopeguard::defer! { let _ = delete(&id); }
        let secrets = AccountSecrets {
            password: "test credential".into(),
            ..Default::default()
        };
        write(&id, &serde_json::to_vec(&secrets)?)?;
        ensure!(
            read(&id)?.password == secrets.password,
            "Credential Manager must preserve the account password"
        );
        delete(&id)?;
        ensure!(read(&id).is_err(), "Deleted credentials must not be returned");
        Ok(())
    }
}
