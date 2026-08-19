//! Underprint's native application core.
//!
//! This crate owns media limits, profile selection, adaptive embedding policy,
//! and result schemas. Algorithm crates implement [`WatermarkEngine`] and do
//! not duplicate these product rules.

mod engine;
mod error;
mod media;
mod orchestrator;
mod profile;
mod result;

pub use engine::WatermarkEngine;
pub use error::{Error, ErrorKind, Result};
pub use media::{ImagePolicy, LoadedImage, load_image, normalize_for_model, serialize_png};
pub use orchestrator::{EmbedOptions, Underprint};
pub use profile::{ArtifactDescriptor, Capability, ProfileDescriptor};
pub use result::{
    BuildInfo, CapabilitiesReport, Detection, DetectionReport, DetectionState, EmbeddingReport,
    InputSummary, RuntimeConfiguration,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const ABI_VERSION: u32 = 1;
pub const DETECTION_SCHEMA: &str = "underprint.detection/v1";
pub const EMBEDDING_SCHEMA: &str = "underprint.embedding/v1";
pub const CAPABILITIES_SCHEMA: &str = "underprint.capabilities/v1";
pub const ERROR_SCHEMA: &str = "underprint.error/v1";
pub const BUILD_SCHEMA: &str = "underprint.build/v1";
pub const TRUSTMARK_Q_BCH5_PROFILE: &str = "trustmark-q-bch5@1";

/// Validate Schematio's binary BCH-5 payload shape.
pub fn validate_bch5_payload(payload: &str) -> Result<()> {
    if payload.len() != 61 || !payload.bytes().all(|bit| matches!(bit, b'0' | b'1')) {
        return Err(Error::invalid_argument(
            "payload must contain exactly 61 binary bits",
        ));
    }
    Ok(())
}

/// Generate a deterministic, inclusive strength schedule.
pub fn strength_schedule(start: f32, ceiling: f32, step: f32) -> Result<Vec<f32>> {
    if !start.is_finite()
        || !ceiling.is_finite()
        || !step.is_finite()
        || start <= 0.0
        || ceiling < start
        || step <= 0.0
    {
        return Err(Error::invalid_argument(
            "strengths must be finite and positive, and ceiling must be at least start",
        ));
    }

    let mut values = Vec::new();
    let mut current = start;
    while current <= ceiling + 1e-6 {
        values.push((current * 10_000.0).round() / 10_000.0);
        current += step;
    }
    if values.last().is_none_or(|last| *last < ceiling) {
        values.push(ceiling);
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_exact_bch5_payload() {
        assert!(validate_bch5_payload(&("01".repeat(30) + "1")).is_ok());
        for invalid in ["", &"0".repeat(60), &"0".repeat(62), &"x".repeat(61)] {
            assert!(validate_bch5_payload(invalid).is_err());
        }
    }

    #[test]
    fn strength_schedule_matches_schematio_policy() {
        assert_eq!(
            strength_schedule(0.6, 1.0, 0.1).unwrap(),
            vec![0.6, 0.7, 0.8, 0.9, 1.0]
        );
        assert_eq!(
            strength_schedule(0.8, 1.0, 0.1).unwrap(),
            vec![0.8, 0.9, 1.0]
        );
    }

    #[test]
    fn published_schema_contracts_match_runtime_identifiers() {
        let contracts = [
            (
                include_str!("../../../schemas/capabilities-v1.schema.json"),
                CAPABILITIES_SCHEMA,
            ),
            (
                include_str!("../../../schemas/detection-v1.schema.json"),
                DETECTION_SCHEMA,
            ),
            (
                include_str!("../../../schemas/embedding-v1.schema.json"),
                EMBEDDING_SCHEMA,
            ),
            (
                include_str!("../../../schemas/error-v1.schema.json"),
                ERROR_SCHEMA,
            ),
        ];
        for (source, identifier) in contracts {
            let schema: serde_json::Value = serde_json::from_str(source).unwrap();
            assert_eq!(schema["properties"]["schema"]["const"], identifier);
            assert_eq!(
                schema["$schema"],
                "https://json-schema.org/draft/2020-12/schema"
            );
        }
        serde_json::from_str::<serde_json::Value>(include_str!(
            "../../../schemas/common-v1.schema.json"
        ))
        .unwrap();
    }
}
