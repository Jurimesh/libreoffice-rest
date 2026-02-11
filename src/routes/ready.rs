use axum::{http::StatusCode, response::IntoResponse};
use tokio::process::Command;

pub async fn handler() -> impl IntoResponse {
    match Command::new("libreoffice").arg("--version").output().await {
        Ok(output) if output.status.success() => (StatusCode::OK, "READY"),
        Ok(output) => {
            tracing::warn!(
                "LibreOffice version check failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            (StatusCode::SERVICE_UNAVAILABLE, "NOT READY")
        }
        Err(e) => {
            tracing::warn!("LibreOffice not accessible: {}", e);
            (StatusCode::SERVICE_UNAVAILABLE, "NOT READY")
        }
    }
}
