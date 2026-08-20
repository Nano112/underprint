use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{
        DefaultBodyLimit, Multipart, State,
        multipart::{MultipartError, MultipartRejection},
    },
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Serialize;
use tokio::sync::Semaphore;
use tower::ServiceBuilder;
use tower_http::{
    catch_panic::CatchPanicLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    sensitive_headers::SetSensitiveRequestHeadersLayer,
    timeout::{RequestBodyDeadlineLayer, TimeoutLayer},
    trace::TraceLayer,
};
use underprint::{
    CapabilitiesReport, DetectionReport, DetectionState, ERROR_SCHEMA, EmbedOptions,
    EmbeddingReport, Error, ErrorKind, RuntimeConfiguration, TRUSTMARK_Q_BCH5_PROFILE, Underprint,
};

pub const MAX_UPLOAD_BYTES: usize = 10 * 1024 * 1024;
const MAX_MULTIPART_BYTES: usize = MAX_UPLOAD_BYTES + 256 * 1024;
pub const OPENAPI: &str = include_str!("../../../docs/openapi.yaml");
const COMMON_SCHEMA: &str = include_str!("../../../schemas/common-v1.schema.json");
const CAPABILITIES_SCHEMA_JSON: &str = include_str!("../../../schemas/capabilities-v1.schema.json");
const DETECTION_SCHEMA_JSON: &str = include_str!("../../../schemas/detection-v1.schema.json");
const EMBEDDING_SCHEMA_JSON: &str = include_str!("../../../schemas/embedding-v1.schema.json");
const ERROR_SCHEMA_JSON: &str = include_str!("../../../schemas/error-v1.schema.json");
const SCHEMATIO_MODEL: &str = "trustmark-q-bch5";

#[derive(Clone)]
pub struct AppState {
    application: Option<Arc<Underprint>>,
    capabilities: CapabilitiesReport,
    auth_token: Option<Arc<str>>,
    concurrency: Arc<Semaphore>,
    rate: Arc<Mutex<TokenBucket>>,
    metrics: Arc<Metrics>,
}

impl AppState {
    pub fn ready(
        application: Underprint,
        capabilities: CapabilitiesReport,
        auth_token: Option<String>,
        concurrency: usize,
        requests_per_second: u32,
    ) -> Self {
        Self {
            application: Some(Arc::new(application)),
            capabilities,
            auth_token: auth_token.map(Arc::from),
            concurrency: Arc::new(Semaphore::new(concurrency.max(1))),
            rate: Arc::new(Mutex::new(TokenBucket::new(requests_per_second.max(1)))),
            metrics: Arc::new(Metrics::default()),
        }
    }

    pub fn unavailable(capabilities: CapabilitiesReport) -> Self {
        Self {
            application: None,
            capabilities,
            auth_token: None,
            concurrency: Arc::new(Semaphore::new(1)),
            rate: Arc::new(Mutex::new(TokenBucket::new(1))),
            metrics: Arc::new(Metrics::default()),
        }
    }
}

#[derive(Default)]
struct Metrics {
    requests: AtomicU64,
    embedding_requests: AtomicU64,
    detection_requests: AtomicU64,
    successes: AtomicU64,
    failures: AtomicU64,
    rejected: AtomicU64,
    rate_rejected: AtomicU64,
    concurrency_rejected: AtomicU64,
    duration_micros: AtomicU64,
    embedding_strength_tenths: AtomicU64,
    embedding_strength_observations: AtomicU64,
}

struct TokenBucket {
    tokens: f64,
    capacity: f64,
    refill_per_second: f64,
    updated: Instant,
}

impl TokenBucket {
    fn new(rate: u32) -> Self {
        let capacity = f64::from(rate);
        Self {
            tokens: capacity,
            capacity,
            refill_per_second: capacity,
            updated: Instant::now(),
        }
    }

    fn admit(&mut self) -> bool {
        let now = Instant::now();
        self.tokens = (self.tokens
            + now.duration_since(self.updated).as_secs_f64() * self.refill_per_second)
            .min(self.capacity);
        self.updated = now;
        if self.tokens < 1.0 {
            return false;
        }
        self.tokens -= 1.0;
        true
    }
}

