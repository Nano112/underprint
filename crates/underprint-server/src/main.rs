use std::{
    env, io,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use underprint::{CapabilitiesReport, Underprint};
use underprint_server::{AppState, default_runtime, router};
use underprint_trustmark::{TrustmarkEngine, descriptor, verify_models};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if env::args().nth(1).as_deref() == Some("--healthcheck") {
        healthcheck(env::args().nth(2).as_deref().unwrap_or("127.0.0.1:8080"))?;
        return Ok(());
    }

    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "underprint_server=info,tower_http=info".into()),
        )
        .with_current_span(false)
        .with_span_list(false)
        .init();

    let models =
        PathBuf::from(env::var("UNDERPRINT_MODELS_DIR").unwrap_or_else(|_| "models".into()));
    verify_models(&models)?;
    let engine = Arc::new(TrustmarkEngine::load(&models)?);
    engine.initialize()?;
    let mut application = Underprint::default();
    application.register(engine)?;
    let capabilities = CapabilitiesReport::new(true, None, default_runtime(), vec![descriptor()]);
    let address: SocketAddr = env::var("UNDERPRINT_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8080".into())
        .parse()?;
    let auth_token = env::var("UNDERPRINT_API_TOKEN")
        .ok()
        .filter(|value| !value.is_empty());
    validate_bind_auth(address, auth_token.as_deref())?;
    let state = AppState::ready(
        application,
        capabilities,
        auth_token,
        env_usize("UNDERPRINT_MAX_CONCURRENCY", 2),
        env_u32("UNDERPRINT_REQUESTS_PER_SECOND", 10),
    );
    let timeout = Duration::from_secs(env_u64("UNDERPRINT_REQUEST_TIMEOUT_SECONDS", 30));
    let listener = tokio::net::TcpListener::bind(address).await?;
    tracing::info!(%address, "underprint service ready");
    axum::serve(listener, router(state, timeout))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

fn healthcheck(address: &str) -> io::Result<()> {
    let mut stream = TcpStream::connect_timeout(
        &address.parse().map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid address: {error}"),
            )
        })?,
        Duration::from_secs(3),
    )?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    stream.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;
    let mut response = [0_u8; 256];
    let read = stream.read(&mut response)?;
    if response[..read].starts_with(b"HTTP/1.1 200") {
        Ok(())
    } else {
        Err(io::Error::other("Underprint health endpoint is not ready"))
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutdown requested");
}

fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn env_u32(name: &str, default: u32) -> u32 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn env_usize(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn validate_bind_auth(address: SocketAddr, auth_token: Option<&str>) -> io::Result<()> {
    if !address.ip().is_loopback() && auth_token.is_none_or(str::is_empty) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "UNDERPRINT_API_TOKEN is required for a non-loopback bind",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_bind_requires_authentication() {
        assert!(validate_bind_auth("127.0.0.1:8080".parse().unwrap(), None).is_ok());
        assert!(validate_bind_auth("0.0.0.0:8080".parse().unwrap(), None).is_err());
        assert!(validate_bind_auth("0.0.0.0:8080".parse().unwrap(), Some("token")).is_ok());
    }
}
