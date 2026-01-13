use security_framework::passwords::{get_generic_password, set_generic_password, delete_generic_password};

const SERVICE: &str = "com.marginalia";

/// Store a value in the macOS Keychain
pub fn store_keychain(account: &str, value: &str) -> Result<(), String> {
    // Delete existing entry if present
    let _ = delete_generic_password(SERVICE, account);

    set_generic_password(SERVICE, account, value.as_bytes())
        .map_err(|e| format!("Failed to store in keychain: {}", e))
}

/// Retrieve a value from the macOS Keychain
pub fn get_keychain(account: &str) -> Option<String> {
    get_generic_password(SERVICE, account)
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
}

/// Delete a value from the macOS Keychain
pub fn delete_keychain(account: &str) -> Result<(), String> {
    delete_generic_password(SERVICE, account)
        .map_err(|e| format!("Failed to delete from keychain: {}", e))
}
