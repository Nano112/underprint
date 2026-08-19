use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArtifactDescriptor {
    pub name: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Embed,
    Detect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileDescriptor {
    pub id: String,
    pub algorithm: String,
    pub version: u32,
    pub payload_codec: String,
    pub payload_bits: u16,
    pub capabilities: Vec<Capability>,
    pub media_types: Vec<String>,
    pub runtime: String,
    pub artifacts: Vec<ArtifactDescriptor>,
}
