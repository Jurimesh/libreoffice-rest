use axum::{body::Body, extract::Multipart, http::StatusCode, response::Response};
use hyper::header;

use crate::libreoffice;

#[axum::debug_handler]
pub async fn handler(mut multipart: Multipart) -> Response {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut input_format: Option<String> = None;
    let mut output_format: Option<String> = None;

    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap_or("");

        match name {
            "file" => {
                let data = field.bytes().await.unwrap();
                file_bytes = Some(data.to_vec());
            }
            "input_format" => {
                let text = field.text().await.unwrap();
                input_format = Some(text);
            }
            "output_format" => {
                let text = field.text().await.unwrap();
                output_format = Some(text);
            }
            _ => {}
        }
    }

    match (file_bytes, input_format, output_format) {
        (Some(bytes), Some(input), Some(output)) => {
            let converted_bytes =
                libreoffice::convert_libreoffice(bytes, input, output.clone()).unwrap();

            let filename = format!("converted.{}", &output);
            let content_type = mime_guess::from_ext(output.as_str())
                .first_or_octet_stream()
                .to_string();

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type)
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", filename),
                )
                .body(Body::from(converted_bytes))
                .unwrap()
        }
        _ => Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from(
                "Missing required fields: file, input_format, output_format",
            ))
            .unwrap(),
    }
}
