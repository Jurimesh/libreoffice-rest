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
