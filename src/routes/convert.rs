use axum::{body::Body, extract::Multipart, http::StatusCode, response::Response};
use hyper::header;

use crate::libreoffice;

#[axum::debug_handler]
pub async fn handler(mut multipart: Multipart) -> Response {
    // Extract multipart data with proper error handling
    let (file_bytes, input_format, output_format) =
        match extract_multipart_data(&mut multipart).await {
            Ok(data) => data,
            Err(response) => return response,
        };

    match (file_bytes, input_format, output_format) {
        (Some(bytes), Some(input), Some(output)) => handle_conversion(bytes, input, output).await,
        _ => create_error_response(
            StatusCode::BAD_REQUEST,
            "Missing required fields: file, input_format, output_format",
        ),
    }
}

async fn extract_multipart_data(
    multipart: &mut Multipart,
) -> Result<(Option<Vec<u8>>, Option<String>, Option<String>), Response<Body>> {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut input_format: Option<String> = None;
    let mut output_format: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("");

        match name {
            "file" => match field.bytes().await {
                Ok(data) => file_bytes = Some(data.to_vec()),
                Err(e) => {
                    println!("Error reading file field: {}", e);
                    return Err(create_error_response(
                        StatusCode::BAD_REQUEST,
                        "Error reading uploaded file",
                    ));
                }
            },
            "input_format" => match field.text().await {
                Ok(text) => input_format = Some(text),
                Err(e) => {
                    println!("Error reading input_format field: {}", e);
                    return Err(create_error_response(
                        StatusCode::BAD_REQUEST,
                        "Error reading input_format",
                    ));
                }
            },
            "output_format" => match field.text().await {
                Ok(text) => output_format = Some(text),
                Err(e) => {
                    println!("Error reading output_format field: {}", e);
                    return Err(create_error_response(
                        StatusCode::BAD_REQUEST,
                        "Error reading output_format",
                    ));
                }
            },
            _ => {
                // Skip unknown fields
            }
        }
    }

    Ok((file_bytes, input_format, output_format))
}

async fn handle_conversion(bytes: Vec<u8>, input: String, output: String) -> Response<Body> {
    tracing::info!("Starting conversion request: {} -> {}", input, output);

    match libreoffice::convert_libreoffice(bytes, input, output.clone()).await {
        Ok(converted_bytes) => {
            tracing::info!("Conversion completed successfully");
            create_success_response(converted_bytes, output)
        }
        Err(e) => {
            tracing::error!("Conversion failed: {}", e);
            create_conversion_error_response(e)
        }
    }
}

fn create_success_response(converted_bytes: Vec<u8>, output_format: String) -> Response<Body> {
    let filename = format!("converted.{}", output_format);
    let content_type = mime_guess::from_ext(output_format.as_str())
        .first_or_octet_stream()
        .to_string();

    match Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(Body::from(converted_bytes))
    {
        Ok(response) => response,
        Err(e) => {
            println!("Error building success response: {}", e);
            create_error_response(StatusCode::INTERNAL_SERVER_ERROR, "Error building response")
        }
    }
}

fn create_conversion_error_response(e: libreoffice::LibreOfficeError) -> Response<Body> {
    let (status, message) = match e {
        libreoffice::LibreOfficeError::Timeout => (
            StatusCode::REQUEST_TIMEOUT,
            "Conversion timed out".to_string(),
        ),
        libreoffice::LibreOfficeError::CorruptedInput(_) => (
            StatusCode::BAD_REQUEST,
            format!("Invalid or corrupted input file: {}", e),
        ),
        libreoffice::LibreOfficeError::UnsupportedConversion { from, to } => (
            StatusCode::BAD_REQUEST,
            format!("Unsupported conversion from {} to {}", from, to),
        ),
        libreoffice::LibreOfficeError::PasswordProtected => (
            StatusCode::BAD_REQUEST,
            "File is password protected".to_string(),
        ),
        libreoffice::LibreOfficeError::EmptyOrInvalidInput => (
            StatusCode::BAD_REQUEST,
            "Input file is empty or invalid".to_string(),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Conversion failed: {}", e),
        ),
    };

    create_error_response(status, &message)
}

// Helper function to create error responses safely
fn create_error_response(status: StatusCode, message: &str) -> Response<Body> {
    Response::builder()
        .status(status)
        .body(Body::from(message.to_string()))
        .unwrap_or_else(|e| {
            println!("Failed to build error response: {}", e);
            Response::new(Body::from("Internal server error"))
        })
}
