use std::{collections::BTreeMap, sync::Arc, time::Instant};

use crate::{
    Detection, DetectionReport, DetectionState, EmbeddingReport, Error, ImagePolicy,
    ProfileDescriptor, Result, TRUSTMARK_Q_BCH5_PROFILE, WatermarkEngine, load_image,
    normalize_for_model, serialize_png, strength_schedule, validate_bch5_payload,
};

#[derive(Debug, Clone)]
pub struct EmbedOptions {
    pub profile: String,
    pub strength: f32,
    pub max_strength: f32,
    pub strength_step: f32,
}

impl Default for EmbedOptions {
    fn default() -> Self {
        Self {
            profile: TRUSTMARK_Q_BCH5_PROFILE.to_owned(),
            strength: 0.6,
            max_strength: 1.0,
            strength_step: 0.1,
        }
    }
}

pub struct Underprint {
    policy: ImagePolicy,
    engines: BTreeMap<String, Arc<dyn WatermarkEngine>>,
}

impl Default for Underprint {
    fn default() -> Self {
        Self::new(ImagePolicy::default())
    }
}

impl Underprint {
    pub fn new(policy: ImagePolicy) -> Self {
        Self {
            policy,
            engines: BTreeMap::new(),
        }
    }

    pub fn register(&mut self, engine: Arc<dyn WatermarkEngine>) -> Result<()> {
        let id = engine.descriptor().id.clone();
        if self.engines.insert(id.clone(), engine).is_some() {
            return Err(Error::invalid_argument(format!(
                "profile {id} is already registered"
            )));
        }
        Ok(())
    }

    pub fn profiles(&self) -> Vec<ProfileDescriptor> {
        self.engines
            .values()
            .map(|engine| engine.descriptor().clone())
            .collect()
    }

    pub fn detect(&self, source: &[u8], profile: &str) -> Result<DetectionReport> {
        let started = Instant::now();
        let engine = self.engine(profile)?;
        let loaded = load_image(source, &self.policy)?;
        let image = normalize_for_model(loaded.image, &self.policy);
        let payload = engine.detect(&image)?;
        if let Some(payload) = payload.as_deref() {
            validate_bch5_payload(payload)?;
        }
        let descriptor = engine.descriptor();
        let detection = Detection {
            state: if payload.is_some() {
                DetectionState::Present
            } else {
                DetectionState::NotPresent
            },
            algorithm: descriptor.algorithm.clone(),
            profile: descriptor.id.clone(),
            payload_codec: descriptor.payload_codec.clone(),
            payload,
            artifacts: descriptor.artifacts.clone(),
        };
        Ok(DetectionReport::new(
            loaded.summary,
            vec![detection],
            elapsed_ms(started),
        ))
    }

    pub fn embed(
        &self,
        source: &[u8],
        payload: &str,
        options: &EmbedOptions,
    ) -> Result<EmbeddingReport> {
        let started = Instant::now();
        validate_bch5_payload(payload)?;
        let engine = self.engine(&options.profile)?;
        let loaded = load_image(source, &self.policy)?;
        let image = normalize_for_model(loaded.image, &self.policy);

        for strength in strength_schedule(
            options.strength,
            options.max_strength,
            options.strength_step,
        )? {
            let protected = engine.embed(&image, payload, strength)?;
            let output = serialize_png(&protected, &self.policy)?;
            let serialized = load_image(&output, &self.policy)?;
            if engine.detect(&serialized.image)?.as_deref() == Some(payload) {
                let descriptor = engine.descriptor();
                return Ok(EmbeddingReport::new(
                    loaded.summary,
                    output,
                    descriptor.algorithm.clone(),
                    descriptor.id.clone(),
                    descriptor.payload_codec.clone(),
                    payload.to_owned(),
                    strength,
                    descriptor.artifacts.clone(),
                    elapsed_ms(started),
                ));
            }
        }

        Err(Error::algorithm(
            "serialized watermark self-verification failed after exhausting strength schedule",
        ))
    }

    fn engine(&self, profile: &str) -> Result<&Arc<dyn WatermarkEngine>> {
        self.engines
            .get(profile)
            .ok_or_else(|| Error::unavailable(format!("profile {profile} is unavailable")))
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use image::DynamicImage;

    use super::*;
    use crate::{ArtifactDescriptor, Capability};

    struct FakeEngine {
        descriptor: ProfileDescriptor,
        minimum_strength: f32,
        strengths: Mutex<Vec<f32>>,
        payload: Mutex<Option<String>>,
    }

    impl FakeEngine {
        fn new(minimum_strength: f32) -> Self {
            Self {
                descriptor: ProfileDescriptor {
                    id: TRUSTMARK_Q_BCH5_PROFILE.to_owned(),
                    algorithm: "fake".to_owned(),
                    version: 1,
                    payload_codec: "binary-bch5".to_owned(),
                    payload_bits: 61,
                    capabilities: vec![Capability::Embed, Capability::Detect],
                    media_types: vec!["image/png".to_owned()],
                    runtime: "test".to_owned(),
                    artifacts: vec![ArtifactDescriptor {
                        name: "fake".to_owned(),
                        sha256: "0".repeat(64),
                    }],
                },
                minimum_strength,
                strengths: Mutex::new(Vec::new()),
                payload: Mutex::new(None),
            }
        }
    }

    impl WatermarkEngine for FakeEngine {
        fn descriptor(&self) -> &ProfileDescriptor {
            &self.descriptor
        }

        fn embed(
            &self,
            image: &DynamicImage,
            payload: &str,
            strength: f32,
        ) -> Result<DynamicImage> {
            self.strengths.lock().unwrap().push(strength);
            *self.payload.lock().unwrap() = Some(payload.to_owned());
            Ok(image.clone())
        }

        fn detect(&self, _image: &DynamicImage) -> Result<Option<String>> {
            let verified = self
                .strengths
                .lock()
                .unwrap()
                .last()
                .is_some_and(|strength| *strength >= self.minimum_strength);
            Ok(verified.then(|| self.payload.lock().unwrap().clone().unwrap()))
        }
    }

    fn source() -> Vec<u8> {
        serialize_png(&DynamicImage::new_rgb8(320, 180), &Default::default()).unwrap()
    }

    #[test]
    fn embed_stops_at_first_verified_strength() {
        let engine = Arc::new(FakeEngine::new(0.8));
        let mut underprint = Underprint::default();
        underprint.register(engine.clone()).unwrap();
        let report = underprint
            .embed(&source(), &"0".repeat(61), &EmbedOptions::default())
            .unwrap();
        assert_eq!(report.selected_strength, 0.8);
        assert_eq!(*engine.strengths.lock().unwrap(), vec![0.6, 0.7, 0.8]);
        assert!(report.output.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn embed_fails_closed_at_ceiling() {
        let engine = Arc::new(FakeEngine::new(1.1));
        let mut underprint = Underprint::default();
        underprint.register(engine.clone()).unwrap();
        assert!(
            underprint
                .embed(&source(), &"0".repeat(61), &EmbedOptions::default())
                .is_err()
        );
        assert_eq!(
            *engine.strengths.lock().unwrap(),
            vec![0.6, 0.7, 0.8, 0.9, 1.0]
        );
    }
}
