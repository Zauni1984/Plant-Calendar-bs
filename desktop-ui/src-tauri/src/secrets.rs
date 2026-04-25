// GDPR Art. 32 — Security of processing.
// Secrets at rest (Tuya local_keys, SQLCipher master key) are stored in the
// platform's native credential store (Windows Credential Manager,
// macOS Keychain, libsecret/KWallet on Linux) instead of plaintext on disk.

use anyhow::Context;

const SERVICE: &str = "blocksocial-plants-calendar";

/// Store a secret in the OS keychain under the given name.
pub fn set(name: &str, value: &str) -> anyhow::Result<()> {
    let entry = keyring::Entry::new(SERVICE, name)
        .with_context(|| format!("failed to open keyring entry {name}"))?;
    entry
        .set_password(value)
        .with_context(|| format!("failed to write keyring entry {name}"))?;
    Ok(())
}

/// Retrieve a secret from the OS keychain. Returns `Ok(None)` when the entry
/// does not exist (so callers can distinguish "missing" from "error").
pub fn get(name: &str) -> anyhow::Result<Option<String>> {
    let entry = keyring::Entry::new(SERVICE, name)
        .with_context(|| format!("failed to open keyring entry {name}"))?;
    match entry.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e).with_context(|| format!("failed to read keyring entry {name}")),
    }
}

/// Delete a secret. Tolerates a missing entry.
pub fn delete(name: &str) -> anyhow::Result<()> {
    let entry = keyring::Entry::new(SERVICE, name)
        .with_context(|| format!("failed to open keyring entry {name}"))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e).with_context(|| format!("failed to delete keyring entry {name}")),
    }
}
