use serde::Serialize;

use crate::{
    ABI_VERSION, ArtifactDescriptor, BUILD_SCHEMA, CAPABILITIES_SCHEMA, DETECTION_SCHEMA,
    EMBEDDING_SCHEMA, ERROR_SCHEMA, ProfileDescriptor, VERSION,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BuildInfo {
    pub schema: &'static str,
    pub version: &'static str,
    pub abi_version: u32,
    pub capabilities_schema: &'static str,
    pub detection_schema: &'static str,
    pub embedding_schema: &'static str,
    pub error_schema: &'static str,
}

impl BuildInfo {
    pub const fn current() -> Self {
        Self {
            schema: BUILD_SCHEMA,
            version: VERSION,
            abi_version: ABI_VERSION,
            capabilities_schema: CAPABILITIES_SCHEMA,
            detection_schema: DETECTION_SCHEMA,
            embedding_schema: EMBEDDING_SCHEMA,
            error_schema: ERROR_SCHEMA,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RuntimeConfiguration {
    pub intra_threads: usize,
    pub cpu_arena: bool,
    pub memory_pattern: bool,
    pub prepacking: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilitiesReport {
    pub schema: &'static str,
    pub build: BuildInfo,
    pub ready: bool,
    pub unavailable_reason: Option<String>,
    pub runtime: RuntimeConfiguration,
    pub profiles: Vec<ProfileDescriptor>,
}

impl CapabilitiesReport {
    pub fn new(
        ready: bool,
        unavailable_reason: Option<String>,
        runtime: RuntimeConfiguration,
        profiles: Vec<ProfileDescriptor>,
    ) -> Self {
        Self {
            schema: CAPABILITIES_SCHEMA,
            build: BuildInfo::current(),
            ready,
            unavailable_reason,
            runtime,
            profiles,
        }
    }
}

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
    pub build: BuildInfo,
    pub input: InputSummary,
    pub detections: Vec<Detection>,
    pub partial: bool,
    pub warnings: Vec<String>,
    pub duration_ms: u64,
}

impl DetectionReport {
    pub(crate) fn new(input: InputSummary, detections: Vec<Detection>, duration_ms: u64) -> Self {
        Self {
            schema: DETECTION_SCHEMA,
            build: BuildInfo::current(),
            input,
            detections,
            partial: false,
            warnings: Vec::new(),
            duration_ms,
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
    pub build: BuildInfo,
    pub input: InputSummary,
    pub output_sha256: String,
    pub output_bytes: u64,
    pub algorithm: String,
    pub profile: String,
    pub payload_codec: String,
    pub payload: String,
    pub selected_strength: f64,
    pub self_verified: bool,
    pub artifacts: Vec<ArtifactDescriptor>,
    pub duration_ms: u64,
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
        duration_ms: u64,
    ) -> Self {
        let output_sha256 = crate::media::sha256_hex(&output);
        let output_bytes = output.len() as u64;
        Self {
            schema: EMBEDDING_SCHEMA,
            build: BuildInfo::current(),
            input,
            output_sha256,
            output_bytes,
            algorithm,
            profile,
            payload_codec,
            payload,
            selected_strength: f64::from((selected_strength * 10_000.0).round() as i32) / 10_000.0,
            self_verified: true,
            artifacts,
            duration_ms,
            output,
        }
    }
}
