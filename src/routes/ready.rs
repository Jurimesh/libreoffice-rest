use axum::{http::StatusCode, response::IntoResponse};
use tokio::process::Command;

pub async fn handler() -> impl IntoResponse {
    match tokio::time::timeout(
        std::time::Duration::from_secs(3),
        Command::new("libreoffice").arg("--version").output(),
    )
    .await
    {
        Ok(Ok(output)) if output.status.success() => (StatusCode::OK, "READY"),
        Ok(Ok(output)) => {
            tracing::warn!(
                "LibreOffice version check failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            (StatusCode::SERVICE_UNAVAILABLE, "NOT READY")
        }
        Ok(Err(e)) => {
            tracing::warn!("LibreOffice not accessible: {}", e);
            (StatusCode::SERVICE_UNAVAILABLE, "NOT READY")
        }
        Err(_) => {
            tracing::warn!("LibreOffice version check timed out");
            (StatusCode::SERVICE_UNAVAILABLE, "NOT READY")
        }
    }
}
