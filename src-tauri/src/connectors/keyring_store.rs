//! Armazenamento seguro de credenciais via chaveiro nativo do SO (Secret
//! Service/D-Bus no Linux, Credential Manager no Windows, Keychain no macOS)
//! usando a crate `keyring` v3 — o backend é escolhido em tempo de build por
//! `Cargo.toml` (`target.'cfg(target_os = "...")'`.dependencies`). A camada é
//! um trait (`CredentialStore`) para ser testável com um fake em memória, já
//! que o chaveiro real não está disponível em CI/sandboxes headless.

use std::collections::HashMap;
use std::sync::Mutex;

pub const KEYRING_SERVICE: &str = "dev.vetrobi.app";
const LEGACY_KEYRING_SERVICE: &str = "dev.prismbi.app";

/// Abstração sobre o armazenamento de segredos, indexado por uma chave
/// lógica (`connection_id`, ou `"ai:gemini"` para a chave de IA).
pub trait CredentialStore: Send + Sync {
    fn set(&self, key: &str, secret: &str) -> Result<(), String>;
    fn get(&self, key: &str) -> Result<Option<String>, String>;
    fn exists(&self, key: &str) -> Result<bool, String> {
        Ok(self.get(key)?.is_some())
    }
    fn delete(&self, key: &str) -> Result<(), String>;
}

/// Implementação real: chaveiro nativo do SO via a crate `keyring`
/// (Secret Service no Linux, Credential Manager no Windows, Keychain no
/// macOS — selecionado em tempo de build).
pub struct NativeKeyringStore;

impl NativeKeyringStore {
    pub fn new() -> Self {
        Self
    }

    fn entry(&self, key: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(KEYRING_SERVICE, key)
            .map_err(|e| format!("Falha ao acessar o chaveiro do sistema: {}", e))
    }

    fn legacy_entry(&self, key: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(LEGACY_KEYRING_SERVICE, key)
            .map_err(|e| format!("Falha ao acessar o chaveiro do sistema: {}", e))
    }
}

impl Default for NativeKeyringStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialStore for NativeKeyringStore {
    fn set(&self, key: &str, secret: &str) -> Result<(), String> {
        self.entry(key)?
            .set_password(secret)
            .map_err(|e| format!("Falha ao salvar credencial no chaveiro: {}", e))
    }

    fn get(&self, key: &str) -> Result<Option<String>, String> {
        match self.entry(key)?.get_password() {
            Ok(pw) => Ok(Some(pw)),
            Err(keyring::Error::NoEntry) => match self.legacy_entry(key)?.get_password() {
                Ok(pw) => Ok(Some(pw)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(e) => Err(format!("Falha ao ler credencial do chaveiro: {}", e)),
            },
            Err(e) => Err(format!("Falha ao ler credencial do chaveiro: {}", e)),
        }
    }

    fn delete(&self, key: &str) -> Result<(), String> {
        match self.entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(e) => return Err(format!("Falha ao remover credencial do chaveiro: {}", e)),
        }
        match self.legacy_entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("Falha ao remover credencial do chaveiro: {}", e)),
        }
    }
}


/// Fake em memória usado nos testes unitários e, opcionalmente, como
/// fallback quando o Secret Service não está disponível no ambiente.
#[derive(Default)]
pub struct InMemoryCredentialStore {
    data: Mutex<HashMap<String, String>>,
}

impl InMemoryCredentialStore {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(HashMap::new()),
        }
    }
}

impl CredentialStore for InMemoryCredentialStore {
    fn set(&self, key: &str, secret: &str) -> Result<(), String> {
        self.data
            .lock()
            .map_err(|_| "Lock do fake de credenciais corrompido".to_string())?
            .insert(key.to_string(), secret.to_string());
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<String>, String> {
        Ok(self
            .data
            .lock()
            .map_err(|_| "Lock do fake de credenciais corrompido".to_string())?
            .get(key)
            .cloned())
    }

    fn delete(&self, key: &str) -> Result<(), String> {
        self.data
            .lock()
            .map_err(|_| "Lock do fake de credenciais corrompido".to_string())?
            .remove(key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_store_set_get_exists_delete_roundtrip() {
        let store = InMemoryCredentialStore::new();
        assert_eq!(store.exists("conn_1").unwrap(), false);
        assert_eq!(store.get("conn_1").unwrap(), None);

        store.set("conn_1", "s3cr3t-password").unwrap();
        assert_eq!(store.exists("conn_1").unwrap(), true);
        assert_eq!(store.get("conn_1").unwrap().as_deref(), Some("s3cr3t-password"));

        store.delete("conn_1").unwrap();
        assert_eq!(store.exists("conn_1").unwrap(), false);
        assert_eq!(store.get("conn_1").unwrap(), None);
    }

    #[test]
    fn in_memory_store_isolates_keys() {
        let store = InMemoryCredentialStore::new();
        store.set("a", "secret-a").unwrap();
        store.set("b", "secret-b").unwrap();
        assert_eq!(store.get("a").unwrap().as_deref(), Some("secret-a"));
        assert_eq!(store.get("b").unwrap().as_deref(), Some("secret-b"));
        store.delete("a").unwrap();
        assert_eq!(store.get("a").unwrap(), None);
        assert_eq!(store.get("b").unwrap().as_deref(), Some("secret-b"));
    }

    #[test]
    fn deleting_missing_key_is_not_an_error() {
        let store = InMemoryCredentialStore::new();
        assert!(store.delete("never-set").is_ok());
    }

    /// Requer um chaveiro real do SO (Secret Service/D-Bus no Linux,
    /// Credential Manager no Windows, Keychain no macOS); não disponível em
    /// CI headless, por isso ignorado por padrão.
    #[test]
    #[ignore = "requer chaveiro nativo do SO, indisponível em CI headless"]
    fn native_keyring_store_roundtrip_against_real_os_keyring() {
        let store = NativeKeyringStore::new();
        let key = "vetrobi-test-key-connectors-v080";
        store.set(key, "integration-secret").unwrap();
        assert_eq!(store.get(key).unwrap().as_deref(), Some("integration-secret"));
        store.delete(key).unwrap();
        assert_eq!(store.get(key).unwrap(), None);
    }
}
