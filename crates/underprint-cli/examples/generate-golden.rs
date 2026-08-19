use std::{fs, path::Path, sync::Arc};

use image::{DynamicImage, Rgb, RgbImage};
use underprint::{EmbedOptions, Underprint, serialize_png};
use underprint_trustmark::TrustmarkEngine;

const PAYLOAD: &str = "1011011110011000111111000000011111011111011100000110110110111";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let destination = root.join("tests/golden/trustmark-q-bch5-v1");
    fs::create_dir_all(&destination)?;

    let input = synthetic_fixture();
    let input_png = serialize_png(&DynamicImage::ImageRgb8(input), &Default::default())?;

    let mut application = Underprint::default();
    application.register(Arc::new(TrustmarkEngine::load(root.join("models"))?))?;
    let embedding = application.embed(&input_png, PAYLOAD, &EmbedOptions::default())?;
    let detection = application.detect(&embedding.output, &embedding.profile)?;
    if detection.detections[0].payload.as_deref() != Some(PAYLOAD) {
        return Err("generated vector did not recover the exact payload".into());
    }

    fs::write(destination.join("input.png"), &input_png)?;
    fs::write(destination.join("protected.png"), &embedding.output)?;
    let manifest = serde_json::json!({
        "schema": "underprint.golden-vector/v1",
        "generator": {
            "underprint_version": underprint::VERSION,
            "command": "cargo run -p underprint-cli --example generate-golden"
        },
        "profile": embedding.profile,
        "payload": PAYLOAD,
        "selected_strength": embedding.selected_strength,
        "input_sha256": embedding.input.sha256,
        "output_sha256": embedding.output_sha256,
        "artifacts": embedding.artifacts,
        "synthetic": true,
        "production_user_data": false
    });
    fs::write(
        destination.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    println!("wrote {}", destination.display());
    Ok(())
}

fn synthetic_fixture() -> RgbImage {
    RgbImage::from_fn(192, 128, |x, y| {
        let checker = if (x / 16 + y / 16) % 2 == 0 { 28 } else { 0 };
        Rgb([
            ((x * 255 / 191) as u8).saturating_add(checker),
            ((y * 255 / 127) as u8).saturating_add(checker / 2),
            (((x + y) * 255 / 318) as u8).saturating_add(checker / 3),
        ])
    })
}
