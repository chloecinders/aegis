use std::sync::LazyLock;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::platform::cache::Cache;

pub fn scale(width: u32, height: u32) -> Option<(u32, u32)> {
    let longest = width.max(height);

    if longest <= 960 || width == 0 || height == 0 {
        return None;
    }

    let ratio = 960.0 / f64::from(longest);
    let shrink = |side: u32| ((f64::from(side) * ratio).round() as u32).max(1);

    Some((shrink(width), shrink(height)))
}

static SEEN: LazyLock<Cache<String, Reading>> =
    LazyLock::new(|| Cache::new(1024, Some(Duration::from_secs(3600))));

static LANES: LazyLock<tokio::sync::Semaphore> = LazyLock::new(|| {
    let cores = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);

    tokio::sync::Semaphore::new((cores / 4).clamp(1, 4))
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub text: String,
    pub left: u32,
    pub top: u32,
    pub height: u32,
}

impl Block {
    fn centre(&self) -> u32 {
        self.top + self.height / 2
    }
}

pub fn reading_order(mut blocks: Vec<Block>) -> String {
    blocks.retain(|block| !block.text.trim().is_empty());
    blocks.sort_by_key(|block| (block.centre(), block.left));

    let mut lines: Vec<Vec<Block>> = Vec::new();

    for block in blocks {
        let joins = lines.last().is_some_and(|line| {
            line.iter().any(|placed| {
                let apart = placed.centre().abs_diff(block.centre());

                apart * 2 <= placed.height.min(block.height).max(1)
            })
        });

        match joins {
            true => lines
                .last_mut()
                .expect("a line was just read from the end")
                .push(block),
            false => lines.push(vec![block]),
        }
    }

    lines
        .iter_mut()
        .map(|line| {
            line.sort_by_key(|block| block.left);

            line.iter()
                .map(|block| block.text.trim())
                .collect::<Vec<&str>>()
                .join(" ")
        })
        .collect::<Vec<String>>()
        .join("\n")
}

#[cfg(feature = "ocr")]
mod backend {
    use std::sync::LazyLock;

    use xberg::plugins::OcrBackend;
    use xberg::{
        OcrBoundingGeometry, OcrConfig, OcrElementConfig, PaddleOcrBackend, PaddleOcrConfig,
    };

    use super::Block;

    static ENGINE: LazyLock<Result<PaddleOcrBackend, String>> = LazyLock::new(|| {
        use ort::ep::CPU;

        let committed = ort::init()
            .with_execution_providers([CPU::default().with_arena_allocator(false).build()])
            .commit();

        if !committed {
            tracing::warn!("onnx runtime has already been configured");
        }

        PaddleOcrBackend::new().map_err(|failure| failure.to_string())
    });

    static CONFIG: LazyLock<OcrConfig> = LazyLock::new(|| OcrConfig {
        auto_rotate: false,
        paddle_ocr_config: serde_json::to_value(PaddleOcrConfig {
            padding: 0,
            det_limit_side_len: 960,
            ..PaddleOcrConfig::default()
        })
        .ok(),
        element_config: Some(OcrElementConfig {
            include_elements: true,
            ..OcrElementConfig::default()
        }),
        ..OcrConfig::default()
    });

    pub async fn read(bytes: &[u8]) -> Option<super::Reading> {
        let engine = ENGINE.as_ref().ok()?;
        let result = engine.process_image(bytes, &CONFIG).await.ok()?;

        let Some(elements) = result.ocr_elements else {
            return Some(super::Reading {
                text: result.content.replace("\n\n", " "),
                runs: 0,
            });
        };

        let blocks: Vec<Block> = elements
            .iter()
            .map(|element| {
                let (left, top, height) = match &element.geometry {
                    OcrBoundingGeometry::Rectangle {
                        left, top, height, ..
                    } => (*left, *top, *height),
                    OcrBoundingGeometry::Quadrilateral { points } => {
                        let left = points.iter().map(|point| point.x).min().unwrap_or(0);
                        let top = points.iter().map(|point| point.y).min().unwrap_or(0);
                        let bottom = points.iter().map(|point| point.y).max().unwrap_or(0);

                        (left, top, bottom.saturating_sub(top))
                    }
                };

                Block {
                    text: element.text.clone(),
                    left,
                    top,
                    height,
                }
            })
            .collect();

        Some(super::Reading {
            runs: blocks.len(),
            text: super::reading_order(blocks),
        })
    }
}

#[cfg(not(feature = "ocr"))]
mod backend {
    pub async fn read(_bytes: &[u8]) -> Option<super::Reading> {
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reading {
    pub text: String,
    pub runs: usize,
}

pub async fn read(bytes: &[u8]) -> Option<Reading> {
    if !available() {
        return None;
    }

    let key = crate::platform::text::hex(&Sha256::digest(bytes));

    if let Some(known) = SEEN.get(&key) {
        return Some(known);
    }

    let reading = {
        let _lane = LANES.acquire().await.ok()?;

        backend::read(bytes).await
    };

    if let Some(read) = reading.as_ref() {
        SEEN.insert(key, read.clone());
    }

    reading
}

pub fn available() -> bool {
    cfg!(feature = "ocr")
}

pub fn forget() {
    SEEN.clear();
}

pub fn sweep() {
    SEEN.sweep();
}

fn page() -> image::RgbImage {
    let mut canvas = image::RgbImage::from_pixel(320, 64, image::Rgb([255, 255, 255]));

    for glyph in 0..6 {
        let left = 16 + glyph * 40;

        for x in left..(left + 24).min(320) {
            for y in 16..48 {
                canvas.put_pixel(x, y, image::Rgb([0, 0, 0]));
            }
        }
    }

    canvas
}

pub async fn warm() {
    if !available() {
        return;
    }

    let started = std::time::Instant::now();
    let mut written = std::io::Cursor::new(Vec::new());

    if let Err(failure) = page().write_to(&mut written, image::ImageFormat::Png) {
        tracing::warn!("ocr warm-up could not draw its own page: {failure}");

        return;
    }

    match read(&written.into_inner()).await {
        Some(_) => tracing::info!("ocr engine warm in {:?}", started.elapsed()),
        None => tracing::warn!("ocr engine did not start; image rules will not trigger"),
    }
}
