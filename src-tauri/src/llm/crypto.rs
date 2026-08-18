use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rand::RngCore;
use sha2::{Digest, Sha256};

const APP_SALT: &[u8] = b"axiom-risk-agent-llm-v1";

fn cipher() -> Result<Aes256Gcm, String> {
    let mut hasher = Sha256::new();
    hasher.update(APP_SALT);
    // bind to user home path lightly so dumps aren't trivially portable
    if let Some(home) = dirs::home_dir() {
        hasher.update(home.to_string_lossy().as_bytes());
    }
    let key = hasher.finalize();
    Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())
}

pub fn encrypt(plain: &str) -> Result<String, String> {
    let cipher = cipher()?;
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plain.as_bytes())
        .map_err(|e| format!("encrypt failed: {e}"))?;
    let mut packed = Vec::with_capacity(12 + ciphertext.len());
    packed.extend_from_slice(&nonce_bytes);
    packed.extend_from_slice(&ciphertext);
    Ok(B64.encode(packed))
}

pub fn decrypt(encoded: &str) -> Result<String, String> {
    let packed = B64
        .decode(encoded.trim())
        .map_err(|e| format!("api key decode failed: {e}"))?;
    if packed.len() < 13 {
        return Err("api key ciphertext too short".into());
    }
    let (nonce_bytes, ciphertext) = packed.split_at(12);
    let cipher = cipher()?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let plain = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "api key 解密失败，请重新保存 API Key".to_string())?;
    String::from_utf8(plain).map_err(|e| e.to_string())
}
