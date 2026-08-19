// Copyright 2024 Adobe
// All Rights Reserved.
//
// NOTICE: Adobe permits you to use, modify, and distribute this file in
// accordance with the terms of the Adobe license agreement accompanying
// it.

//! # Trustmark
//!
//! An implementation of TrustMark watermarking for the Content Authenticity Initiative (CAI) in
//! Rust, as described in:
//!
//! ---
//!
//! **TrustMark - Universal Watermarking for Arbitrary Resolution Images**
//!
//! <https://arxiv.org/abs/2311.18297>
//!
//! [Tu Bui]<sup>1</sup>, [Shruti Agarwal]<sup>2</sup>, [John Collomosse]<sup>1,2</sup>
//!
//! <sup>1</sup>DECaDE Centre for the Decentralized Digital Economy, University of Surrey, UK.\
//! <sup>2</sup>Adobe Research, San Jose CA.
//!
//! ---
//!
//! This is a re-implementation of the [trustmark] Python library.
//!
//! [Tu Bui]: https://www.surrey.ac.uk/people/tu-bui
//! [Shruti Agarwal]: https://research.adobe.com/person/shruti-agarwal/
//! [John Collomosse]: https://www.collomosse.com/
//! [trustmark]: https://pypi.org/project/trustmark/
//!
//! ## Example
//!
//! ```rust,no_run
//! use trustmark::{Trustmark, Version, Variant};
//!
//! # fn main() {
//! let mut tm = Trustmark::new("./models", Variant::Q, Version::Bch5).unwrap();
//! let input = image::open("../images/ghost.png").unwrap();
//! let output = tm.encode("0010101".to_owned(), &input, 0.95);
//! # }
//! ```
use std::path::{Path, PathBuf};

use image::{DynamicImage, GenericImageView as _};
use ort::{CPUExecutionProvider, GraphOptimizationLevel, Session, SessionInputValue};

use self::{
    bits::Bits,
    image_processing::{ModelImage, ResidualImage},
};

mod bits;
mod image_processing;
mod model;

/// A Trustmark model whose encoder and decoder sessions are loaded on demand.
pub struct Trustmark {
    encoder: Option<Session>,
    decoder: Option<Session>,
    encoder_path: PathBuf,
    decoder_path: PathBuf,
    runtime: RuntimeOptions,
    version: Version,
    variant: Variant,
}

/// CPU runtime policy for the two ONNX Runtime sessions.
///
/// These options affect resource use, not the model, codec, or profile
/// identity. Callers should benchmark their deployment topology before
/// changing the defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeOptions {
    pub intra_threads: usize,
    pub cpu_arena: bool,
    pub memory_pattern: bool,
    pub prepacking: bool,
}

