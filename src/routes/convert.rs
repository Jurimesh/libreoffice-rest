use axum::{body::Body, extract::Multipart, http::StatusCode, response::Response};
use hyper::header;
use tracing::Instrument;

use crate::{error::create_error_response, libreoffice};

const MAX_OUTPUT_FORMAT_LEN: usize = 20;

#[axum::debug_handler]
pub async fn handler(mut multipart: Multipart) -> Response {
    let request_id = uuid::Uuid::new_v4();

    async move {
        let (file_bytes, input_format, output_format) =
            match extract_multipart_data(&mut multipart).await {
                Ok(data) => data,
                Err(response) => return response,
            };

        handle_conversion(file_bytes, input_format, output_format).await
    }
    .instrument(tracing::info_span!("convert", %request_id))
    .await
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
                if file_bytes.is_some() {
                    return Err(create_error_response(
                        StatusCode::BAD_REQUEST,
                        "Duplicate 'file' field",
                    ));
                }

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
                if output_format.is_some() {
                    return Err(create_error_response(
                        StatusCode::BAD_REQUEST,
                        "Duplicate 'output_format' field",
                    ));
                }

                let bytes = field.bytes().await.map_err(|e| {
                    tracing::debug!("Error reading output_format field: {}", e);
                    create_error_response(StatusCode::BAD_REQUEST, "Error reading output_format")
                })?;

                if bytes.len() > MAX_OUTPUT_FORMAT_LEN {
                    return Err(create_error_response(
                        StatusCode::BAD_REQUEST,
                        "output_format field too large",
                    ));
                }

                let text = String::from_utf8(bytes.to_vec()).map_err(|_| {
                    create_error_response(
                        StatusCode::BAD_REQUEST,
                        "output_format must be valid UTF-8",
                    )
                })?;

                if text.is_empty() {
                    return Err(create_error_response(
                        StatusCode::BAD_REQUEST,
                        "output_format must not be empty",
                    ));
                }

                // Allow LibreOffice filter specs like "pdf:writer_pdf_Export".
                // Only ASCII alphanumerics, ':', and '_' are permitted.
                if !text
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == ':' || c == '_')
                {
                    return Err(create_error_response(
                        StatusCode::BAD_REQUEST,
                        "output_format may only contain ASCII letters, digits, ':', and '_'",
                    ));
                }

                // The extension portion (before the first ':') must be
                // non-empty and strictly alphanumeric.
                let extension = text.split(':').next().unwrap_or("");
                if extension.is_empty()
                    || !extension.chars().all(|c| c.is_ascii_alphanumeric())
                {
                    return Err(create_error_response(
                        StatusCode::BAD_REQUEST,
                        "output_format extension (before ':') must be non-empty and alphanumeric",
                    ));
                }

                output_format = Some(text);
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
    tracing::info!(
        input_filename = %input_filename,
        output_format = %output_format,
        input_size_bytes = bytes.len(),
        "Starting conversion request",
    );

    // Get file extension from input filename
    let input_format = match input_filename.rsplit('.').next() {
        Some(ext) => ext.to_lowercase(),
        None => String::from(""),
    };

    match libreoffice::convert_libreoffice(bytes, &input_format, &output_format).await {
        Ok(converted_bytes) => {
            tracing::info!(
                output_size_bytes = converted_bytes.len(),
                "Conversion completed successfully",
            );
            create_success_response(converted_bytes, &output_format)
        }
        Err(e) => {
            tracing::error!("Conversion failed: {}", e);
            e.into()
        }
    }
}

fn create_success_response(converted_bytes: Vec<u8>, output_format: &str) -> Response<Body> {
    // Extract the extension portion (before any ':' filter spec) for the
    // filename and content-type. The extension is validated as alphanumeric.
    let ext = output_format.split(':').next().unwrap_or(output_format);
    let filename = format!("converted.{}", ext);
    let content_type = mime_guess::from_ext(ext)
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