pub fn router(state: AppState, request_timeout: Duration) -> Router {
    let middleware = ServiceBuilder::new()
        .layer(SetSensitiveRequestHeadersLayer::new([
            header::AUTHORIZATION,
            header::COOKIE,
        ]))
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(CatchPanicLayer::new())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::GATEWAY_TIMEOUT,
            request_timeout,
        ))
        .layer(RequestBodyDeadlineLayer::new(request_timeout));

    Router::new()
        .route("/health", get(health))
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/metrics", get(metrics))
        .route("/openapi.yaml", get(openapi))
        .route("/schemas/common-v1.schema.json", get(common_schema))
        .route(
            "/schemas/capabilities-v1.schema.json",
            get(capabilities_schema),
        )
        .route("/schemas/detection-v1.schema.json", get(detection_schema))
        .route("/schemas/embedding-v1.schema.json", get(embedding_schema))
        .route("/schemas/error-v1.schema.json", get(error_schema))
        .route("/embed", post(embed_schematio))
        .route("/decode", post(detect_schematio))
        .route("/v1/algorithms", get(algorithms))
        .route("/v1/embeddings", post(embed_v1))
        .route("/v1/detections", post(detect_v1))
        .route("/v1/verifications", post(verify))
        .layer(DefaultBodyLimit::max(MAX_MULTIPART_BYTES))
        .layer(middleware)
        .with_state(state)
}

#[derive(Serialize)]
struct Health<'a> {
    status: &'a str,
}

async fn health(State(state): State<AppState>) -> Response {
    if state.application.is_some() {
        (StatusCode::OK, Json(Health { status: "ok" })).into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(Health {
                status: "unavailable",
            }),
        )
            .into_response()
    }
}

async fn live() -> Json<Health<'static>> {
    Json(Health { status: "live" })
}

async fn ready(State(state): State<AppState>) -> Response {
    let status = if state.application.is_some() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(state.capabilities)).into_response()
}

async fn algorithms(State(state): State<AppState>, headers: HeaderMap) -> ApiResult {
    authorize(&state, &headers)?;
    Ok(Json(state.capabilities).into_response())
}

async fn embed_v1(
    State(state): State<AppState>,
    headers: HeaderMap,
    multipart: Result<Multipart, MultipartRejection>,
) -> ApiResult {
    let fields = parse_multipart(multipart.map_err(ApiError::from_multipart_rejection)?).await?;
    let report = run_embedding(&state, &headers, fields, "payload").await?;
    embedding_response(report, false)
}

async fn embed_schematio(
    State(state): State<AppState>,
    headers: HeaderMap,
    multipart: Result<Multipart, MultipartRejection>,
) -> ApiResult {
    let fields = parse_multipart(multipart.map_err(ApiError::from_multipart_rejection)?).await?;
    let report = run_embedding(&state, &headers, fields, "token").await?;
    embedding_response(report, true)
}

async fn detect_v1(
    State(state): State<AppState>,
    headers: HeaderMap,
    multipart: Result<Multipart, MultipartRejection>,
) -> ApiResult {
    let fields = parse_multipart(multipart.map_err(ApiError::from_multipart_rejection)?).await?;
    let report = run_detection(&state, &headers, fields).await?;
    Ok(Json(report).into_response())
}

#[derive(Serialize)]
struct SchematioDetection<'a> {
    present: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<&'a str>,
    model: &'static str,
}

async fn detect_schematio(
    State(state): State<AppState>,
    headers: HeaderMap,
    multipart: Result<Multipart, MultipartRejection>,
) -> ApiResult {
    let fields = parse_multipart(multipart.map_err(ApiError::from_multipart_rejection)?).await?;
    let report = run_detection(&state, &headers, fields).await?;
    let payload = report
        .detections
        .iter()
        .find(|detection| detection.state == DetectionState::Present)
        .and_then(|detection| detection.payload.as_deref());
    Ok(Json(SchematioDetection {
        present: payload.is_some(),
        token: payload,
        model: SCHEMATIO_MODEL,
    })
    .into_response())
}