impl Default for RuntimeOptions {
    fn default() -> Self {
        Self {
            intra_threads: std::thread::available_parallelism()
                .map_or(1, usize::from)
                .min(6),
            cpu_arena: false,
            memory_pattern: true,
            prepacking: true,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("watermark is corrupt or missing")]
    CorruptWatermark,
    #[error("onnx error: {0}")]
    Ort(#[from] ort::Error),
    #[error("image processing error: {0}")]
    ImageProcessing(#[from] image_processing::Error),
    #[error("bits processing error: {0}")]
    Bits(bits::Error),
    #[error("invalid model variant")]
    InvalidModelVariant,
    #[error("invalid runtime options")]
    InvalidRuntimeOptions,
}

impl From<bits::Error> for Error {
    fn from(value: bits::Error) -> Self {
        match value {
            bits::Error::CorruptWatermark => Error::CorruptWatermark,
            err => Error::Bits(err),
        }
    }
}

pub use bits::Version;
pub use model::Variant;

impl Trustmark {
    /// Configure a Trustmark model for lazy loading.
    pub fn new<P: AsRef<Path>>(
        models: P,
        variant: Variant,
        version: Version,
    ) -> Result<Self, Error> {
        Self::new_with_options(models, variant, version, RuntimeOptions::default())
    }

    pub fn new_with_options<P: AsRef<Path>>(
        models: P,
        variant: Variant,
        version: Version,
        options: RuntimeOptions,
    ) -> Result<Self, Error> {
        if options.intra_threads == 0 {
            return Err(Error::InvalidRuntimeOptions);
        }
        Ok(Self {
            encoder: None,
            decoder: None,
            encoder_path: models.as_ref().join(variant.encoder_filename()),
            decoder_path: models.as_ref().join(variant.decoder_filename()),
            runtime: options,
            version,
            variant,
        })
    }

    /// Initialize both model sessions eagerly, for readiness checks and
    /// deployments that prefer paying all startup cost before serving work.
    pub fn initialize(&mut self) -> Result<(), Error> {
        self.encoder()?;
        self.decoder()?;
        Ok(())
    }

    fn encoder(&mut self) -> Result<&Session, Error> {
        if self.encoder.is_none() {
            self.encoder = Some(build_session(&self.encoder_path, self.runtime)?);
        }
        Ok(self.encoder.as_ref().expect("encoder was initialized"))
    }

    fn decoder(&mut self) -> Result<&Session, Error> {
        if self.decoder.is_none() {
            self.decoder = Some(build_session(&self.decoder_path, self.runtime)?);
        }
        Ok(self.decoder.as_ref().expect("decoder was initialized"))
    }

    /// Encode a watermark into an image.
    ///
    /// `watermark` is a bitstring encoding the watermark identifier to encode. `img` is the image
    /// which will be watermarked. `strength` is a number between 0 and 1 indicating how strong the
    /// resulting watermark should be. 0.95 is a normal strength.
    pub fn encode(
        &mut self,
        watermark: String,
        img: &DynamicImage,
        strength: f32,
    ) -> Result<DynamicImage, Error> {
        let variant = self.variant;
        let (original_width, original_height) = img.dimensions();
        let aspect_ratio = original_width as f32 / original_height as f32;

        // the image is always encoded with size 256x256
        let encode_size = 256;

        let input_img: ort::Value<ort::TensorValueType<f32>> =
            ModelImage(encode_size, variant, img).try_into()?;
        let bits: ort::Value<ort::TensorValueType<f32>> =
            Bits::apply_error_correction_and_schema(watermark, self.version)?.into();
        let inputs = [
            SessionInputValue::from(input_img.view()),
            SessionInputValue::from(bits),
        ];
        let outputs = self.encoder()?.run(inputs)?;
        let output_img = outputs["image"].try_extract_tensor::<f32>()?;
        let input_tensor = input_img.try_extract_tensor::<f32>()?;
        let residual = (variant.strength_multiplier() * strength) * (&output_img - &input_tensor);

        // Residual should be small perturbations.
        let mut residual = residual.clamp(-0.2, 0.2);
        if (variant == Variant::Q && !(0.5..=2.0).contains(&aspect_ratio)) || variant == Variant::P
        {
            residual = image_processing::remove_boundary_artifact(
                residual,
                (original_width as usize, original_height as usize),
                variant,
            );
        }

        let residual: DynamicImage = ResidualImage(encode_size, variant, residual).try_into()?;

        Ok(image_processing::apply_residual(img.clone(), residual))
    }

    /// Decode a watermark from an image.
    pub fn decode(&mut self, img: &DynamicImage) -> Result<String, Error> {
        // P variant has a smaller decode size
        let decode_size = if self.variant == Variant::P { 224 } else { 256 };

        let img: ort::Value<ort::TensorValueType<f32>> =
            ModelImage(decode_size, self.variant, img).try_into()?;
        let outputs = self.decoder()?.run(ort::inputs![
            "image" => img,
        ]?)?;
        let watermark = outputs["output"].try_extract_tensor::<f32>()?.to_owned();
        let watermark: Bits = watermark.try_into()?;
        Ok(watermark.get_data())
    }
}

fn build_session(path: &Path, options: RuntimeOptions) -> Result<Session, Error> {
    let cpu = if options.cpu_arena {
        CPUExecutionProvider::default().with_arena_allocator()
    } else {
        CPUExecutionProvider::default()
    };
    Ok(Session::builder()?
        .with_execution_providers([cpu.build()])?
        .with_optimization_level(GraphOptimizationLevel::Level3)?
        .with_intra_threads(options.intra_threads)?
        .with_memory_pattern(options.memory_pattern)?
        .with_prepacking(options.prepacking)?
        .commit_from_file(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires model artifacts in the vendored package directory"]
    fn loading_models() {
        let mut trustmark = Trustmark::new("./models", Variant::Q, Version::Bch5).unwrap();
        trustmark.initialize().unwrap();
    }

    fn roundtrip(path: impl AsRef<Path>) {
        let mut tm = Trustmark::new("./models", Variant::Q, Version::Bch5).unwrap();
        let input = image::open(path.as_ref()).unwrap();
        let watermark = "1011011110011000111111000000011111011111011100000110110110111".to_owned();
        let encoded = tm.encode(watermark.clone(), &input, 0.95).unwrap();
        encoded.to_rgba8().save("./test.png").unwrap();
        let input = image::open("./test.png").unwrap();
        let decoded = tm.decode(&input).unwrap();
        assert_eq!(watermark, decoded);
    }

    #[test]
    #[ignore = "requires upstream image and model fixtures"]
    fn roundtrip_ghost() {
        roundtrip("../images/ghost.png");
    }

    #[test]
    #[ignore = "requires upstream image and model fixtures"]
    fn roundtrip_ufo() {
        roundtrip("../images/ufo_240.jpg");
    }
}
