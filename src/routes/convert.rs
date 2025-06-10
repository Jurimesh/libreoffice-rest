use axum::{body::Body, extract::Multipart, http::StatusCode, response::Response};
use hyper::header;

use crate::{error::create_error_response, libreoffice};

#[axum::debug_handler]
pub async fn handler(mut multipart: Multipart) -> Response {
    // Extract multipart data with proper error handling
    let (file_bytes, input_format, output_format) =
        match extract_multipart_data(&mut multipart).await {
            Ok(data) => data,
            Err(response) => return response,
        };

    handle_conversion(file_bytes, input_format, output_format).await
}

async fn extract_multipart_data(
    multipart: &mut Multipart,
) -> Result<(Vec<u8>, String, String), Response<Body>> {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut input_filename: Option<String> = None;
    let mut output_format: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("");

        match name {
            "file" => {
                input_filename = Some(field.file_name().unwrap_or("unknown_file").to_string());

                file_bytes = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| {
                            tracing::debug!("Error reading file field: {:?}", e);
                            create_error_response(
                                StatusCode::BAD_REQUEST,
                                "Error reading uploaded file",
                            )
                        })?
                        .to_vec(),
                )
            }
            "output_format" => {
                output_format = Some(field.text().await.map_err(|e| {
                    tracing::debug!("Error reading output_format field: {}", e);
                    create_error_response(StatusCode::BAD_REQUEST, "Error reading output_format")
                })?)
            }
            _ => {
                // Skip unknown fields
            }
        }
    }

    match (file_bytes, input_filename, output_format) {
        (Some(bytes), Some(input_filename), Some(output_format)) => {
            Ok((bytes, input_filename, output_format))
        }
        _ => Err(create_error_response(
            StatusCode::BAD_REQUEST,
            "Missing required fields: file, output_format",
        )),
    }
}

async fn handle_conversion(
    bytes: Vec<u8>,
    input_filename: String,
    output_format: String,
) -> Response<Body> {
    tracing::debug!(
        "Starting conversion request: {} -> {}",
        input_filename,
        output_format
    );

    // Get file extension from input filename
    let input_format = match input_filename.rsplit('.').next() {
        Some(ext) => ext.to_lowercase(),
        None => String::from(""),
    };

    match libreoffice::convert_libreoffice(bytes, &input_format, &output_format).await {
        Ok(converted_bytes) => {
            tracing::debug!("Conversion completed successfully");
            create_success_response(converted_bytes, &output_format)
        }
        Err(e) => {
            tracing::error!("Conversion failed: {}", e);
            e.into()
        }
    }
}

fn create_success_response(converted_bytes: Vec<u8>, output_format: &str) -> Response<Body> {
    let filename = format!("converted.{}", output_format);
    let content_type = mime_guess::from_ext(output_format)
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
            tracing::error!("Error building success response: {}", e);
            create_error_response(StatusCode::INTERNAL_SERVER_ERROR, "Error building response")
        }
    }
}
