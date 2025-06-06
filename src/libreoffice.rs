use std::path::PathBuf;
use tempfile::{TempDir, tempdir};
use tokio::process::Command as TokioCommand;

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

pub type Result<T> = std::result::Result<T, LibreOfficeError>;

fn temp_dir_with_files(input_name: &str) -> std::io::Result<(PathBuf, PathBuf, TempDir)> {
    let temp_dir = tempdir()?;
    let input_path = temp_dir.path().join(input_name);
    let output_dir = temp_dir.path().to_path_buf();

    Ok((input_path, output_dir, temp_dir))
}

/// Analyzes LibreOffice error output to provide more specific error messages
fn analyze_libreoffice_error(stderr: &str, stdout: &str, from: &str, to: &str) -> LibreOfficeError {
    let combined_output = format!("{} {}", stderr, stdout).to_lowercase();

    // Check for specific error patterns
    if combined_output.contains("password") || combined_output.contains("encrypted") {
        return LibreOfficeError::PasswordProtected;
    }

    if combined_output.contains("format") && combined_output.contains("not supported") {
        return LibreOfficeError::UnsupportedConversion {
            from: from.to_string(),
            to: to.to_string(),
        };
    }

    if combined_output.contains("corrupt")
        || combined_output.contains("damaged")
        || combined_output.contains("invalid")
        || combined_output.contains("parse error")
        || combined_output.contains("bad file")
    {
        return LibreOfficeError::CorruptedInput(format!(
            "File appears to be corrupted or in an invalid format"
        ));
    }

    if combined_output.contains("empty")
        || combined_output.contains("no content")
        || combined_output.contains("zero bytes")
    {
        return LibreOfficeError::EmptyOrInvalidInput;
    }

    if combined_output.contains("filter") && combined_output.contains("not found") {
        return LibreOfficeError::UnsupportedConversion {
            from: from.to_string(),
            to: to.to_string(),
        };
    }

    // Default to generic conversion failed with full output
    LibreOfficeError::ConversionFailed(format!(
        "LibreOffice conversion failed. stderr: {}, stdout: {}",
        stderr, stdout
    ))
}

/// Analyzes why the output file is missing to provide more specific error messages
fn analyze_missing_output_error(output_dir: &PathBuf, from: &str, to: &str) -> LibreOfficeError {
    // Check what files actually exist in the output directory
    if let Ok(entries) = std::fs::read_dir(output_dir) {
        let files: Vec<String> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect();

        println!("Files found in output directory: {:?}", files);

        // If no files at all were created, likely input file issue
        if files.is_empty() {
            return LibreOfficeError::CorruptedInput(
                "No output files were generated - input file may be corrupted or invalid"
                    .to_string(),
            );
        }

        // If files exist but not the expected format, conversion issue
        if !files.is_empty() {
            return LibreOfficeError::UnsupportedConversion {
                from: from.to_string(),
                to: to.to_string(),
            };
        }
    }

    // Fallback to generic error
    LibreOfficeError::OutputNotFound
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

    // Check if conversion succeeded and analyze the error
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("LibreOffice stderr: {}", stderr);
        println!("LibreOffice stdout: {}", stdout);

        // Analyze the error output for specific issues
        let error = analyze_libreoffice_error(&stderr, &stdout, &from, &to);
        return Err(error);
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
                // No output file found - this could indicate various issues
                return Err(analyze_missing_output_error(&output_dir, &from, &to));
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
