# Underprint future

Underprint can grow from an image-watermark compatibility library into a
general content marking, provenance, and forensic inspection toolkit. This
document describes that direction. It is intentionally non-normative: concrete
delivery gates remain in [`TODO.md`](TODO.md), while product invariants remain in
[`docs/requirements.md`](docs/requirements.md).

The central principle is that no single watermark is proof of authorship or
authenticity. Underprint should preserve independent observations from
watermarks, signed provenance, metadata, fingerprints, and content structure,
then leave policy conclusions to its caller.

## State of the art by threat model

There is no universal best algorithm. Current systems specialize in
imperceptibility, resilience, payload capacity, localization, temporal media,
or integration with a generator.

| Medium or goal | Candidate | Why it matters | Redistribution position |
|---|---|---|---|
| Robust post-hoc images | [PixelSeal](https://github.com/facebookresearch/videoseal) | Strong open robustness/imperceptibility trade-off and a 256-bit payload | Released code and models are MIT |
| High-capacity images | [ChunkySeal](https://github.com/facebookresearch/videoseal) | A 1,024-bit payload where capacity matters more than minimal size | Released code and models are MIT |
| Localized image detection | [Watermark Anything](https://github.com/facebookresearch/watermark-anything) | Localizes marks, partial removal, splicing, and multiple marked regions | Only the MIT SA-1B checkpoint is eligible; exclude the non-commercial COCO checkpoint |
| Video | [VideoSeal](https://github.com/facebookresearch/videoseal) | Temporal consistency, streaming operation, and a stable 256-bit profile | Released code and models are MIT |
| Audio | [AudioSeal](https://github.com/facebookresearch/audioseal) | Fast streaming detection and sample-level localization | Code and released weights are MIT |
| Text | [TextSeal](https://github.com/facebookresearch/textseal) | Generation-time and post-hoc schemes, localized detection, and contamination research | Toolkit is Apache-2.0; loaded language models retain their own licences |
| Text reference | [SynthID Text](https://github.com/google-deepmind/synthid-text) | Strong published generation-time design with simple and trained detectors | Software is Apache-2.0, but the repository is explicitly a reference implementation |
| Signed provenance | [C2PA](https://c2pa.org/) through [`c2pa-rs`](https://github.com/contentauth/c2pa-rs) | Cryptographically signed claims, ingredients, edits, and trust information | Rust implementation is MIT/Apache-2.0 |
| Closed multimodal system | [SynthID](https://deepmind.google/models/synthid/) | Deployed across image, video, audio, and text in Google's ecosystem | Do not promise redistribution of unavailable image/audio/video implementations |

These labels describe current public results, not permanent rankings. Every
candidate must pass Underprint's own reproducible evaluation before becoming a
supported immutable profile. In particular, neural regeneration and aggressive
editing can remove or replace contemporary post-hoc watermarks.

## Algorithm portfolio

Underprint should add algorithms because they contribute a distinct signal,
not merely to increase the engine count.

### Near-term image portfolio

1. Keep `trustmark-q-bch5@1` as the Schematio compatibility profile.
2. Add C2PA discovery and verification as an independent provenance signal.
3. Evaluate PixelSeal as the robust general-purpose image profile.
4. Evaluate the MIT Watermark Anything checkpoint for localization and
   tamper-region reporting.
5. Define a clean-room classical DWT/DCT profile as a small, fast, model-free
   baseline. Label it as a compatibility signal rather than forensic proof.
6. Add perceptual hashes only as similarity evidence and never as an
   authenticity detector.

ChunkySeal should follow only when a real use case needs its 1,024-bit payload.
StegaStamp-style physical capture, RAW zero-bit detection, and private
organization profiles remain research candidates until their licences,
robustness, false-positive behaviour, and runtime cost are demonstrated.

### Video and audio

VideoSeal should operate on decoded frames while the container layer preserves
audio, subtitles, chapters, metadata, timing, and unknown tracks during remuxing.
AudioSeal should independently mark and inspect audio tracks. A video file may
therefore contain video, audio, C2PA, metadata, and fingerprint observations in
one report without collapsing them into a single score.

### Text

Text watermarking is a separate domain. Generation-time schemes alter token
selection inside a language model; post-hoc schemes necessarily rewrite the
text and can change meaning or style. The public API should consequently expose
explicit `generate`, `rewrite`, and `detect` operations rather than pretending
that text behaves like a raster image.

Text support should also distinguish:

- statistical generation watermarks;
- localized detection;
- post-hoc rewriting;
- dataset contamination or watermark radioactivity;
- cryptographic signing of an unchanged document.

The last item is provenance, not linguistic watermarking, and is often the
correct option when the original wording must remain intact.

## Media families and formats

Underprint should generalize around canonical media families rather than one
engine per file extension.

### Raster images

First-class targets:

- JPEG;
- PNG;
- WebP;
- AVIF;
- TIFF;
- HEIF/HEIC;
- JPEG XL.

Secondary targets include BMP, GIF/APNG, and camera RAW formats. Image engines
operate on a canonical colour representation; adapters remain responsible for
orientation, alpha, colour profiles, bit depth, animation, safe allocation, and
output encoding.

### Video

Target MP4/MOV, WebM, and Matroska first, with H.264, H.265/HEVC, AV1, VP9, and
ProRes as explicit codec capabilities. AVI can be a compatibility format rather
than a design centre. Reports must identify the container, each stream, the
decoder/encoder used, and any lossy conversion.

### Audio

Target WAV, FLAC, MP3, AAC/M4A, Opus/Ogg, AIFF, and ALAC. Decode to bounded,
normalized PCM for algorithms while retaining the original sample rate, channel
layout, and encoding facts in the report.

### Documents and vector content

Target PDF and SVG first, followed by EPUB, DOCX, ODT, and HTML. Document
handling should combine:

- C2PA or another signed provenance envelope;
- traversal and independent marking of embedded raster assets;
- text and image fingerprinting;
- optional production of a separately labelled, watermarked rendered-page
  derivative.

Underprint must not silently rasterize an editable document: doing so destroys
searchability, accessibility, structure, and future editing. C2PA already
defines manifest embedding across multiple image, audio, video, font, and
document formats; support should follow the current specification rather than
inventing incompatible container metadata.

### Structured, 3D, and game assets

Longer-term format families include:

- glTF/GLB, OBJ, and STL;
- CAD exchange formats;
- TTF, OTF, and WOFF2 fonts;
- Minecraft `.schem`, `.litematic`, and NBT data.

These require domain-specific structural algorithms rather than pixel models.
A Minecraft profile is especially relevant to Schematio: it could combine a
signed manifest with a redundant structural mark designed to survive conversion
between Sponge, WorldEdit, and Litematica representations. Any structural
channel must be tested against canonicalization, palette reordering, format
conversion, cropping, rotation, and block substitution before being described
as robust.

### Arbitrary files and source trees

Invisible watermarking is normally the wrong abstraction for archives,
executables, source trees, and model artifacts. Underprint can still cover them
through detached signatures, Merkle manifests, cryptographic hashes, Sigstore
or in-toto attestations, and embedded ownership manifests where a format safely
supports them.

## General architecture

The engine should separate media adaptation from signals and conclusions:

```text
                            Underprint
                                |
             +------------------+------------------+
             |                  |                  |
        Watermarks          Provenance         Fingerprints
             |                  |                  |
     image/video/audio/text    C2PA        cryptographic/perceptual
             |
       canonical media
             |
    format/container adapters
```

The conceptual Rust boundaries are:

```rust
trait MediaAdapter {
    fn probe(&self, input: &[u8]) -> Result<MediaInfo>;
    fn decode(&self, input: &[u8]) -> Result<CanonicalMedia>;
    fn encode(
        &self,
        media: &CanonicalMedia,
        options: &EncodeOptions,
    ) -> Result<Vec<u8>>;
}

trait WatermarkEngine {
    fn capabilities(&self) -> Capabilities;
    fn embed(
        &self,
        media: &CanonicalMedia,
        payload: &[u8],
    ) -> Result<Embedding>;
    fn detect(&self, media: &CanonicalMedia) -> Result<Detection>;
}

trait ProvenanceEngine {
    fn sign(&self, asset: &[u8], claim: &Claim) -> Result<Vec<u8>>;
    fn verify(&self, asset: &[u8]) -> Result<ProvenanceReport>;
}
```

The exact interfaces may evolve, but their responsibilities must remain
separate. Decoding a container, detecting a mark, validating a signature, and
deciding whether an application trusts the result are different operations.

Suggested workspace modules are:

```text
underprint-core
underprint-image
underprint-video
underprint-audio
underprint-text
underprint-document
underprint-provenance
underprint-ffi
underprint-cli
```

Features and release profiles should keep the compatibility library modest.
Schematio should be able to load an image-only shared library without absorbing
video codecs, text-generation runtimes, or every model. The full CLI and service
can assemble the broader suite.

## Stable profile identity

Every supported algorithm must have an immutable profile identifier that pins:

- model and artifact hashes;
- preprocessing and canonical media rules;
- encoder and decoder behaviour;
- payload and error-correction codecs;
- thresholds and calibration version;
- required runtime behaviour;
- output serialization policy.

Possible profile and signal identifiers include:

```text
trustmark-q-bch5@1
wam-sa1b-32@1
pixelseal-256@1
chunkyseal-1024@1
videoseal-256@1
audioseal-16@1
dwt-dct-64@1
c2pa-v2@1
metadata-rights@1
phash64@1
```

A model update must not silently change an existing profile. Profiles may share
implementation code, but they must remain independently detectable for as long
as Underprint claims backward compatibility.

## Detection plans

Multimodal support should preserve bounded, explainable execution:

- `fast`: signatures, metadata, cheap fingerprints, and explicitly selected
  low-cost detectors;
- `balanced`: the primary algorithms for the detected media family;
- `deep`: bounded transforms, regional or temporal search, and all eligible
  engines under explicit CPU, memory, GPU, and deadline budgets.

Successful results survive another engine's timeout or failure. Conflicting
signals remain visible. Raw model scores are not called confidence until they
have been calibrated on a versioned corpus with measured false-positive rates.

## Suggested expansion order

1. Finish the TrustMark production and compatibility gates already tracked in
   `TODO.md`.
2. Add C2PA, metadata inspection, and carefully labelled fingerprints.
3. Add PixelSeal and Watermark Anything as complementary image profiles after
   ONNX parity, CPU, memory, model-licence, and redistribution spikes.
4. Add a clean-room DWT/DCT baseline for model-free deployments.
5. Add VideoSeal with bounded streaming decode and remuxing.
6. Add AudioSeal, including audio tracks inside video containers.
7. Add PDF/SVG traversal and signing without destructive rasterization.
8. Research a versioned Minecraft schematic structural profile.
9. Add text watermarking as a distinct, optional model-integrated domain.
10. Evaluate 3D and other structured formats only with format-specific threat
    models and robustness corpora.

The result should be one consistent toolkit and report model, not one enormous
mandatory binary. Underprint's value is the shared contract: bounded media
handling, immutable profiles, reproducible evidence, honest uncertainty, and
native access from Rust, C, PHP, Go, the CLI, and services.
