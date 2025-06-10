use axum::body::Body;
use hyper::{Response, StatusCode};

pub type Result<T> = std::result::Result<T, LibreOfficeError>;

#[derive(Debug, thiserror::Error)]
pub enum LibreOfficeError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Conversion timeout")]
    Timeout,
    #[error("Conversion failed: {0}")]
    ConversionFailed(String),
    #[error("Output file not found after conversion")]
    OutputNotFound,
    #[error("Corrupted or invalid input file: {0}")]
    CorruptedInput(String),
    #[error("Unsupported format conversion from {from} to {to}")]
    UnsupportedConversion { from: String, to: String },
    #[error("File is password protected")]
    PasswordProtected,
    #[error("Input file is empty or invalid")]
    EmptyOrInvalidInput,
}

impl From<LibreOfficeError> for Response<Body> {
    fn from(error: LibreOfficeError) -> Self {
        let (status, message) = match error {
            LibreOfficeError::Timeout => (
                StatusCode::REQUEST_TIMEOUT,
                "Conversion timed out".to_string(),
            ),
            LibreOfficeError::CorruptedInput(_) => (
                StatusCode::BAD_REQUEST,
                format!("Invalid or corrupted input file: {}", error),
            ),
            LibreOfficeError::UnsupportedConversion { from, to } => (
                StatusCode::BAD_REQUEST,
                format!("Unsupported conversion from {} to {}", from, to),
            ),
            LibreOfficeError::PasswordProtected => (
                StatusCode::BAD_REQUEST,
                "File is password protected".to_string(),
            ),
            LibreOfficeError::EmptyOrInvalidInput => (
                StatusCode::BAD_REQUEST,
                "Input file is empty or invalid".to_string(),
            ),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Conversion failed: {}", error),
            ),
        };

        create_error_response(status, &message)
    }
}

// Helper function to create error responses safely
pub fn create_error_response(status: StatusCode, message: &str) -> Response<Body> {
    Response::builder()
        .status(status)
        .body(Body::from(message.to_string()))
        .unwrap_or_else(|e| {
            tracing::error!("Failed to build error response: {}", e);
            Response::new(Body::from("Internal server error"))
        })
}
