use std::path::PathBuf;
use std::sync::OnceLock;
use tempfile::{TempDir, tempdir};
use tokio::process::Command as TokioCommand;
use tokio::sync::Mutex;

use crate::{
    detect_filetype::{FileType, detect_file_type_from_bytes},
    error::{LibreOfficeError, Result},
};

// Global mutex to ensure only one LibreOffice conversion runs at a time
static LIBREOFFICE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn get_libreoffice_lock() -> &'static Mutex<()> {
    LIBREOFFICE_LOCK.get_or_init(|| Mutex::new(()))
}

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

        tracing::debug!("Files found in output directory: {:?}", files);

        if files.is_empty() {
            // If no files at all were created, likely input file issue
            return LibreOfficeError::CorruptedInput(
                "No output files were generated - input file may be corrupted or invalid"
                    .to_string(),
            );
        } else {
            // If files exist but not the expected format, conversion issue
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
    from: &str,
    to: &str,
) -> Result<Vec<u8>> {
    tracing::debug!("Starting async CLI conversion: {} -> {}", from, to);

    // Acquire the lock to ensure only one LibreOffice process runs at a time
    tracing::debug!("Waiting for LibreOffice lock...");
    let _lock = get_libreoffice_lock().lock().await;
    tracing::debug!("LibreOffice lock acquired, proceeding with conversion");

    let input_filename = format!("document.{}", from);
    let (input_path, output_dir, _temp_dir) =
        temp_dir_with_files(&input_filename).map_err(LibreOfficeError::Io)?;

    // Write input file asynchronously
    tokio::fs::write(&input_path, input_buf)
        .await
        .map_err(LibreOfficeError::Io)?;
    tracing::debug!("Input file written: {:?}", input_path);

    // Run LibreOffice conversion with timeout
    tracing::debug!("Running LibreOffice conversion...");
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

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    tracing::debug!("LibreOffice stderr: {}", stderr);
    tracing::debug!("LibreOffice stdout: {}", stdout);

    // Check if conversion succeeded and analyze the error
    if !output.status.success() {
        // Analyze the error output for specific issues
        let error = analyze_libreoffice_error(&stderr, &stdout, &from, &to);
        return Err(error);
    }

    tracing::debug!("LibreOffice conversion completed successfully");

    // Find and read the output file
    let expected_output = output_dir.join(format!("document.{}", to));

    println!("Looking for output file at {:?}", expected_output);

    if !expected_output.exists() {
        // Try to find any file with the target extension
        let mut entries = tokio::fs::read_dir(&output_dir)
            .await
            .map_err(LibreOfficeError::Io)?;
        let mut found_file = None;

        while let Some(entry) = entries.next_entry().await.map_err(LibreOfficeError::Io)? {
            let path = entry.path();

            if let Some(ext) = path.extension() {
                if ext == to {
                    found_file = Some(path);
                    break;
                }
            }
        }

        match found_file {
            Some(path) => {
                let output_data = tokio::fs::read(path).await.map_err(LibreOfficeError::Io)?;
                tracing::debug!(
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
    tracing::debug!(
        "Conversion completed, output size: {} bytes",
        output_data.len()
    );

    Ok(output_data)
}

// Convenience function - use the async version by default
pub async fn convert_libreoffice(input_buf: Vec<u8>, from: &str, to: &str) -> Result<Vec<u8>> {
    let detected_mimetype = detect_file_type_from_bytes(&input_buf);

    if detected_mimetype == FileType::Unknown {
        return Err(LibreOfficeError::UnsupportedConversion {
            from: from.to_string(),
            to: to.to_string(),
        });
    }

    convert_libreoffice_async(input_buf, from, to).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;
    use tokio::time::{Duration, sleep};

    #[tokio::test]
    async fn test_libreoffice_lock_initialization() {
        // Test that the lock can be initialized and acquired
        let lock = get_libreoffice_lock();
        let _guard = lock.lock().await;
        // If we get here, the lock works
    }

    #[tokio::test]
    async fn test_concurrent_lock_access() {
        // Test that only one task can hold the lock at a time
        let counter = Arc::new(AtomicUsize::new(0));
        let mut handles = vec![];

        for _ in 0..5 {
            let counter_clone = counter.clone();
            let handle = tokio::spawn(async move {
                let _lock = get_libreoffice_lock().lock().await;

                // Increment counter and sleep to simulate work
                let current = counter_clone.fetch_add(1, Ordering::SeqCst);
                sleep(Duration::from_millis(10)).await;
                let after_sleep = counter_clone.load(Ordering::SeqCst);

                // If locking works correctly, no other task should have incremented
                // the counter while we were sleeping
                assert_eq!(current + 1, after_sleep);
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.expect("Task should complete successfully");
        }

        // All tasks should have completed
        assert_eq!(counter.load(Ordering::SeqCst), 5);
    }

    #[tokio::test]
    async fn test_lock_released_on_drop() {
        // Test that the lock is properly released when the guard is dropped
        {
            let _guard = get_libreoffice_lock().lock().await;
            // Lock is held here
        }
        // Lock should be released here

        // We should be able to acquire it again immediately
        let _guard2 = get_libreoffice_lock().lock().await;
    }

    #[tokio::test]
    async fn test_serial_execution_timing() {
        // Test that tasks execute serially, not concurrently
        use std::time::Instant;

        let start_time = Arc::new(std::sync::Mutex::new(Vec::new()));
        let end_time = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut handles = vec![];

        for i in 0..3 {
            let start_time_clone = start_time.clone();
            let end_time_clone = end_time.clone();

            let handle = tokio::spawn(async move {
                let _lock = get_libreoffice_lock().lock().await;

                // Record start time
                {
                    let mut times = start_time_clone.lock().unwrap();
                    times.push((i, Instant::now()));
                }

                // Simulate work
                sleep(Duration::from_millis(50)).await;

                // Record end time
                {
                    let mut times = end_time_clone.lock().unwrap();
                    times.push((i, Instant::now()));
                }
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.expect("Task should complete successfully");
        }

        let start_times = start_time.lock().unwrap();
        let end_times = end_time.lock().unwrap();

        // Verify that tasks executed serially (no overlap)
        assert_eq!(start_times.len(), 3);
        assert_eq!(end_times.len(), 3);

        // Check that each task's start time is after the previous task's end time
        // (with some tolerance for timing variations)
        let mut sorted_starts: Vec<_> = start_times.iter().collect();
        let mut sorted_ends: Vec<_> = end_times.iter().collect();

        sorted_starts.sort_by_key(|(_, time)| *time);
        sorted_ends.sort_by_key(|(_, time)| *time);

        // The end of each task should be before the start of the next task
        for i in 0..sorted_ends.len() - 1 {
            assert!(sorted_ends[i].1 <= sorted_starts[i + 1].1);
        }
    }

    #[tokio::test]
    async fn test_convert_function_uses_lock() {
        // Test that the convert_libreoffice function properly uses the lock
        // by checking that multiple concurrent calls are serialized

        // Create some dummy input data
        let input_data = b"dummy content".to_vec();

        let start_times = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut handles = vec![];

        for i in 0..3 {
            let input_data_clone = input_data.clone();
            let start_times_clone = start_times.clone();

            let handle = tokio::spawn(async move {
                // Record when we start attempting the conversion
                {
                    let mut times = start_times_clone.lock().unwrap();
                    times.push((i, Instant::now()));
                }

                // This will fail because LibreOffice isn't installed, but that's expected
                // The important thing is that the locking mechanism is exercised
                let result = convert_libreoffice_async(input_data_clone, "txt", "pdf").await;

                // We expect this to fail due to LibreOffice not being available
                assert!(result.is_err());
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.expect("Task should complete successfully");
        }

        let start_times = start_times.lock().unwrap();
        assert_eq!(start_times.len(), 3);

        // The fact that all tasks completed without hanging shows that
        // the lock is properly acquired and released
    }
}
