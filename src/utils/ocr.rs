use std::{env::current_dir, io};

use tesseract_rs::TesseractAPI;
use thiserror::Error;

use crate::{BOT_CONFIG, utils::send_error};

#[derive(Error, Debug)]
pub enum OcrError {
    #[error("Could not find current working directory")]
    DirectoryError,
    #[error("Could not download training data")]
    TrainingDataDownloadError,
    #[error("Tesseract error: {0:?}")]
    TesseractError(#[from] tesseract_rs::TesseractError),
    #[error("FS error: {0:?}")]
    FsError(#[from] io::Error)
}

pub struct ImageData {
    pub raw: Vec<u8>,
    pub width: i32,
    pub height: i32,
}

pub async fn image_to_string(image_data: ImageData) -> Result<String, OcrError> {
    let api = TesseractAPI::new();

    let Ok(current_dir) = current_dir() else {
        send_error("OCR FAILED".into(), "Could not find current working directory.".into());
        return Err(OcrError::DirectoryError);
    };

    download_training_data().await?;

    api.init(current_dir.join("tesseract"), "eng")?;
    api.set_image(&image_data.raw, image_data.width, image_data.height, 4, image_data.width * 4)?;

    if let Some(whitelist) = BOT_CONFIG.ocr_character_whitelist.clone() {
        api.set_variable("tessedit_char_whitelist", &whitelist)?;
    }

    Ok(api.get_utf8_text()?)
}

async fn download_training_data() -> Result<(), OcrError> {
    let Ok(current_dir) = current_dir() else {
        return Err(OcrError::DirectoryError);
    };

    if !std::fs::exists(current_dir.join("tesseract").join("eng.traineddata"))? {
        if !std::fs::exists(current_dir.join("tesseract"))? {
            std::fs::create_dir(current_dir.join("tesseract"))?;
        }

        let Ok(req) = reqwest::get(BOT_CONFIG.ocr_training_data.clone().unwrap_or(String::new())).await else { return Err(OcrError::TrainingDataDownloadError) };
        let Ok(bytes) = req.bytes().await else { return Err(OcrError::TrainingDataDownloadError) };
        if let Err(err) = std::fs::write(current_dir.join("tesseract").join("eng.traineddata"), &bytes) {
            dbg!(err);
        }
    }

    Ok(())
}
