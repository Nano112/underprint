use image::DynamicImage;

use crate::{ProfileDescriptor, Result};

/// A native algorithm engine. Implementations may use linked native runtimes,
/// but must not spawn or call an implementation in another language.
pub trait WatermarkEngine: Send + Sync {
    fn descriptor(&self) -> &ProfileDescriptor;

    fn embed(&self, image: &DynamicImage, payload: &str, strength: f32) -> Result<DynamicImage>;

    fn detect(&self, image: &DynamicImage) -> Result<Option<String>>;
}
