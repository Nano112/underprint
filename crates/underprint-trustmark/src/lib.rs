//! Native TrustMark support for Underprint.
//!
//! Adobe's TrustMark Rust implementation and model architecture remain Adobe's
//! work under its MIT licence. This crate adds Underprint profile identity,
//! artifact verification, error mapping, and orchestration integration.

use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Mutex,
};

use image::DynamicImage;
use sha2::{Digest, Sha256};
use trustmark::{RuntimeOptions, Trustmark, Variant, Version};
use underprint::{
    ArtifactDescriptor, Capability, Error, ProfileDescriptor, Result, TRUSTMARK_Q_BCH5_PROFILE,
    WatermarkEngine,
};

pub const ENCODER_FILENAME: &str = "encoder_Q.onnx";
pub const DECODER_FILENAME: &str = "decoder_Q.onnx";
pub const ENCODER_SHA256: &str = "19b3d1b25836130ffd78775a8f61539f993375d1823ef0e59ba5b8dffb4f892d";
pub const DECODER_SHA256: &str = "ee3268f057c9dabef680e169302f5973d0589feea86189ed229a896cc3aa88df";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrustmarkOptions {
    pub intra_threads: usize,
    pub cpu_arena: bool,
    pub memory_pattern: bool,
    pub prepacking: bool,
}

impl Default for TrustmarkOptions {
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

impl TrustmarkOptions {
    /// Retain ONNX activation arenas for the lowest single-request latency at
    /// the cost of substantially higher resident memory per process.
    pub fn throughput() -> Self {
        Self {
            cpu_arena: true,
            memory_pattern: false,
            ..Self::default()
        }
    }
}

pub struct TrustmarkEngine {
    descriptor: ProfileDescriptor,
    inner: Mutex<Trustmark>,
}

impl TrustmarkEngine {
    /// Verify the pinned Q artifacts and configure lazy encoder/decoder sessions.
    pub fn load(models_dir: impl AsRef<Path>) -> Result<Self> {
        Self::load_with_options(models_dir, TrustmarkOptions::default())
    }

    pub fn load_with_options(
        models_dir: impl AsRef<Path>,
        options: TrustmarkOptions,
    ) -> Result<Self> {
        if !(1..=64).contains(&options.intra_threads) {
            return Err(Error::invalid_argument(
                "TrustMark intra_threads must be between 1 and 64",
            ));
        }
        let models_dir = models_dir.as_ref();
        verify_artifact(models_dir.join(ENCODER_FILENAME), ENCODER_SHA256)?;
        verify_artifact(models_dir.join(DECODER_FILENAME), DECODER_SHA256)?;

        let inner = Trustmark::new_with_options(
            models_dir,
            Variant::Q,
            Version::Bch5,
            RuntimeOptions {
                intra_threads: options.intra_threads,
                cpu_arena: options.cpu_arena,
                memory_pattern: options.memory_pattern,
                prepacking: options.prepacking,
            },
        )
        .map_err(|error| Error::unavailable(format!("failed to load TrustMark Q: {error}")))?;
        Ok(Self {
            descriptor: descriptor(),
            inner: Mutex::new(inner),
        })
    }

    pub fn initialize(&self) -> Result<()> {
        self.inner
            .lock()
            .map_err(|_| Error::internal("TrustMark model lock was poisoned"))?
            .initialize()
            .map_err(|error| {
                Error::unavailable(format!("failed to initialize TrustMark Q: {error}"))
            })
    }
}

impl WatermarkEngine for TrustmarkEngine {
    fn descriptor(&self) -> &ProfileDescriptor {
        &self.descriptor
    }

    fn embed(&self, image: &DynamicImage, payload: &str, strength: f32) -> Result<DynamicImage> {
        self.inner
            .lock()
            .map_err(|_| Error::internal("TrustMark model lock was poisoned"))?
            .encode(payload.to_owned(), image, strength)
            .map_err(|error| Error::algorithm(format!("TrustMark embed failed: {error}")))
    }

    fn detect(&self, image: &DynamicImage) -> Result<Option<String>> {
        match self
            .inner
            .lock()
            .map_err(|_| Error::internal("TrustMark model lock was poisoned"))?
            .decode(image)
        {
            Ok(payload) => Ok(Some(payload)),
            Err(trustmark::Error::CorruptWatermark) => Ok(None),
            Err(error) => Err(Error::algorithm(format!(
                "TrustMark detection failed: {error}"
            ))),
        }
    }
}

pub fn descriptor() -> ProfileDescriptor {
    ProfileDescriptor {
        id: TRUSTMARK_Q_BCH5_PROFILE.to_owned(),
        algorithm: "trustmark".to_owned(),
        version: 1,
        payload_codec: "binary-bch5".to_owned(),
        payload_bits: 61,
        capabilities: vec![Capability::Embed, Capability::Detect],
        media_types: vec![
            "image/png".to_owned(),
            "image/jpeg".to_owned(),
            "image/webp".to_owned(),
        ],
        runtime: "onnxruntime-cpu".to_owned(),
        artifacts: vec![
            ArtifactDescriptor {
                name: ENCODER_FILENAME.to_owned(),
                sha256: ENCODER_SHA256.to_owned(),
            },
            ArtifactDescriptor {
                name: DECODER_FILENAME.to_owned(),
                sha256: DECODER_SHA256.to_owned(),
            },
        ],
    }
}

pub fn verify_models(models_dir: impl AsRef<Path>) -> Result<()> {
    let models_dir = models_dir.as_ref();
    verify_artifact(models_dir.join(ENCODER_FILENAME), ENCODER_SHA256)?;
    verify_artifact(models_dir.join(DECODER_FILENAME), DECODER_SHA256)
}

fn verify_artifact(path: PathBuf, expected: &str) -> Result<()> {
    let mut file = File::open(&path).map_err(|_| {
        Error::unavailable(format!(
            "required model artifact {} is missing",
            path.file_name().unwrap_or_default().to_string_lossy()
        ))
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| Error::unavailable("failed to read model artifact"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = hex_digest(hasher.finalize().as_slice());
    if actual != expected {
        return Err(Error::unavailable(format!(
            "model artifact {} failed SHA-256 verification",
            path.file_name().unwrap_or_default().to_string_lossy()
        )));
    }
    Ok(())
}

fn hex_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_is_frozen_compatibility_profile() {
        let descriptor = descriptor();
        assert_eq!(descriptor.id, "trustmark-q-bch5@1");
        assert_eq!(descriptor.payload_bits, 61);
        assert_eq!(descriptor.artifacts[0].sha256, ENCODER_SHA256);
    }

    #[test]
    fn missing_models_are_unavailable() {
        let directory = std::env::temp_dir().join("underprint-models-that-do-not-exist");
        assert!(verify_models(directory).is_err());
    }

    #[test]
    fn default_runtime_is_bounded() {
        let options = TrustmarkOptions::default();
        assert!((1..=6).contains(&options.intra_threads));
        assert!(!options.cpu_arena);
        assert!(options.memory_pattern);
        assert!(options.prepacking);
    }

    #[test]
    fn rejects_invalid_thread_count_before_loading_artifacts() {
        let error = TrustmarkEngine::load_with_options(
            "/models-do-not-matter",
            TrustmarkOptions {
                intra_threads: 0,
                ..TrustmarkOptions::default()
            },
        )
        .err()
        .expect("zero threads must be rejected");
        assert_eq!(error.kind, underprint::ErrorKind::InvalidArgument);
    }
}