async fn run_embedding(
    state: &AppState,
    headers: &HeaderMap,
    fields: HashMap<String, Vec<u8>>,
    payload_field: &str,
) -> Result<EmbeddingReport, ApiError> {
    authorize(state, headers)?;
    let started = Instant::now();
    state.metrics.requests.fetch_add(1, Ordering::Relaxed);
    state
        .metrics
        .embedding_requests
        .fetch_add(1, Ordering::Relaxed);
    let permit = admit(state)?;
    let image = required_field(&fields, "image")?.clone();
    let payload = required_text(&fields, payload_field)?;
    let profile =
        optional_text(&fields, "profile").unwrap_or_else(|| TRUSTMARK_Q_BCH5_PROFILE.to_owned());
    let application = application(state)?;
    let result = tokio::task::spawn_blocking(move || {
        application.embed(
            &image,
            &payload,
            &EmbedOptions {
                profile,
                ..EmbedOptions::default()
            },
        )
    })
    .await
    .map_err(|_| ApiError::internal("embedding worker failed"))?;
    drop(permit);
    match result {
        Ok(report) => {
            record(state, started, true);
            state.metrics.embedding_strength_tenths.fetch_add(
                (report.selected_strength * 10.0).round() as u64,
                Ordering::Relaxed,
            );
            state
                .metrics
                .embedding_strength_observations
                .fetch_add(1, Ordering::Relaxed);
            Ok(report)
        }
        Err(error) => {
            record(state, started, false);
            Err(error.into())
        }
    }
}

fn embedding_response(report: EmbeddingReport, schematio: bool) -> ApiResult {
    let report_json = serde_json::to_vec(&report)
        .map_err(|_| ApiError::internal("failed to serialize embedding report"))?;
    let token = report.payload.clone();
    let strength = report.selected_strength.to_string();
    let mut response = ([(header::CONTENT_TYPE, "image/png")], report.output).into_response();
    response.headers_mut().insert(
        "x-underprint-report",
        HeaderValue::from_str(&URL_SAFE_NO_PAD.encode(report_json))
            .map_err(|_| ApiError::internal("embedding report header is invalid"))?,
    );
    if schematio {
        response.headers_mut().insert(
            "x-watermark-token",
            HeaderValue::from_str(&token)
                .map_err(|_| ApiError::internal("watermark token header is invalid"))?,
        );
        response.headers_mut().insert(
            "x-watermark-model",
            HeaderValue::from_static(SCHEMATIO_MODEL),
        );
        response.headers_mut().insert(
            "x-watermark-strength",
            HeaderValue::from_str(&strength)
                .map_err(|_| ApiError::internal("watermark strength header is invalid"))?,
        );
    }
    Ok(response)
}

async fn run_detection(
    state: &AppState,
    headers: &HeaderMap,
    fields: HashMap<String, Vec<u8>>,
) -> Result<DetectionReport, ApiError> {
    authorize(state, headers)?;
    let started = Instant::now();
    state.metrics.requests.fetch_add(1, Ordering::Relaxed);
    state
        .metrics
        .detection_requests
        .fetch_add(1, Ordering::Relaxed);
    let permit = admit(state)?;
    let image = required_field(&fields, "image")?.clone();
    let profile =
        optional_text(&fields, "profile").unwrap_or_else(|| TRUSTMARK_Q_BCH5_PROFILE.to_owned());
    let application = application(state)?;
    let result = tokio::task::spawn_blocking(move || application.detect(&image, &profile))
        .await
        .map_err(|_| ApiError::internal("detection worker failed"))?;
    drop(permit);
    match result {
        Ok(report) => {
            record(state, started, true);
            Ok(report)
        }
        Err(error) => {
            record(state, started, false);
            Err(error.into())
        }
    }
}

async fn verify(State(state): State<AppState>, headers: HeaderMap) -> ApiResult {
    authorize(&state, &headers)?;
    Err(ApiError::from(Error::unavailable(
        "portable evidence verification is not compiled in this edition",
    )))
}

