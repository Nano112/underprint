use std::{
    fs::{self, File},
    io::{self, IsTerminal, Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
    sync::Arc,
};

use clap::{Args, Parser, Subcommand};
use serde::Serialize;
use tempfile::NamedTempFile;
use underprint::{
    ABI_VERSION, BuildInfo, CapabilitiesReport, ERROR_SCHEMA, EmbedOptions, Error, ErrorKind,
    RuntimeConfiguration, TRUSTMARK_Q_BCH5_PROFILE, Underprint, VERSION,
};
use underprint_trustmark::{TrustmarkEngine, TrustmarkOptions, descriptor, verify_models};

#[derive(Debug, Parser)]
#[command(
    name = "underprint",
    version = VERSION,
    about = "Native invisible watermarking and provenance toolkit",
    color = clap::ColorChoice::Never
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
    /// Input image path, or - to read bounded bytes from stdin.
    input: PathBuf,

    /// Output PNG path, or - for binary stdout.
    #[arg(
        short,
        long,
        required_unless_present = "in_place",
        conflicts_with = "in_place"
    )]
    output: Option<PathBuf>,

    /// Atomically replace an existing output path.
    #[arg(long, conflicts_with = "in_place")]
    overwrite: bool,

    /// Atomically replace the input file after successful self-verification.
    #[arg(long)]
    in_place: bool,

    /// Permit binary image output when stdout is a terminal.
    #[arg(long)]
    force: bool,

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
    /// Input image path, or - to read bounded bytes from stdin.
    input: PathBuf,

    #[arg(long, default_value = TRUSTMARK_Q_BCH5_PROFILE)]
    profile: String,

    #[command(flatten)]
    models: ModelArgs,
}

#[derive(Debug, Serialize)]
struct VersionView {
    name: &'static str,
    build: BuildInfo,
    profiles: Vec<underprint::ProfileDescriptor>,
}

#[derive(Debug, Serialize)]
struct ErrorDocument<'a> {
    schema: &'static str,
    code: ErrorKind,
    exit_code: u8,
    message: &'a str,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let json = cli.json;
    match run(cli) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            let code = exit_code(error.kind);
            if json {
                let document = ErrorDocument {
                    schema: ERROR_SCHEMA,
                    code: error.kind,
                    exit_code: code,
                    message: &error.message,
                };
                let _ = serde_json::to_writer(io::stderr().lock(), &document);
                eprintln!();
            } else {
                eprintln!("underprint: {error}");
            }
            ExitCode::from(code)
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
                build: BuildInfo::current(),
                profiles: vec![descriptor()],
            };
            if cli.json {
                print_json(&version)?;
            } else {
                println!("underprint {VERSION} (ABI {ABI_VERSION})");
            }
            Ok(0)
        }
    }
}

