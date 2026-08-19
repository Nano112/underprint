use serde::Serialize;

use crate::{ArtifactDescriptor, DETECTION_SCHEMA, EMBEDDING_SCHEMA};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InputSummary {
    pub sha256: String,
    pub media_type: String,
    pub bytes: u64,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionState {
    Present,
    NotPresent,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Detection {
    pub state: DetectionState,
    pub algorithm: String,
    pub profile: String,
    pub payload_codec: String,
    pub payload: Option<String>,
    pub artifacts: Vec<ArtifactDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DetectionReport {
    pub schema: &'static str,
    pub input: InputSummary,
    pub detections: Vec<Detection>,
    pub partial: bool,
    pub warnings: Vec<String>,
}

impl DetectionReport {
    pub(crate) fn new(input: InputSummary, detections: Vec<Detection>) -> Self {
        Self {
            schema: DETECTION_SCHEMA,
            input,
            detections,
            partial: false,
            warnings: Vec::new(),
        }
    }

    pub fn is_present(&self) -> bool {
        self.detections
            .iter()
            .any(|detection| detection.state == DetectionState::Present)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EmbeddingReport {
    pub schema: &'static str,
    pub input: InputSummary,
    pub output_sha256: String,
    pub output_bytes: u64,
    pub algorithm: String,
    pub profile: String,
    pub payload_codec: String,
    pub payload: String,
    pub selected_strength: f32,
    pub self_verified: bool,
    pub artifacts: Vec<ArtifactDescriptor>,
    #[serde(skip)]
    pub output: Vec<u8>,
}

impl EmbeddingReport {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        input: InputSummary,
        output: Vec<u8>,
        algorithm: String,
        profile: String,
        payload_codec: String,
        payload: String,
        selected_strength: f32,
        artifacts: Vec<ArtifactDescriptor>,
    ) -> Self {
        let output_sha256 = crate::media::sha256_hex(&output);
        let output_bytes = output.len() as u64;
        Self {
            schema: EMBEDDING_SCHEMA,
            input,
            output_sha256,
            output_bytes,
            algorithm,
            profile,
            payload_codec,
            payload,
            selected_strength,
            self_verified: true,
            artifacts,
            output,
        }
    }
}