async fn metrics(State(state): State<AppState>) -> String {
    let ready = u8::from(state.application.is_some());
    format!(
        concat!(
            "# TYPE underprint_requests_total counter\nunderprint_requests_total {}\n",
            "underprint_operation_requests_total{{operation=\"embedding\",profile=\"trustmark-q-bch5@1\"}} {}\n",
            "underprint_operation_requests_total{{operation=\"detection\",profile=\"trustmark-q-bch5@1\"}} {}\n",
            "# TYPE underprint_successes_total counter\nunderprint_successes_total {}\n",
            "# TYPE underprint_failures_total counter\nunderprint_failures_total {}\n",
            "# TYPE underprint_rejections_total counter\nunderprint_rejections_total {}\n",
            "underprint_resource_rejections_total{{resource=\"rate\"}} {}\n",
            "underprint_resource_rejections_total{{resource=\"concurrency\"}} {}\n",
            "# TYPE underprint_duration_microseconds_total counter\nunderprint_duration_microseconds_total {}\n",
            "# TYPE underprint_queue_wait_microseconds_total counter\nunderprint_queue_wait_microseconds_total 0\n",
            "# TYPE underprint_embedding_strength_tenths_total counter\nunderprint_embedding_strength_tenths_total {}\n",
            "# TYPE underprint_embedding_strength_observations_total counter\nunderprint_embedding_strength_observations_total {}\n",
            "# TYPE underprint_model_ready gauge\nunderprint_model_ready{{profile=\"trustmark-q-bch5@1\"}} {}\n"
        ),
        state.metrics.requests.load(Ordering::Relaxed),
        state.metrics.embedding_requests.load(Ordering::Relaxed),
        state.metrics.detection_requests.load(Ordering::Relaxed),
        state.metrics.successes.load(Ordering::Relaxed),
        state.metrics.failures.load(Ordering::Relaxed),
        state.metrics.rejected.load(Ordering::Relaxed),
        state.metrics.rate_rejected.load(Ordering::Relaxed),
        state.metrics.concurrency_rejected.load(Ordering::Relaxed),
        state.metrics.duration_micros.load(Ordering::Relaxed),
        state
            .metrics
            .embedding_strength_tenths
            .load(Ordering::Relaxed),
        state
            .metrics
            .embedding_strength_observations
            .load(Ordering::Relaxed),
        ready,
    )
}

async fn openapi() -> Response {
    (
        [(header::CONTENT_TYPE, "application/yaml; charset=utf-8")],
        OPENAPI,
    )
        .into_response()
}

fn json_schema(source: &'static str) -> Response {
    ([(header::CONTENT_TYPE, "application/schema+json")], source).into_response()
}

async fn common_schema() -> Response {
    json_schema(COMMON_SCHEMA)
}

async fn capabilities_schema() -> Response {
    json_schema(CAPABILITIES_SCHEMA_JSON)
}

async fn detection_schema() -> Response {
    json_schema(DETECTION_SCHEMA_JSON)
}

async fn embedding_schema() -> Response {
    json_schema(EMBEDDING_SCHEMA_JSON)
}

async fn error_schema() -> Response {
    json_schema(ERROR_SCHEMA_JSON)
}

fn authorize(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let Some(expected) = state.auth_token.as_deref() else {
        return Ok(());
    };
    let provided = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if provided.is_some_and(|value| constant_time_equal(value.as_bytes(), expected.as_bytes())) {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "authentication required",
        ))
    }
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        difference |= usize::from(
            left.get(index).copied().unwrap_or(0) ^ right.get(index).copied().unwrap_or(0),
        );
    }
    difference == 0
}

fn admit(state: &AppState) -> Result<tokio::sync::OwnedSemaphorePermit, ApiError> {
    let admitted = state
        .rate
        .lock()
        .map_err(|_| ApiError::internal("rate limiter unavailable"))?
        .admit();
    if !admitted {
        state.metrics.rejected.fetch_add(1, Ordering::Relaxed);
        state.metrics.rate_rejected.fetch_add(1, Ordering::Relaxed);
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "resource_limit",
            "request rate limit exceeded",
        ));
    }
    state.concurrency.clone().try_acquire_owned().map_err(|_| {
        state.metrics.rejected.fetch_add(1, Ordering::Relaxed);
        state
            .metrics
            .concurrency_rejected
            .fetch_add(1, Ordering::Relaxed);
        ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "resource_limit",
            "service concurrency limit reached",
        )
    })
}

fn application(state: &AppState) -> Result<Arc<Underprint>, ApiError> {
    state.application.clone().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "required profile is not ready",
        )
    })
}

fn record(state: &AppState, started: Instant, success: bool) {
    let counter = if success {
        &state.metrics.successes
    } else {
        &state.metrics.failures
    };
    counter.fetch_add(1, Ordering::Relaxed);
    state.metrics.duration_micros.fetch_add(
        u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
        Ordering::Relaxed,
    );
}

