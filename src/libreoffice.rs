use std::{path::PathBuf, sync::Mutex};

use crate::libreofficekit::{DocUrl, Office, OfficeError};
use tempfile::{TempDir, tempdir};
use tokio::task;

pub static OFFICE_TEST_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, thiserror::Error)]
pub enum LibreOfficeError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("LibreOffice SDK error: {0}")]
    Office(#[from] OfficeError),
    #[error("Conversion timeout")]
    Timeout,
    #[error("Invalid file format: {0}")]
    InvalidFormat(String),
    #[error("LibreOffice not found")]
    NotFound,
    #[error("Conversion failed: {0}")]
    ConversionFailed(String),
}

pub type Result<T> = std::result::Result<T, LibreOfficeError>;

fn temp_file(name: &str) -> (PathBuf, TempDir) {
    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join(name);

    (output_path, temp_dir)
}

pub async fn convert_libreoffice(input_buf: Vec<u8>, from: String, to: String) -> Result<Vec<u8>> {
    // Move the entire conversion to a blocking task to avoid Send issues
    task::spawn_blocking(move || {
        let _lock = OFFICE_TEST_LOCK.lock();

        let office = Office::new(Office::find_install_path().unwrap())?;

        let (input_path, _temp_dir) = temp_file(&format!("input.{}", from));
        std::fs::write(&input_path, input_buf).map_err(LibreOfficeError::Io)?;
        let input_url = DocUrl::from_path(&input_path)?;

        let (output_path, _temp_dir) = temp_file(&format!("output.{}", to));
        let output_url = DocUrl::from_path(&output_path)?;

        let mut document = office.document_load(&input_url)?;
        let _doc = document.save_as(&output_url, &to, None)?;

        // Read output file and return as buffer
        let output_data = std::fs::read(output_path).map_err(LibreOfficeError::Io)?;
        Ok(output_data)
    })
    .await
    .map_err(|_| LibreOfficeError::ConversionFailed("Task panicked".to_string()))?
}
