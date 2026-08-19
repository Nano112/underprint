#![allow(non_camel_case_types)]

use std::{
    collections::HashMap,
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
    ptr, slice,
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use serde::{Deserialize, Serialize};
use underprint_core::{
    ABI_VERSION, CapabilitiesReport, EmbedOptions, Error, ErrorKind, RuntimeConfiguration,
    TRUSTMARK_Q_BCH5_PROFILE, Underprint,
};
use underprint_trustmark::{TrustmarkEngine, TrustmarkOptions, descriptor};

pub const UP_ABI_VERSION: u32 = ABI_VERSION;

#[repr(C)]
pub struct up_context {
    _private: [u8; 0],
}

#[repr(C)]
pub struct up_result {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct up_bytes_view {
    pub data: *const u8,
    pub len: usize,
}

impl up_bytes_view {
    const EMPTY: Self = Self {
        data: ptr::null(),
        len: 0,
    };
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum up_status {
    UP_OK = 0,
    UP_NOT_DETECTED = 1,
    UP_INVALID_ARGUMENT = 2,
    UP_INVALID_INPUT = 3,
    UP_UNAVAILABLE = 4,
    UP_UNTRUSTED_EVIDENCE = 5,
    UP_RESOURCE_LIMIT = 6,
    UP_INTERNAL = 10,
}

struct ContextEntry {
    application: Option<Underprint>,
    unavailable_reason: Option<String>,
    runtime: RuntimeConfiguration,
}

struct ResultEntry {
    json: Vec<u8>,
    output: Vec<u8>,
}

static CONTEXTS: LazyLock<Mutex<HashMap<usize, Arc<ContextEntry>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static RESULTS: LazyLock<Mutex<HashMap<usize, Arc<ResultEntry>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT_HANDLE: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextConfig {
    models_dir: Option<String>,
    intra_threads: Option<usize>,
    cpu_arena: Option<bool>,
    memory_pattern: Option<bool>,
    prepacking: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct DetectOptions {
    profile: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmbedFfiOptions {
    payload: String,
    profile: Option<String>,
    strength: Option<f32>,
    max_strength: Option<f32>,
    strength_step: Option<f32>,
}

fn runtime_configuration(options: TrustmarkOptions) -> RuntimeConfiguration {
    RuntimeConfiguration {
        intra_threads: options.intra_threads,
        cpu_arena: options.cpu_arena,
        memory_pattern: options.memory_pattern,
        prepacking: options.prepacking,
    }
}

#[derive(Serialize)]
struct ErrorDocument<'a> {
    schema: &'static str,
    code: ErrorKind,
    message: &'a str,
}

#[unsafe(no_mangle)]
pub extern "C" fn up_abi_version() -> u32 {
    UP_ABI_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn up_version() -> up_bytes_view {
    up_bytes_view {
        data: underprint_core::VERSION.as_ptr(),
        len: underprint_core::VERSION.len(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn up_context_create(
    config_json: up_bytes_view,
    out: *mut *mut up_context,
) -> up_status {
    ffi_guard(|| context_create(config_json, out))
}

#[unsafe(no_mangle)]
pub extern "C" fn up_context_capabilities(
    context: *mut up_context,
    out: *mut *mut up_result,
) -> up_status {
    ffi_guard(|| {
        initialize_out(out)?;
        let context = get_context(context)?;
        let document = CapabilitiesReport::new(
            context.application.is_some(),
            context.unavailable_reason.clone(),
            context.runtime,
            vec![descriptor()],
        );
        put_result(out, &document, Vec::new())?;
        Ok(up_status::UP_OK)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn up_detect(
    context: *mut up_context,
    image: up_bytes_view,
    options_json: up_bytes_view,
    out: *mut *mut up_result,
) -> up_status {
    ffi_guard(|| {
        initialize_out(out)?;
        let context = get_context(context)?;
        let source = borrowed_bytes(image, false)?;
        let options: DetectOptions = parse_json(options_json, true)?;
        let application = context.application.as_ref().ok_or_else(|| {
            Error::unavailable(
                context
                    .unavailable_reason
                    .as_deref()
                    .unwrap_or("TrustMark profile is unavailable"),
            )
        });
        let application = match application {
            Ok(application) => application,
            Err(error) => return put_error(out, &error),
        };
        let profile = options
            .profile
            .as_deref()
            .unwrap_or(TRUSTMARK_Q_BCH5_PROFILE);
        match application.detect(source, profile) {
            Ok(report) => {
                let present = report.is_present();
                put_result(out, &report, Vec::new())?;
                Ok(if present {
                    up_status::UP_OK
                } else {
                    up_status::UP_NOT_DETECTED
                })
            }
            Err(error) => put_error(out, &error),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn up_embed(
    context: *mut up_context,
    image: up_bytes_view,
    options_json: up_bytes_view,
    out: *mut *mut up_result,
) -> up_status {
    ffi_guard(|| {
        initialize_out(out)?;
        let context = get_context(context)?;
        let source = borrowed_bytes(image, false)?;
        let options: EmbedFfiOptions = parse_json(options_json, false)?;
        let application = match context.application.as_ref() {
            Some(application) => application,
            None => {
                let error = Error::unavailable(
                    context
                        .unavailable_reason
                        .as_deref()
                        .unwrap_or("TrustMark profile is unavailable"),
                );
                return put_error(out, &error);
            }
        };
        let embed_options = EmbedOptions {
            profile: options
                .profile
                .unwrap_or_else(|| TRUSTMARK_Q_BCH5_PROFILE.to_owned()),
            strength: options.strength.unwrap_or(0.6),
            max_strength: options.max_strength.unwrap_or(1.0),
            strength_step: options.strength_step.unwrap_or(0.1),
        };
        match application.embed(source, &options.payload, &embed_options) {
            Ok(report) => {
                let output = report.output.clone();
                put_result(out, &report, output)?;
                Ok(up_status::UP_OK)
            }
            Err(error) => put_error(out, &error),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn up_verify(
    context: *mut up_context,
    _image: up_bytes_view,
    _evidence: up_bytes_view,
    _options_json: up_bytes_view,
    out: *mut *mut up_result,
) -> up_status {
    ffi_guard(|| {
        initialize_out(out)?;
        let _context = get_context(context)?;
        put_error(
            out,
            &Error::unavailable("portable evidence verification is not compiled in this edition"),
        )
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn up_result_json(result: *mut up_result) -> up_bytes_view {
    ffi_value_guard(|| result_view(result, |entry| &entry.json)).unwrap_or(up_bytes_view::EMPTY)
}

#[unsafe(no_mangle)]
pub extern "C" fn up_result_output(result: *mut up_result) -> up_bytes_view {
    ffi_value_guard(|| result_view(result, |entry| &entry.output)).unwrap_or(up_bytes_view::EMPTY)
}

#[unsafe(no_mangle)]
pub extern "C" fn up_result_free(result: *mut up_result) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !result.is_null()
            && let Ok(mut results) = RESULTS.lock()
        {
            results.remove(&(result as usize));
        }
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn up_context_free(context: *mut up_context) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !context.is_null()
            && let Ok(mut contexts) = CONTEXTS.lock()
        {
            contexts.remove(&(context as usize));
        }
    }));
}

fn context_create(
    config_json: up_bytes_view,
    out: *mut *mut up_context,
) -> Result<up_status, Error> {
    initialize_context_out(out)?;
    let config: ContextConfig = parse_json(config_json, true)?;
    let defaults = TrustmarkOptions::default();
    let options = TrustmarkOptions {
        intra_threads: config.intra_threads.unwrap_or(defaults.intra_threads),
        cpu_arena: config.cpu_arena.unwrap_or(defaults.cpu_arena),
        memory_pattern: config.memory_pattern.unwrap_or(defaults.memory_pattern),
        prepacking: config.prepacking.unwrap_or(defaults.prepacking),
    };
    let (application, unavailable_reason) = if let Some(models_dir) = config.models_dir {
        match TrustmarkEngine::load_with_options(Path::new(&models_dir), options) {
            Ok(engine) => {
                let mut application = Underprint::default();
                application.register(Arc::new(engine))?;
                (Some(application), None)
            }
            Err(error) => (None, Some(error.to_string())),
        }
    } else {
        (
            None,
            Some(
                "models_dir was not configured; capability inspection remains available".to_owned(),
            ),
        )
    };

    let handle = allocate_handle::<up_context>();
    CONTEXTS
        .lock()
        .map_err(|_| Error::internal("context registry is unavailable"))?
        .insert(
            handle as usize,
            Arc::new(ContextEntry {
                application,
                unavailable_reason,
                runtime: runtime_configuration(options),
            }),
        );
    unsafe { out.write(handle) };
    Ok(up_status::UP_OK)
}

fn get_context(handle: *mut up_context) -> Result<Arc<ContextEntry>, Error> {
    if handle.is_null() {
        return Err(Error::invalid_argument("context handle is null"));
    }
    CONTEXTS
        .lock()
        .map_err(|_| Error::internal("context registry is unavailable"))?
        .get(&(handle as usize))
        .cloned()
        .ok_or_else(|| Error::invalid_argument("context handle is invalid or closed"))
}

fn put_result(
    out: *mut *mut up_result,
    value: &impl Serialize,
    output: Vec<u8>,
) -> Result<(), Error> {
    let json = serde_json::to_vec(value)
        .map_err(|_| Error::internal("failed to serialize native result"))?;
    let handle = allocate_handle::<up_result>();
    RESULTS
        .lock()
        .map_err(|_| Error::internal("result registry is unavailable"))?
        .insert(handle as usize, Arc::new(ResultEntry { json, output }));
    unsafe { out.write(handle) };
    Ok(())
}

fn put_error(out: *mut *mut up_result, error: &Error) -> Result<up_status, Error> {
    put_result(
        out,
        &ErrorDocument {
            schema: "underprint.error/v1",
            code: error.kind,
            message: &error.message,
        },
        Vec::new(),
    )?;
    Ok(status_for_error(error.kind))
}

fn result_view(
    handle: *mut up_result,
    select: impl FnOnce(&ResultEntry) -> &[u8],
) -> Result<up_bytes_view, Error> {
    if handle.is_null() {
        return Err(Error::invalid_argument("result handle is null"));
    }
    let result = RESULTS
        .lock()
        .map_err(|_| Error::internal("result registry is unavailable"))?
        .get(&(handle as usize))
        .cloned()
        .ok_or_else(|| Error::invalid_argument("result handle is invalid or freed"))?;
    let bytes = select(&result);
    Ok(if bytes.is_empty() {
        up_bytes_view::EMPTY
    } else {
        up_bytes_view {
            data: bytes.as_ptr(),
            len: bytes.len(),
        }
    })
}

fn parse_json<T>(view: up_bytes_view, empty_is_default: bool) -> Result<T, Error>
where
    T: for<'de> Deserialize<'de> + Default,
{
    let bytes = borrowed_bytes(view, true)?;
    if bytes.is_empty() && empty_is_default {
        return Ok(T::default());
    }
    serde_json::from_slice(bytes).map_err(|_| Error::invalid_argument("JSON options are invalid"))
}

fn borrowed_bytes(view: up_bytes_view, allow_empty: bool) -> Result<&'static [u8], Error> {
    if view.len == 0 {
        if allow_empty {
            return Ok(&[]);
        }
        return Err(Error::invalid_argument("byte buffer is empty"));
    }
    if view.data.is_null() {
        return Err(Error::invalid_argument(
            "byte buffer pointer is null while length is non-zero",
        ));
    }
    if view.len > isize::MAX as usize {
        return Err(Error::resource_limit(
            "byte buffer length is not addressable",
        ));
    }
    // SAFETY: the ABI contract requires callers to provide a readable buffer of
    // exactly `len` bytes and keep it alive for this synchronous call.
    Ok(unsafe { slice::from_raw_parts(view.data, view.len) })
}

fn initialize_out(out: *mut *mut up_result) -> Result<(), Error> {
    if out.is_null() {
        return Err(Error::invalid_argument("result output pointer is null"));
    }
    unsafe { out.write(ptr::null_mut()) };
    Ok(())
}

fn initialize_context_out(out: *mut *mut up_context) -> Result<(), Error> {
    if out.is_null() {
        return Err(Error::invalid_argument("context output pointer is null"));
    }
    unsafe { out.write(ptr::null_mut()) };
    Ok(())
}

fn allocate_handle<T>() -> *mut T {
    loop {
        let value = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
        if value != 0 {
            return value as *mut T;
        }
    }
}

fn status_for_error(kind: ErrorKind) -> up_status {
    match kind {
        ErrorKind::InvalidArgument => up_status::UP_INVALID_ARGUMENT,
        ErrorKind::InvalidInput => up_status::UP_INVALID_INPUT,
        ErrorKind::Unavailable => up_status::UP_UNAVAILABLE,
        ErrorKind::UntrustedEvidence => up_status::UP_UNTRUSTED_EVIDENCE,
        ErrorKind::ResourceLimit => up_status::UP_RESOURCE_LIMIT,
        ErrorKind::Algorithm | ErrorKind::Internal => up_status::UP_INTERNAL,
    }
}

fn ffi_guard(operation: impl FnOnce() -> Result<up_status, Error>) -> up_status {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(status)) => status,
        Ok(Err(error)) => status_for_error(error.kind),
        Err(_) => up_status::UP_INTERNAL,
    }
}

fn ffi_value_guard<T>(operation: impl FnOnce() -> Result<T, Error>) -> Option<T> {
    catch_unwind(AssertUnwindSafe(operation))
        .ok()
        .and_then(Result::ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(bytes: &[u8]) -> up_bytes_view {
        up_bytes_view {
            data: bytes.as_ptr(),
            len: bytes.len(),
        }
    }

    #[test]
    fn creates_lightweight_context_and_reports_capabilities() {
        let mut context = ptr::null_mut();
        assert_eq!(
            up_context_create(up_bytes_view::EMPTY, &mut context),
            up_status::UP_OK
        );
        assert!(!context.is_null());

        let mut result = ptr::null_mut();
        assert_eq!(
            up_context_capabilities(context, &mut result),
            up_status::UP_OK
        );
        let json = up_result_json(result);
        let document: serde_json::Value =
            serde_json::from_slice(unsafe { slice::from_raw_parts(json.data, json.len) }).unwrap();
        assert_eq!(document["build"]["abi_version"], 1);
        assert_eq!(document["ready"], false);
        assert_eq!(document["runtime"]["cpu_arena"], false);
        assert_eq!(document["runtime"]["memory_pattern"], true);
        assert_eq!(document["runtime"]["prepacking"], true);
        assert!(document["runtime"]["intra_threads"].as_u64().unwrap() <= 6);

        up_result_free(result);
        up_context_free(context);
    }

    #[test]
    fn stale_and_wrong_type_handles_do_not_dereference_memory() {
        let mut context = ptr::null_mut();
        assert_eq!(
            up_context_create(up_bytes_view::EMPTY, &mut context),
            up_status::UP_OK
        );
        up_context_free(context);
        up_context_free(context);

        let mut result = ptr::null_mut();
        assert_eq!(
            up_context_capabilities(context, &mut result),
            up_status::UP_INVALID_ARGUMENT
        );
        assert!(result.is_null());
        assert_eq!(up_result_json(context.cast()).len, 0);
    }

    #[test]
    fn rejects_nonzero_length_null_buffer() {
        let mut context = ptr::null_mut();
        let invalid = up_bytes_view {
            data: ptr::null(),
            len: 1,
        };
        assert_eq!(
            up_context_create(invalid, &mut context),
            up_status::UP_INVALID_ARGUMENT
        );
        assert!(context.is_null());
    }

    #[test]
    fn unavailable_embed_still_returns_structured_error() {
        let mut context = ptr::null_mut();
        up_context_create(up_bytes_view::EMPTY, &mut context);
        let options =
            br#"{"payload":"0000000000000000000000000000000000000000000000000000000000000"}"#;
        let image = [1_u8];
        let mut result = ptr::null_mut();
        assert_eq!(
            up_embed(context, view(&image), view(options), &mut result),
            up_status::UP_UNAVAILABLE
        );
        assert!(!result.is_null());
        up_result_free(result);
        up_context_free(context);
    }

    #[test]
    fn arbitrary_handles_and_json_never_cross_or_dereference_foreign_memory() {
        let mut state = 0x6a09_e667_f3bc_c909_u64;
        for length in 0..2_000_usize {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let random_handle = (state as usize | 1) as *mut up_context;
            let mut result = ptr::null_mut();
            assert_eq!(
                up_context_capabilities(random_handle, &mut result),
                up_status::UP_INVALID_ARGUMENT
            );
            assert!(result.is_null());
            assert_eq!(up_result_json(random_handle.cast()).len, 0);
            assert_eq!(up_result_output(random_handle.cast()).len, 0);
            up_context_free(random_handle);
            up_result_free(random_handle.cast());

            let bytes: Vec<u8> = (0..length.min(64))
                .map(|index| state.wrapping_shr((index % 8 * 8) as u32) as u8)
                .collect();
            let mut context = ptr::null_mut();
            let status = up_context_create(view(&bytes), &mut context);
            assert!(matches!(
                status,
                up_status::UP_OK | up_status::UP_INVALID_ARGUMENT
            ));
            up_context_free(context);
        }
    }
}
