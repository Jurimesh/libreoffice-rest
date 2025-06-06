use std::path::PathBuf;

use crate::libreofficekit::{DocUrl, Office, OfficeError};
use tempfile::{TempDir, tempdir};

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
    println!("Starting conversion: {} -> {}", from, to);

    let (input_path, _temp_dir1) = temp_file(&format!("input.{}", from));
    let (output_path, _temp_dir2) = temp_file(&format!("output.{}", to));

    // Async file write
    tokio::fs::write(&input_path, input_buf)
        .await
        .map_err(LibreOfficeError::Io)?;
    println!("Input file written: {:?}", input_path);

    // Synchronous LibreOffice operations (in spawn_blocking)
    let output_path_clone = output_path.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let office = Office::new(Office::find_install_path().unwrap())?;
        let input_url = DocUrl::from_path(&input_path)?;
        let output_url = DocUrl::from_path(&output_path_clone)?;

        let mut document = office.document_load(&input_url)?;
        document.save_as(&output_url, &to, None)?;

        Ok(())
    })
    .await
    .map_err(|e| LibreOfficeError::ConversionFailed(e.to_string()))??;

    // Async file read
    let output_data = tokio::fs::read(output_path)
        .await
        .map_err(LibreOfficeError::Io)?;
    println!(
        "Conversion completed, output size: {} bytes",
        output_data.len()
    );

    Ok(output_data)
}
