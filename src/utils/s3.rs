use rand::RngCore;
use s3::{Bucket, Region, creds::Credentials};
use sha2::{Digest, Sha256};
use tracing::warn;

use crate::BOT_CONFIG;

pub fn get_predicted_url(guild_id: u64, token: &str, ext: &str) -> (String, String) {
    let endpoint = BOT_CONFIG.s3.endpoint.clone();
    let public_base = BOT_CONFIG.s3.public_base_url.clone().unwrap_or(endpoint);
    let key = format!("refs/{}/{}.{}", guild_id, token, ext);
    let url = format!("{}/{}", public_base.trim_end_matches('/'), key);
    (key, url)
}

pub async fn upload_image_with_key(key: String, data: Vec<u8>, content_type: &str) -> bool {
    let endpoint = BOT_CONFIG.s3.endpoint.clone();
    let bucket_name = BOT_CONFIG.s3.bucket.clone();
    let access_key = BOT_CONFIG.s3.access_key.clone();
    let secret_key = BOT_CONFIG.s3.secret_key.clone();
    let region_str = BOT_CONFIG.s3.region.clone().unwrap_or_default();

    if access_key.is_empty() || secret_key.is_empty() {
        return false;
    }

    let creds = match Credentials::new(Some(&access_key), Some(&secret_key), None, None, None) {
        Ok(c) => c,
        Err(err) => {
            warn!("S3: failed to build credentials; err = {err:?}");
            return false;
        }
    };

    let region = Region::Custom {
        region: region_str.to_string(),
        endpoint: endpoint.to_string(),
    };

    let bucket = match Bucket::new(&bucket_name, region, creds) {
        Ok(b) => b.with_path_style(),
        Err(err) => {
            warn!("S3: failed to create bucket handle; err = {err:?}");
            return false;
        }
    };

    let mut bucket = bucket;
    bucket.add_header("x-amz-acl", "public-read");

    match bucket
        .put_object_with_content_type(&key, &data, content_type)
        .await
    {
        Ok(_) => true,
        Err(err) => {
            warn!("S3: upload failed for key={key}; err = {err:?}");
            false
        }
    }
}

pub async fn upload_image(
    guild_id: u64,
    data: Vec<u8>,
    ext: &str,
    content_type: &str,
) -> Option<String> {
    let token = random_token();
    let (key, url) = get_predicted_url(guild_id, &token, ext);
    if upload_image_with_key(key, data, content_type).await {
        Some(url)
    } else {
        None
    }
}

pub fn detect_content_type(data: &[u8]) -> &'static str {
    match data {
        d if d.starts_with(b"\x89PNG") => "image/png",
        d if d.starts_with(b"\xff\xd8\xff") => "image/jpeg",
        d if d.starts_with(b"GIF8") => "image/gif",
        d if d.len() > 11 && d.starts_with(b"RIFF") && &d[8..12] == b"WEBP" => "image/webp",
        _ => "application/octet-stream",
    }
}

pub fn ext_for_content_type(ct: &str) -> &'static str {
    match ct {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "bin",
    }
}

pub fn random_token() -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().fold(String::with_capacity(32), |mut s, b| {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
        s
    })
}
