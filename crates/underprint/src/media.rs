use std::io::Cursor;

use image::{DynamicImage, GenericImageView, ImageFormat, ImageReader, Limits, RgbImage, imageops};
use sha2::{Digest, Sha256};

use crate::{Error, InputSummary, Result};

#[derive(Debug, Clone)]
pub struct ImagePolicy {
    pub max_input_bytes: usize,
    pub max_output_bytes: usize,
    pub max_dimension: u32,
    pub min_model_dimension: u32,
    pub max_decode_alloc: u64,
}

impl Default for ImagePolicy {
    fn default() -> Self {
        Self {
            max_input_bytes: 10 * 1024 * 1024,
            max_output_bytes: 64 * 1024 * 1024,
            max_dimension: 4096,
            min_model_dimension: 64,
            max_decode_alloc: 128 * 1024 * 1024,
        }
    }
}

#[derive(Debug)]
pub struct LoadedImage {
    pub image: DynamicImage,
    pub summary: InputSummary,
}

pub fn load_image(source: &[u8], policy: &ImagePolicy) -> Result<LoadedImage> {
    if source.is_empty() {
        return Err(Error::invalid_input("input is empty"));
    }
    if source.len() > policy.max_input_bytes {
        return Err(Error::resource_limit(
            "image exceeds the 10 MiB input limit",
        ));
    }

    let mut reader = ImageReader::new(Cursor::new(source))
        .with_guessed_format()
        .map_err(|_| Error::invalid_input("input is not a readable image"))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(policy.max_dimension);
    limits.max_image_height = Some(policy.max_dimension);
    limits.max_alloc = Some(policy.max_decode_alloc);
    reader.limits(limits);
    let format = reader
        .format()
        .ok_or_else(|| Error::invalid_input("input is not a supported image"))?;
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
    ) {
        return Err(Error::invalid_input(
            "only PNG, JPEG, and still WebP images are supported",
        ));
    }

    let image = reader.decode().map_err(|error| match error {
        image::ImageError::Limits(_) => {
            Error::resource_limit("image exceeds the safe dimension or pixel allocation limit")
        }
        _ => Error::invalid_input("input is not a readable image"),
    })?;
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 || width > policy.max_dimension || height > policy.max_dimension {
        return Err(Error::invalid_input(
            "image dimensions must be between 1 and 4096 pixels",
        ));
    }

    let media_type = match format {
        ImageFormat::Png => "image/png",
        ImageFormat::Jpeg => "image/jpeg",
        ImageFormat::WebP => "image/webp",
        _ => unreachable!(),
    };
    let summary = InputSummary {
        sha256: sha256_hex(source),
        media_type: media_type.to_owned(),
        bytes: source.len() as u64,
        width,
        height,
    };

    Ok(LoadedImage { image, summary })
}

pub fn normalize_for_model(image: DynamicImage, policy: &ImagePolicy) -> DynamicImage {
    let rgb = image.into_rgb8();
    let (width, height) = rgb.dimensions();
    if width >= policy.min_model_dimension && height >= policy.min_model_dimension {
        return DynamicImage::ImageRgb8(rgb);
    }

    let scale = f64::max(
        policy.min_model_dimension as f64 / width as f64,
        policy.min_model_dimension as f64 / height as f64,
    );
    let target_width = (width as f64 * scale).round() as u32;
    let target_height = (height as f64 * scale).round() as u32;
    if target_width <= policy.max_dimension && target_height <= policy.max_dimension {
        return DynamicImage::ImageRgb8(imageops::resize(
            &rgb,
            target_width,
            target_height,
            imageops::FilterType::Lanczos3,
        ));
    }

    let canvas_width = width.max(policy.min_model_dimension);
    let canvas_height = height.max(policy.min_model_dimension);
    let mut canvas = RgbImage::new(canvas_width, canvas_height);
    let x = i64::from((canvas_width - width) / 2);
    let y = i64::from((canvas_height - height) / 2);
    imageops::replace(&mut canvas, &rgb, x, y);
    DynamicImage::ImageRgb8(canvas)
}

pub fn serialize_png(image: &DynamicImage, policy: &ImagePolicy) -> Result<Vec<u8>> {
    let mut output = Cursor::new(Vec::new());
    // Neural engines commonly return normalized floating-point pixels. PNG
    // output is deliberately quantized to 8-bit RGB before self-verification,
    // matching the compatibility worker's final serialization boundary.
    if matches!(image, DynamicImage::ImageRgb8(_)) {
        image
            .write_to(&mut output, ImageFormat::Png)
            .map_err(|_| Error::internal("failed to serialize protected image"))?;
    } else {
        DynamicImage::ImageRgb8(image.to_rgb8())
            .write_to(&mut output, ImageFormat::Png)
            .map_err(|_| Error::internal("failed to serialize protected image"))?;
    }
    let output = output.into_inner();
    if output.len() > policy.max_output_bytes {
        return Err(Error::resource_limit(
            "protected image exceeds the 64 MiB output limit",
        ));
    }
    Ok(output)
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        serialize_png(
            &DynamicImage::new_rgb8(width, height),
            &ImagePolicy::default(),
        )
        .unwrap()
    }

    #[test]
    fn reads_supported_images_with_summary() {
        let source = png(320, 180);
        let loaded = load_image(&source, &ImagePolicy::default()).unwrap();
        assert_eq!((loaded.summary.width, loaded.summary.height), (320, 180));
        assert_eq!(loaded.summary.media_type, "image/png");
        assert_eq!(loaded.summary.sha256.len(), 64);
    }

    #[test]
    fn rejects_dimensions_before_algorithm_use() {
        let source = png(4097, 1);
        let error = load_image(&source, &ImagePolicy::default()).unwrap_err();
        assert_eq!(error.kind, crate::ErrorKind::ResourceLimit);
    }

    #[test]
    fn enlarges_small_images_proportionally() {
        let normalized = normalize_for_model(DynamicImage::new_rgb8(10, 20), &Default::default());
        assert_eq!(normalized.dimensions(), (64, 128));
    }
}
