use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::Arc,
};

use clap::{Args, Parser, Subcommand};
use serde::Serialize;
use tempfile::NamedTempFile;
use underprint::{EmbedOptions, Error, ErrorKind, TRUSTMARK_Q_BCH5_PROFILE, Underprint, VERSION};
use underprint_trustmark::{TrustmarkEngine, TrustmarkOptions, descriptor, verify_models};

#[derive(Debug, Parser)]
#[command(
    name = "underprint",
    version = VERSION,
    about = "Native invisible watermarking and provenance toolkit"
)]
struct Cli {
    /// Emit stable machine-readable JSON on stdout.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Inspect installed algorithm profiles.
    Algorithms(AlgorithmsArgs),
    /// Validate model artifacts and initialize the native runtime.
    Doctor(ModelArgs),
    /// Embed a payload and self-verify the serialized output.
    Embed(EmbedArgs),
    /// Detect and decode a watermark.
    Detect(DetectArgs),
    /// Print version and schema information.
    Version,
}

#[derive(Debug, Args)]
struct AlgorithmsArgs {
    #[command(subcommand)]
    command: AlgorithmsCommand,
}

#[derive(Debug, Subcommand)]
enum AlgorithmsCommand {
    /// List compiled profiles and their readiness.
    List(ModelArgs),
    /// Inspect one profile.
    Inspect {
        #[arg(default_value = TRUSTMARK_Q_BCH5_PROFILE)]
        profile: String,
        #[command(flatten)]
        models: ModelArgs,
    },
}

#[derive(Debug, Clone, Args)]
struct ModelArgs {
    /// Directory containing pinned ONNX artifacts.
    #[arg(long, env = "UNDERPRINT_MODELS_DIR", default_value = "models")]
    models: PathBuf,

    /// ONNX Runtime threads used inside each model operation.
    #[arg(long, env = "UNDERPRINT_INTRA_THREADS")]
    intra_threads: Option<usize>,

    /// Retain ONNX Runtime CPU arena allocations between calls.
    #[arg(long, env = "UNDERPRINT_CPU_ARENA", default_value_t = false, action = clap::ArgAction::Set)]
    cpu_arena: bool,

    /// Enable ONNX Runtime memory-pattern planning.
    #[arg(long, env = "UNDERPRINT_MEMORY_PATTERN", default_value_t = true, action = clap::ArgAction::Set)]
    memory_pattern: bool,

    /// Prepack constant model weights for faster inference.
    #[arg(long, env = "UNDERPRINT_PREPACKING", default_value_t = true, action = clap::ArgAction::Set)]
    prepacking: bool,
}

#[derive(Debug, Args)]
struct EmbedArgs {
    input: PathBuf,

    #[arg(short, long)]
    output: PathBuf,

    #[arg(long)]
    payload: String,

    #[arg(long, default_value = TRUSTMARK_Q_BCH5_PROFILE)]
    profile: String,

    #[arg(long, default_value_t = 0.6)]
    strength: f32,

    #[arg(long, default_value_t = 1.0)]
    max_strength: f32,

    #[arg(long, default_value_t = 0.1)]
    strength_step: f32,

    #[command(flatten)]
    models: ModelArgs,
}

#[derive(Debug, Args)]
struct DetectArgs {
    input: PathBuf,

    #[arg(long, default_value = TRUSTMARK_Q_BCH5_PROFILE)]
    profile: String,

    #[command(flatten)]
    models: ModelArgs,
}

#[derive(Debug, Serialize)]
struct CapabilityView {
    schema: &'static str,
    profile: underprint::ProfileDescriptor,
    ready: bool,
    unavailable_reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct VersionView<'a> {
    name: &'a str,
    version: &'a str,
    abi_version: u32,
    detection_schema: &'a str,
    embedding_schema: &'a str,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("underprint: {error}");
            ExitCode::from(exit_code(error.kind))
        }
    }
}

fn run(cli: Cli) -> Result<u8, Error> {
    match cli.command {
        Command::Algorithms(args) => algorithms(args, cli.json),
        Command::Doctor(args) => doctor(args, cli.json),
        Command::Embed(args) => embed(args, cli.json),
        Command::Detect(args) => detect(args, cli.json),
        Command::Version => {
            let version = VersionView {
                name: "underprint",
                version: VERSION,
                abi_version: 1,
                detection_schema: underprint::DETECTION_SCHEMA,
                embedding_schema: underprint::EMBEDDING_SCHEMA,
            };
            if cli.json {
                print_json(&version)?;
            } else {
                println!("underprint {VERSION} (ABI 1)");
            }
            Ok(0)
        }
    }
}

