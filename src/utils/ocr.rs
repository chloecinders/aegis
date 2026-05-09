use kreuzberg::{OcrConfig, PaddleOcrBackend, plugins::OcrBackend};
use std::sync::LazyLock;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OcrError {
    #[error("Extraction failed: {0}")]
    ExtractionError(String),
}

static BACKEND: LazyLock<Result<PaddleOcrBackend, String>> =
    LazyLock::new(|| PaddleOcrBackend::new().map_err(|e| e.to_string()));

static CONFIG: LazyLock<OcrConfig> = LazyLock::new(|| {
    let mut config = OcrConfig::default();
    config.auto_rotate = true;
    config
});

pub async fn extract_text_from_bytes(bytes: &[u8]) -> Result<String, OcrError> {
    let backend = BACKEND
        .as_ref()
        .map_err(|e| OcrError::ExtractionError(e.clone()))?;

    let result = backend
        .process_image(bytes, &CONFIG)
        .await
        .map_err(|e| OcrError::ExtractionError(e.to_string()))?;

    Ok(result.content.replace("\n\n", " "))
}
