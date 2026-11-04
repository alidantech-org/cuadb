//! Error code and HTML regression tests, independent of database availability.

use axum::{
    body::to_bytes,
    http::{HeaderValue, StatusCode},
    response::IntoResponse,
};
use cuadb::error::{AppError, ErrorKind};

#[tokio::test]
async fn error_responses_use_safe_codes_and_html_messages() {
    for (kind, status, code) in [
        (ErrorKind::InvalidInput, StatusCode::BAD_REQUEST, "invalid_input"),
        (ErrorKind::Forbidden, StatusCode::FORBIDDEN, "access_denied"),
        (
            ErrorKind::DatabaseUnavailable,
            StatusCode::SERVICE_UNAVAILABLE,
            "database_unavailable",
        ),
        (ErrorKind::TimedOut, StatusCode::GATEWAY_TIMEOUT, "request_timeout"),
        (ErrorKind::Overloaded, StatusCode::SERVICE_UNAVAILABLE, "server_busy"),
        (ErrorKind::PayloadTooLarge, StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large"),
        (ErrorKind::TemplateFailure, StatusCode::INTERNAL_SERVER_ERROR, "template_error"),
    ] {
        let response = AppError::new(kind).into_response();
        assert_eq!(response.status(), status);
        assert_eq!(
            response.headers().get("x-cuadb-error"),
            Some(&HeaderValue::from_static(code))
        );
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        let rendered = String::from_utf8(body.to_vec()).unwrap();
        assert!(rendered.contains("role=\"alert\""));
        assert!(rendered.contains(kind.message()));
        assert!(!rendered.contains("postgresql://"));
        assert!(!rendered.contains("sqlx"));
    }
}

#[tokio::test]
async fn template_failure_does_not_expose_rendering_details() {
    let response = AppError::template().into_response();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let text = to_bytes(response.into_body(), 4096).await.unwrap();
    assert!(std::str::from_utf8(&text).unwrap().contains("could not complete"));
}
