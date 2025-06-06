use std::path::PathBuf;
use std::process::Command;
use tempfile::{TempDir, tempdir};
use tokio::process::Command as TokioCommand;

#[derive(Debug, thiserror::Error)]
pub enum LibreOfficeError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Conversion timeout")]
    Timeout,
    #[error("Invalid file format: {0}")]
    InvalidFormat(String),
    #[error("LibreOffice not found")]
    NotFound,
    #[error("Conversion failed: {0}")]
    ConversionFailed(String),
    #[error("Output file not found after conversion")]
    OutputNotFound,
}

pub type Result<T> = std::result::Result<T, LibreOfficeError>;

fn temp_dir_with_files(input_name: &str) -> std::io::Result<(PathBuf, PathBuf, TempDir)> {
    let temp_dir = tempdir()?;
    let input_path = temp_dir.path().join(input_name);
    let output_dir = temp_dir.path().to_path_buf();

    Ok((input_path, output_dir, temp_dir))
}

/// Synchronous version using std::process::Command
pub fn convert_libreoffice_sync(input_buf: Vec<u8>, from: String, to: String) -> Result<Vec<u8>> {
    println!("Starting CLI conversion: {} -> {}", from, to);

    let input_filename = format!("document.{}", from);
    let (input_path, output_dir, _temp_dir) =
        temp_dir_with_files(&input_filename).map_err(LibreOfficeError::Io)?;

    // Write input file
    std::fs::write(&input_path, input_buf).map_err(LibreOfficeError::Io)?;
    println!("Input file written: {:?}", input_path);

    // Run LibreOffice conversion
    println!("Running LibreOffice conversion...");
    let output = Command::new("libreoffice")
        .args(&[
            "--headless",
            "--convert-to",
            &to,
            "--outdir",
            output_dir.to_str().unwrap(),
            input_path.to_str().unwrap(),
        ])
        .output()
        .map_err(LibreOfficeError::Io)?;

    // Check if conversion succeeded
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("LibreOffice stderr: {}", stderr);
        println!("LibreOffice stdout: {}", stdout);

        return Err(LibreOfficeError::ConversionFailed(format!(
            "LibreOffice exited with code {:?}. stderr: {}, stdout: {}",
            output.status.code(),
            stderr,
            stdout
        )));
    }

    println!("LibreOffice conversion completed successfully");

    // Find the output file (LibreOffice changes the extension)
    let expected_output = output_dir.join(format!("document.{}", to));

    if !expected_output.exists() {
        // Try to find any file with the target extension
        let entries = std::fs::read_dir(&output_dir).map_err(LibreOfficeError::Io)?;
        let mut found_file = None;

        for entry in entries {
            let entry = entry.map_err(LibreOfficeError::Io)?;
            let path = entry.path();

            if let Some(ext) = path.extension() {
                if ext == to.as_str() {
                    found_file = Some(path);
                    break;
                }
            }
        }

        match found_file {
            Some(path) => {
                let output_data = std::fs::read(path).map_err(LibreOfficeError::Io)?;
                println!(
                    "Conversion completed, output size: {} bytes",
                    output_data.len()
                );
                return Ok(output_data);
            }
            None => {
                return Err(LibreOfficeError::OutputNotFound);
            }
        }
    }

    // Read the converted file
    let output_data = std::fs::read(expected_output).map_err(LibreOfficeError::Io)?;
    println!(
        "Conversion completed, output size: {} bytes",
        output_data.len()
    );

    Ok(output_data)
}

/// Async version using tokio::process::Command with timeout
pub async fn convert_libreoffice_async(
    input_buf: Vec<u8>,
    from: String,
    to: String,
) -> Result<Vec<u8>> {
    println!("Starting async CLI conversion: {} -> {}", from, to);

    let input_filename = format!("document.{}", from);
    let (input_path, output_dir, _temp_dir) =
        temp_dir_with_files(&input_filename).map_err(LibreOfficeError::Io)?;

    // Write input file asynchronously
    tokio::fs::write(&input_path, input_buf)
        .await
        .map_err(LibreOfficeError::Io)?;
    println!("Input file written: {:?}", input_path);

    // Run LibreOffice conversion with timeout
    println!("Running LibreOffice conversion...");
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(60), // 60 second timeout
        TokioCommand::new("libreoffice")
            .args(&[
                "--headless",
                "--convert-to",
                &to,
                "--outdir",
                output_dir.to_str().unwrap(),
                input_path.to_str().unwrap(),
            ])
            .output(),
    )
    .await;

    let output = match output {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => return Err(LibreOfficeError::Io(e)),
        Err(_) => return Err(LibreOfficeError::Timeout),
    };

    // Check if conversion succeeded
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("LibreOffice stderr: {}", stderr);
        println!("LibreOffice stdout: {}", stdout);

        return Err(LibreOfficeError::ConversionFailed(format!(
            "LibreOffice exited with code {:?}. stderr: {}, stdout: {}",
            output.status.code(),
            stderr,
            stdout
        )));
    }

    println!("LibreOffice conversion completed successfully");

    // Find and read the output file
    let expected_output = output_dir.join(format!("document.{}", to));

    if !expected_output.exists() {
        // Try to find any file with the target extension
        let mut entries = tokio::fs::read_dir(&output_dir)
            .await
            .map_err(LibreOfficeError::Io)?;
        let mut found_file = None;

        while let Some(entry) = entries.next_entry().await.map_err(LibreOfficeError::Io)? {
            let path = entry.path();

            if let Some(ext) = path.extension() {
                if ext == to.as_str() {
                    found_file = Some(path);
                    break;
                }
            }
        }

        match found_file {
            Some(path) => {
                let output_data = tokio::fs::read(path).await.map_err(LibreOfficeError::Io)?;
                println!(
                    "Conversion completed, output size: {} bytes",
                    output_data.len()
                );
                return Ok(output_data);
            }
            None => {
                return Err(LibreOfficeError::OutputNotFound);
            }
        }
    }

    // Read the converted file
    let output_data = tokio::fs::read(expected_output)
        .await
        .map_err(LibreOfficeError::Io)?;
    println!(
        "Conversion completed, output size: {} bytes",
        output_data.len()
    );

    Ok(output_data)
}

// Convenience function - use the async version by default
pub async fn convert_libreoffice(input_buf: Vec<u8>, from: String, to: String) -> Result<Vec<u8>> {
    convert_libreoffice_async(input_buf, from, to).await
}