async fn parse_multipart(mut multipart: Multipart) -> Result<HashMap<String, Vec<u8>>, ApiError> {
    let mut fields = HashMap::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(ApiError::from_multipart)?
    {
        let Some(name) = field.name().map(str::to_owned) else {
            continue;
        };
        if !matches!(
            name.as_str(),
            "image" | "file" | "payload" | "profile" | "token"
        ) {
            return Err(ApiError::invalid_argument("unexpected multipart field"));
        }
        let value = field.bytes().await.map_err(ApiError::from_multipart)?;
        if fields.insert(name, value.to_vec()).is_some() {
            return Err(ApiError::invalid_argument("duplicate multipart field"));
        }
    }
    if let Some(file) = fields.remove("file")
        && fields.insert("image".to_owned(), file).is_some()
    {
        return Err(ApiError::invalid_argument("send image or file, not both"));
    }
    Ok(fields)
}

fn required_field<'a>(
    fields: &'a HashMap<String, Vec<u8>>,
    name: &str,
) -> Result<&'a Vec<u8>, ApiError> {
    fields
        .get(name)
        .filter(|bytes| !bytes.is_empty())
        .ok_or_else(|| ApiError::invalid_argument("required multipart field is missing"))
}

fn required_text(fields: &HashMap<String, Vec<u8>>, name: &str) -> Result<String, ApiError> {
    optional_text(fields, name)
        .ok_or_else(|| ApiError::invalid_argument("required text field is missing or invalid"))
}

fn optional_text(fields: &HashMap<String, Vec<u8>>, name: &str) -> Option<String> {
    let value = fields.get(name)?;
    if value.len() > 256 {
        return None;
    }
    std::str::from_utf8(value).ok().map(str::to_owned)
}

type ApiResult = Result<Response, ApiError>;

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: &'static str,
}

impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: &'static str) -> Self {
        Self {
            status,
            code,
            message,
        }
    }

    fn invalid_argument(message: &'static str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_argument", message)
    }

    fn internal(message: &'static str) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", message)
    }

    fn from_multipart(_error: MultipartError) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "invalid_input",
            "invalid or oversized multipart request",
        )
    }

    fn from_multipart_rejection(_error: MultipartRejection) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "invalid_input",
            "request must use bounded multipart/form-data",
        )
    }
}

impl From<Error> for ApiError {
    fn from(error: Error) -> Self {
        let (status, message) = match error.kind {
            ErrorKind::InvalidArgument => (StatusCode::BAD_REQUEST, "invalid operation options"),
            ErrorKind::InvalidInput => (StatusCode::UNPROCESSABLE_ENTITY, "invalid image input"),
            ErrorKind::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "required profile is unavailable",
            ),
            ErrorKind::UntrustedEvidence => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "evidence is invalid or untrusted",
            ),
            ErrorKind::ResourceLimit => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "operation resource limit exceeded",
            ),
            ErrorKind::Algorithm | ErrorKind::Internal => {
                (StatusCode::INTERNAL_SERVER_ERROR, "operation failed")
            }
        };
        Self::new(status, error_kind_code(error.kind), message)
    }
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    schema: &'static str,
    code: &'a str,
    message: &'a str,
}

fn error_kind_code(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::InvalidArgument => "invalid_argument",
        ErrorKind::InvalidInput => "invalid_input",
        ErrorKind::Unavailable => "unavailable",
        ErrorKind::UntrustedEvidence => "untrusted_evidence",
        ErrorKind::ResourceLimit => "resource_limit",
        ErrorKind::Algorithm => "algorithm",
        ErrorKind::Internal => "internal",
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                schema: ERROR_SCHEMA,
                code: self.code,
                message: self.message,
            }),
        )
            .into_response()
    }
}

