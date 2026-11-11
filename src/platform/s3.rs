use std::fmt::Write;

use image::ImageFormat;
use rand::RngCore;

use crate::app::config::S3;
use crate::domain::Snowflake;

pub fn token() -> String {
    let mut bytes = [0u8; 16];

    rand::rng().fill_bytes(&mut bytes);

    bytes
        .iter()
        .fold(String::with_capacity(32), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

pub fn placement(config: &S3, folder: &str, guild: Snowflake, token: &str) -> (String, String) {
    let base = config
        .public_base_url
        .clone()
        .unwrap_or_else(|| config.endpoint.clone());
    let key = format!("{folder}/{guild}/{token}.webp");
    let url = format!("{}/{key}", base.trim_end_matches('/'));

    (key, url)
}

pub fn to_webp(bytes: &[u8]) -> Option<Vec<u8>> {
    let decoded = image::load_from_memory(bytes).ok()?;
    let scaled = match decoded.width().max(decoded.height()) > 1600 {
        true => decoded.resize(1600, 1600, image::imageops::FilterType::Triangle),
        false => decoded,
    };
    let mut encoded = Vec::new();

    scaled
        .write_to(&mut std::io::Cursor::new(&mut encoded), ImageFormat::WebP)
        .ok()?;

    Some(encoded)
}

pub fn available() -> bool {
    cfg!(feature = "s3")
}

fn credentialed(config: &S3) -> bool {
    !config.access_key.is_empty() && !config.secret_key.is_empty()
}

pub fn settings(config: Option<&S3>) -> Option<&S3> {
    config.filter(|config| available() && credentialed(config))
}

#[cfg(feature = "s3")]
mod backend {
    use s3::creds::Credentials;
    use s3::{Bucket, Region};
    use tracing::warn;

    use super::S3;

    const CONTENT_TYPE: &str = "image/webp";

    fn handle(config: &S3) -> Option<Box<Bucket>> {
        if !super::credentialed(config) {
            return None;
        }

        let credentials = Credentials::new(
            Some(&config.access_key),
            Some(&config.secret_key),
            None,
            None,
            None,
        )
        .ok()?;
        let region = Region::Custom {
            region: config.region.clone().unwrap_or_default(),
            endpoint: config.endpoint.clone(),
        };
        let mut bucket = Bucket::new(&config.bucket, region, credentials)
            .ok()?
            .with_path_style();

        bucket.add_header("x-amz-acl", "public-read");

        Some(bucket)
    }

    pub async fn store(config: &S3, key: &str, bytes: &[u8]) -> bool {
        let Some(bucket) = handle(config) else {
            return false;
        };

        match bucket
            .put_object_with_content_type(key, bytes, CONTENT_TYPE)
            .await
        {
            Ok(_) => true,
            Err(err) => {
                warn!("could not store {key}; err = {err:?}");

                false
            }
        }
    }

    pub async fn remove(config: &S3, key: &str) {
        let Some(bucket) = handle(config) else {
            return;
        };

        if let Err(err) = bucket.delete_object(key).await {
            warn!("could not remove {key}; err = {err:?}");
        }
    }
}

#[cfg(not(feature = "s3"))]
mod backend {
    pub async fn store(_config: &super::S3, _key: &str, _bytes: &[u8]) -> bool {
        false
    }

    pub async fn remove(_config: &super::S3, _key: &str) {}
}

pub async fn store(config: &S3, key: &str, bytes: &[u8]) -> bool {
    backend::store(config, key, bytes).await
}

pub async fn remove(config: &S3, key: &str) {
    backend::remove(config, key).await
}