fn algorithms(args: AlgorithmsArgs, json: bool) -> Result<u8, Error> {
    match args.command {
        AlgorithmsCommand::List(models) => {
            let capability = capability(&models.models);
            if json {
                print_json(&vec![capability])?;
            } else {
                let state = if capability.ready {
                    "ready"
                } else {
                    "unavailable"
                };
                println!(
                    "{}\t{}\t{}",
                    capability.profile.id, state, capability.profile.runtime
                );
            }
            Ok(0)
        }
        AlgorithmsCommand::Inspect { profile, models } => {
            if profile != TRUSTMARK_Q_BCH5_PROFILE {
                return Err(Error::unavailable(format!(
                    "profile {profile} is not compiled into this build"
                )));
            }
            let capability = capability(&models.models);
            if json {
                print_json(&capability)?;
            } else {
                println!("profile: {}", capability.profile.id);
                println!("runtime: {}", capability.profile.runtime);
                println!("payload: {} bits", capability.profile.payload_bits);
                println!("ready: {}", capability.ready);
                if let Some(reason) = capability.unavailable_reason {
                    println!("reason: {reason}");
                }
            }
            Ok(0)
        }
    }
}

fn doctor(args: ModelArgs, json: bool) -> Result<u8, Error> {
    verify_models(&args.models)?;
    let engine = TrustmarkEngine::load_with_options(&args.models, runtime_options(&args))?;
    engine.initialize()?;
    let capability = capability(&args.models);
    if json {
        print_json(&capability)?;
    } else {
        println!("{}: ready", capability.profile.id);
        println!("model artifacts verified and native sessions initialized");
    }
    Ok(0)
}

fn embed(args: EmbedArgs, json: bool) -> Result<u8, Error> {
    if args.input == args.output {
        return Err(Error::invalid_argument(
            "input and output paths must differ; in-place mode is not implemented",
        ));
    }
    let source = read_bounded(&args.input, 10 * 1024 * 1024)?;
    let underprint = configured(&args.models)?;
    let report = underprint.embed(
        &source,
        &args.payload,
        &EmbedOptions {
            profile: args.profile,
            strength: args.strength,
            max_strength: args.max_strength,
            strength_step: args.strength_step,
        },
    )?;
    atomic_write(&args.output, &report.output)?;
    if json {
        print_json(&report)?;
    } else {
        println!(
            "embedded {} at strength {:.1} -> {}",
            report.profile,
            report.selected_strength,
            args.output.display()
        );
        println!("output sha256: {}", report.output_sha256);
    }
    Ok(0)
}

fn detect(args: DetectArgs, json: bool) -> Result<u8, Error> {
    let source = read_bounded(&args.input, 10 * 1024 * 1024)?;
    let underprint = configured(&args.models)?;
    let report = underprint.detect(&source, &args.profile)?;
    if json {
        print_json(&report)?;
    } else if report.is_present() {
        let detection = &report.detections[0];
        println!("detected {}", detection.profile);
        println!(
            "payload: {}",
            detection.payload.as_deref().unwrap_or_default()
        );
    } else {
        println!("no qualifying watermark detected");
    }
    Ok(if report.is_present() { 0 } else { 1 })
}

fn configured(models: &ModelArgs) -> Result<Underprint, Error> {
    let engine = Arc::new(TrustmarkEngine::load_with_options(
        &models.models,
        runtime_options(models),
    )?);
    let mut underprint = Underprint::default();
    underprint.register(engine)?;
    Ok(underprint)
}

fn runtime_options(args: &ModelArgs) -> TrustmarkOptions {
    let defaults = TrustmarkOptions::default();
    TrustmarkOptions {
        intra_threads: args.intra_threads.unwrap_or(defaults.intra_threads),
        cpu_arena: args.cpu_arena,
        memory_pattern: args.memory_pattern,
        prepacking: args.prepacking,
    }
}

fn capability(models: &Path) -> CapabilityView {
    let unavailable_reason = verify_models(models).err().map(|error| error.to_string());
    CapabilityView {
        schema: underprint::CAPABILITIES_SCHEMA,
        profile: descriptor(),
        ready: unavailable_reason.is_none(),
        unavailable_reason,
    }
}

fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, Error> {
    let metadata = fs::metadata(path)
        .map_err(|_| Error::invalid_input(format!("cannot read {}", path.display())))?;
    if !metadata.is_file() {
        return Err(Error::invalid_input("input must be a regular file"));
    }
    if metadata.len() > maximum {
        return Err(Error::resource_limit(
            "image exceeds the 10 MiB input limit",
        ));
    }
    fs::read(path).map_err(|_| Error::invalid_input(format!("cannot read {}", path.display())))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut temporary = NamedTempFile::new_in(parent)
        .map_err(|_| Error::internal("failed to create atomic output file"))?;
    temporary
        .write_all(bytes)
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|_| Error::internal("failed to write protected image"))?;
    temporary
        .persist(path)
        .map_err(|_| Error::internal("failed to atomically replace output path"))?;
    Ok(())
}

fn print_json(value: &impl Serialize) -> Result<(), Error> {
    serde_json::to_writer(std::io::stdout().lock(), value)
        .map_err(|_| Error::internal("failed to write JSON output"))?;
    println!();
    Ok(())
}

fn exit_code(kind: ErrorKind) -> u8 {
    match kind {
        ErrorKind::InvalidArgument => 2,
        ErrorKind::InvalidInput => 3,
        ErrorKind::Unavailable => 4,
        ErrorKind::ResourceLimit => 6,
        ErrorKind::Algorithm | ErrorKind::Internal => 10,
    }
}
