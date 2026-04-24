use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

pub fn generate_key() -> [u8; 32] {
    let key = Aes256Gcm::generate_key(OsRng);
    key.into()
}

pub fn key_to_display(key: &[u8; 32]) -> String {
    BASE64.encode(key)
}

pub fn display_to_key(s: &str) -> Option<[u8; 32]> {
    let bytes = BASE64.decode(s.trim()).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Some(key)
}

pub fn encrypt(key: &[u8; 32], plaintext: &str) -> Option<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher.encrypt(&nonce, plaintext.as_bytes()).ok()?;

    let mut combined = nonce.to_vec();
    combined.extend_from_slice(&ciphertext);
    Some(combined)
}

pub fn decrypt(key: &[u8; 32], encoded: &[u8]) -> Option<String> {
    if encoded.len() < 12 {
        return None;
    }

    let (nonce_bytes, ciphertext) = encoded.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher.decrypt(nonce, ciphertext).ok()?;
    String::from_utf8(plaintext).ok()
}
