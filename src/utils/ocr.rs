use std::{env::current_dir, io};

use tesseract_rs::{TessPageSegMode, TesseractAPI};
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
    FsError(#[from] io::Error),
}

pub struct ImageData {
    pub raw: Vec<u8>,
    pub width: i32,
    pub height: i32,
}

async fn get_tesseract() -> Result<TesseractAPI, OcrError> {
    let api = TesseractAPI::new();

    let Ok(current_dir) = current_dir() else {
        send_error(
            "OCR FAILED".into(),
            "Could not find current working directory.".into(),
        );
        return Err(OcrError::DirectoryError);
    };

    download_training_data().await?;

    api.init(current_dir.join("tesseract"), "eng")?;
    api.set_page_seg_mode(TessPageSegMode::PSM_AUTO_OSD)?;

    if let Some(whitelist) = BOT_CONFIG.ocr_character_whitelist.clone() {
        api.set_variable("tessedit_char_whitelist", &whitelist)?;
    }

    Ok(api)
}

pub async fn likely_has_text(image_data: &ImageData) -> Result<bool, OcrError> {
    let api = get_tesseract().await?;

    api.set_image(
        &image_data.raw,
        image_data.width,
        image_data.height,
        4,
        image_data.width * 4,
    )?;

    let text = api.get_utf8_text()?;
    let conf = api.mean_text_conf()?;

    let chars = text.chars().filter(|c| !c.is_whitespace()).count();

    Ok(chars >= 10 && conf >= 30)
}

pub async fn image_to_string(image_data: &ImageData) -> Result<String, OcrError> {
    let api = get_tesseract().await?;

    api.set_image(
        &image_data.raw,
        image_data.width,
        image_data.height,
        4,
        image_data.width * 4,
    )?;

    let text = api.get_utf8_text()?;
    api.end()?;

    Ok(text)
}

pub async fn image_to_string_with_rotation(image_data: &ImageData) -> Result<String, OcrError> {
    let api = get_tesseract().await?;
    api.set_page_seg_mode(TessPageSegMode::PSM_SPARSE_TEXT_OSD)?;

    // let (mut conf0, mut conf90, mut conf180, mut conf270) = (0, 0, 0, 0);

    api.set_image(
        &image_data.raw,
        image_data.width,
        image_data.height,
        4,
        image_data.width * 4,
    )?;

    // conf0 = api.mean_text_conf()?;
    api.end()?;

    todo!()
}

async fn download_training_data() -> Result<(), OcrError> {
    let Ok(current_dir) = current_dir() else {
        return Err(OcrError::DirectoryError);
    };

    if !std::fs::exists(current_dir.join("tesseract"))? {
        std::fs::create_dir(current_dir.join("tesseract"))?;
    }

    let base_url = BOT_CONFIG
        .ocr_training_data
        .clone()
        .unwrap_or(String::new());

    if !std::fs::exists(current_dir.join("tesseract").join("eng.traineddata"))? {
        let Ok(req) = reqwest::get(format!("{base_url}/eng.traineddata")).await else {
            return Err(OcrError::TrainingDataDownloadError);
        };
        let Ok(bytes) = req.bytes().await else {
            return Err(OcrError::TrainingDataDownloadError);
        };
        if let Err(err) = std::fs::write(
            current_dir.join("tesseract").join("eng.traineddata"),
            &bytes,
        ) {
            send_error(
                String::from("OCR DATA DOWNLOAD"),
                format!("Could not download OCR training data {err:?}"),
            );
        }
    }

    if !std::fs::exists(current_dir.join("tesseract").join("osd.traineddata"))? {
        let Ok(req) = reqwest::get(format!("{base_url}/osd.traineddata")).await else {
            return Err(OcrError::TrainingDataDownloadError);
        };
        let Ok(bytes) = req.bytes().await else {
            return Err(OcrError::TrainingDataDownloadError);
        };
        if let Err(err) = std::fs::write(
            current_dir.join("tesseract").join("osd.traineddata"),
            &bytes,
        ) {
            send_error(
                String::from("OCR DATA DOWNLOAD"),
                format!("Could not download OCR training data {err:?}"),
            );
        }
    }

    Ok(())
}