pub fn default_runtime() -> RuntimeConfiguration {
    let defaults = underprint_trustmark::TrustmarkOptions::default();
    RuntimeConfiguration {
        intra_threads: defaults.intra_threads,
        cpu_arena: defaults.cpu_arena,
        memory_pattern: defaults.memory_pattern,
        prepacking: defaults.prepacking,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use image::DynamicImage;
    use tower::ServiceExt;
    use underprint::{
        ArtifactDescriptor, Capability, ImagePolicy, ProfileDescriptor, Result, WatermarkEngine,
        serialize_png,
    };
    use underprint_trustmark::descriptor;

    fn state() -> AppState {
        AppState::unavailable(CapabilitiesReport::new(
            false,
            Some("unavailable".to_owned()),
            default_runtime(),
            vec![descriptor()],
        ))
    }

    struct FakeEngine {
        descriptor: ProfileDescriptor,
        payload: Mutex<Option<String>>,
    }

    impl FakeEngine {
        fn new() -> Self {
            Self {
                descriptor: ProfileDescriptor {
                    id: TRUSTMARK_Q_BCH5_PROFILE.to_owned(),
                    algorithm: "trustmark".to_owned(),
                    version: 1,
                    payload_codec: "binary-bch5".to_owned(),
                    payload_bits: 61,
                    capabilities: vec![Capability::Embed, Capability::Detect],
                    media_types: vec!["image/png".to_owned()],
                    runtime: "test".to_owned(),
                    artifacts: vec![ArtifactDescriptor {
                        name: "test-model".to_owned(),
                        sha256: "0".repeat(64),
                    }],
                },
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
            _strength: f32,
        ) -> Result<DynamicImage> {
            *self.payload.lock().unwrap() = Some(payload.to_owned());
            Ok(image.clone())
        }

        fn detect(&self, _image: &DynamicImage) -> Result<Option<String>> {
            Ok(self.payload.lock().unwrap().clone())
        }
    }

    fn ready_state() -> AppState {
        let engine = Arc::new(FakeEngine::new());
        let mut application = Underprint::default();
        application.register(engine).unwrap();
        AppState::ready(
            application,
            CapabilitiesReport::new(true, None, default_runtime(), vec![descriptor()]),
            None,
            1,
            100,
        )
    }

    fn png() -> Vec<u8> {
        serialize_png(&DynamicImage::new_rgb8(320, 180), &ImagePolicy::default()).unwrap()
    }

    fn multipart(parts: &[(&str, &[u8])]) -> (String, Vec<u8>) {
        let boundary = "underprint-test-boundary";
        let mut body = Vec::new();
        for (name, value) in parts {
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
            );
            body.extend_from_slice(value);
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        (format!("multipart/form-data; boundary={boundary}"), body)
    }

    #[tokio::test]
    async fn liveness_is_independent_from_model_readiness() {
        let app = router(state(), Duration::from_secs(1));
        let live = app
            .clone()
            .oneshot(Request::get("/health/live").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(live.status(), StatusCode::OK);
        let ready = app
            .oneshot(Request::get("/health/ready").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(ready.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn openapi_is_published_by_the_service() {
        let response = router(state(), Duration::from_secs(1))
            .oneshot(Request::get("/openapi.yaml").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(OPENAPI.contains("openapi: 3.1.0"));
        for path in [
            "/health/live",
            "/health/ready",
            "/v1/algorithms",
            "/v1/embeddings",
            "/v1/detections",
            "/v1/verifications",
        ] {
            assert!(OPENAPI.contains(path));
        }
    }

    #[tokio::test]
    async fn schematio_compatibility_contract_round_trips() {
        let token = "0".repeat(61);
        let image = png();
        let (content_type, body) =
            multipart(&[("image", image.as_slice()), ("token", token.as_bytes())]);
        let app = router(ready_state(), Duration::from_secs(2));
        let response = app
            .clone()
            .oneshot(
                Request::post("/embed")
                    .header(header::CONTENT_TYPE, content_type)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["x-watermark-token"], token);
        assert_eq!(response.headers()["x-watermark-model"], SCHEMATIO_MODEL);
        assert!(response.headers().contains_key("x-underprint-report"));
        let protected = to_bytes(response.into_body(), 64 * 1024 * 1024)
            .await
            .unwrap();

        let (content_type, body) = multipart(&[("image", protected.as_ref())]);
        let response = app
            .oneshot(
                Request::post("/decode")
                    .header(header::CONTENT_TYPE, content_type)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        let decoded: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(decoded["present"], true);
        assert_eq!(decoded["token"], token);
        assert_eq!(decoded["model"], SCHEMATIO_MODEL);
    }

    #[test]
    fn token_comparison_does_not_accept_prefixes() {
        assert!(constant_time_equal(b"secret", b"secret"));
        assert!(!constant_time_equal(b"secret", b"secret2"));
        assert!(!constant_time_equal(b"secre", b"secret"));
    }

    #[test]
    fn admission_rejects_work_instead_of_queueing() {
        let mut state = state();
        state.rate = Arc::new(Mutex::new(TokenBucket::new(100)));
        let first = admit(&state).unwrap();
        let second = admit(&state).unwrap_err();
        assert_eq!(second.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            state.metrics.concurrency_rejected.load(Ordering::Relaxed),
            1
        );
        drop(first);
        assert!(admit(&state).is_ok());
    }
}