fn algorithms(args: AlgorithmsArgs, json: bool) -> Result<u8, Error> {
    match args.command {
        AlgorithmsCommand::List(models) => {
            let capability = capability(&models);
            if json {
                print_json(&capability)?;
            } else {
                let state = if capability.ready {
                    "ready"
                } else {
                    "unavailable"
                };
                println!(
                    "{}\t{}\t{}",
                    capability.profiles[0].id, state, capability.profiles[0].runtime
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
            let capability = capability(&models);
            if json {
                print_json(&capability)?;
            } else {
                println!("profile: {}", capability.profiles[0].id);
                println!("runtime: {}", capability.profiles[0].runtime);
                println!("payload: {} bits", capability.profiles[0].payload_bits);
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
    let capability = capability(&args);
    if json {
        print_json(&capability)?;
    } else {
        println!("{}: ready", capability.profiles[0].id);
        println!("model artifacts verified and native sessions initialized");
    }
    Ok(0)
}

fn embed(args: EmbedArgs, json: bool) -> Result<u8, Error> {
    if args.in_place && is_stdio_path(&args.input) {
        return Err(Error::invalid_argument(
            "--in-place requires a regular input path",
        ));
    }
    let output = if args.in_place {
        args.input.clone()
    } else {
        args.output
            .clone()
            .ok_or_else(|| Error::invalid_argument("--output is required"))?
    };
    if !args.in_place && args.input == output && !is_stdio_path(&args.input) {
        return Err(Error::invalid_argument(
            "input and output paths must differ; use --in-place for atomic replacement",
        ));
    }
    if json && is_stdio_path(&output) {
        return Err(Error::invalid_argument(
            "--json cannot share stdout with binary output; choose an output file",
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
    write_output(
        &output,
        &report.output,
        args.overwrite || args.in_place,
        args.force,
    )?;
    if json {
        print_json(&report)?;
    } else if is_stdio_path(&output) {
        eprintln!(
            "embedded {} at strength {:.1}; wrote {} bytes to stdout",
            report.profile, report.selected_strength, report.output_bytes
        );
    } else {
        println!(
            "embedded {} at strength {:.1} -> {}",
            report.profile,
            report.selected_strength,
            output.display()
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

fn capability(args: &ModelArgs) -> CapabilitiesReport {
    let unavailable_reason = verify_models(&args.models)
        .err()
        .map(|error| error.to_string());
    CapabilitiesReport::new(
        unavailable_reason.is_none(),
        unavailable_reason,
        runtime_configuration(runtime_options(args)),
        vec![descriptor()],
    )
}

fn runtime_configuration(options: TrustmarkOptions) -> RuntimeConfiguration {
    RuntimeConfiguration {
        intra_threads: options.intra_threads,
        cpu_arena: options.cpu_arena,
        memory_pattern: options.memory_pattern,
        prepacking: options.prepacking,
    }
}

fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, Error> {
    if is_stdio_path(path) {
        return read_bounded_reader(io::stdin().lock(), maximum);
    }
    let metadata = fs::metadata(path)
        .map_err(|_| Error::invalid_input(format!("cannot read {}", path.display())))?;
    if !metadata.is_file() {
        return Err(Error::invalid_input("input must be a regular file"));
    }
    let file = File::open(path)
        .map_err(|_| Error::invalid_input(format!("cannot read {}", path.display())))?;
    read_bounded_reader(file, maximum)
}

fn read_bounded_reader(reader: impl Read, maximum: u64) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    reader
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| Error::invalid_input("failed to read image input"))?;
    if bytes.len() as u64 > maximum {
        return Err(Error::resource_limit(
            "image exceeds the 10 MiB input limit",
        ));
    }
    Ok(bytes)
}

fn write_output(path: &Path, bytes: &[u8], overwrite: bool, force: bool) -> Result<(), Error> {
    if is_stdio_path(path) {
        let mut stdout = io::stdout().lock();
        if stdout.is_terminal() && !force {
            return Err(Error::invalid_argument(
                "refusing binary output to a terminal; pass --force or choose a file",
            ));
        }
        stdout
            .write_all(bytes)
            .and_then(|_| stdout.flush())
            .map_err(|_| Error::internal("failed to write protected image to stdout"))?;
        return Ok(());
    }
    atomic_write(path, bytes, overwrite)
}

fn atomic_write(path: &Path, bytes: &[u8], overwrite: bool) -> Result<(), Error> {
    if path.exists() && !overwrite {
        return Err(Error::invalid_argument(format!(
            "output {} already exists; pass --overwrite to replace it",
            path.display()
        )));
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut temporary = NamedTempFile::new_in(parent)
        .map_err(|_| Error::internal("failed to create atomic output file"))?;
    temporary
        .write_all(bytes)
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|_| Error::internal("failed to write protected image"))?;
    if overwrite {
        temporary
            .persist(path)
            .map_err(|_| Error::internal("failed to atomically replace output path"))?;
    } else {
        temporary
            .persist_noclobber(path)
            .map_err(|_| Error::invalid_argument("output appeared during atomic write"))?;
    }
    Ok(())
}

fn is_stdio_path(path: &Path) -> bool {
    path == Path::new("-")
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
        ErrorKind::UntrustedEvidence => 5,
        ErrorKind::ResourceLimit => 6,
        ErrorKind::Algorithm | ErrorKind::Internal => 10,
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn bounded_reader_rejects_one_byte_over_limit() {
        assert_eq!(
            read_bounded_reader(Cursor::new([1_u8; 4]), 4)
                .unwrap()
                .len(),
            4
        );
        assert_eq!(
            read_bounded_reader(Cursor::new([1_u8; 5]), 4)
                .unwrap_err()
                .kind,
            ErrorKind::ResourceLimit
        );
    }

    #[test]
    fn atomic_output_does_not_clobber_without_permission() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("output.png");
        fs::write(&output, b"original").unwrap();
        assert_eq!(
            atomic_write(&output, b"replacement", false)
                .unwrap_err()
                .kind,
            ErrorKind::InvalidArgument
        );
        assert_eq!(fs::read(&output).unwrap(), b"original");
        atomic_write(&output, b"replacement", true).unwrap();
        assert_eq!(fs::read(&output).unwrap(), b"replacement");
    }

    #[test]
    fn in_place_and_output_are_mutually_exclusive() {
        assert!(
            Cli::try_parse_from([
                "underprint",
                "embed",
                "input.png",
                "--in-place",
                "--output",
                "other.png",
                "--payload",
                &"0".repeat(61),
            ])
            .is_err()
        );
    }

    #[test]
    fn binary_stdout_and_json_are_rejected_before_model_loading() {
        let cli = Cli::try_parse_from([
            "underprint",
            "--json",
            "embed",
            "input.png",
            "--output",
            "-",
            "--payload",
            &"0".repeat(61),
        ])
        .unwrap();
        assert_eq!(run(cli).unwrap_err().kind, ErrorKind::InvalidArgument);
    }
}
