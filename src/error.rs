//! Stable browser-safe error contract for HTTP handlers and database adapters.
//!
//! Internal driver/template errors must be classified, never interpolated into
//! HTML responses or request logs. Database adapters will map their own failures
//! to these categories when implemented.

use axum::{
    http::{HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};
use std::fmt;

/// Public error categories. No arbitrary strings or credentials are accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidInput,
    Forbidden,
    DatabaseUnavailable,
    TimedOut,
    Overloaded,
    PayloadTooLarge,
    TemplateFailure,
    Internal,
}

impl ErrorKind {
    pub const fn status(self) -> StatusCode {
        match self {
            Self::InvalidInput => StatusCode::BAD_REQUEST,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::DatabaseUnavailable | Self::Overloaded => StatusCode::SERVICE_UNAVAILABLE,
            Self::TimedOut => StatusCode::GATEWAY_TIMEOUT,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::TemplateFailure | Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Stable diagnostic code. Safe to expose to the browser and operator logs.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::Forbidden => "access_denied",
            Self::DatabaseUnavailable => "database_unavailable",
            Self::TimedOut => "request_timeout",
            Self::Overloaded => "server_busy",
            Self::PayloadTooLarge => "payload_too_large",
            Self::TemplateFailure => "template_error",
            Self::Internal => "internal_error",
        }
    }

    /// Static user-facing content, deliberately independent of underlying causes.
    pub const fn message(self) -> &'static str {
        match self {
            Self::InvalidInput => "The request is not valid.",
            Self::Forbidden => "You do not have access to this resource.",
            Self::DatabaseUnavailable => "The database is currently unavailable.",
            Self::TimedOut => "The request took too long. Please try again.",
            Self::Overloaded => "The server is busy. Please try again.",
            Self::PayloadTooLarge => "The request is too large.",
            Self::TemplateFailure | Self::Internal => "The server could not complete the request.",
        }
    }
}

/// Lightweight error used by routes, middleware and future database adapters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppError {
    kind: ErrorKind,
}

impl AppError {
    pub const fn new(kind: ErrorKind) -> Self {
        Self { kind }
    }

    pub const fn kind(self) -> ErrorKind {
        self.kind
    }

    pub const fn template() -> Self {
        Self::new(ErrorKind::TemplateFailure)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.kind.message())
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Both fields originate from a closed enum, never from database content.
        let body = format!(
            concat!(
                "<section role=\"alert\" aria-live=\"assertive\" class=\"cuadb-error\" data-error=\"{}\">",
                "<strong>Request failed</strong><p>{}</p></section>"
            ),
            self.kind.code(),
            self.kind.message()
        );
        let mut response = (self.kind.status(), Html(body)).into_response();
        response
            .headers_mut()
            .insert("x-cuadb-error", HeaderValue::from_static(self.kind.code()));
        response
    }
}

#[cfg(test)]
mod tests {
    use super::{AppError, ErrorKind};
    use axum::response::IntoResponse;

    #[test]
    fn error_categories_have_stable_statuses() {
        for (kind, status) in [
            (ErrorKind::InvalidInput, 400),
            (ErrorKind::Forbidden, 403),
            (ErrorKind::DatabaseUnavailable, 503),
            (ErrorKind::TimedOut, 504),
            (ErrorKind::Overloaded, 503),
            (ErrorKind::PayloadTooLarge, 413),
            (ErrorKind::TemplateFailure, 500),
            (ErrorKind::Internal, 500),
        ] {
            assert_eq!(kind.status().as_u16(), status, "{}", kind.code());
            assert_eq!(AppError::new(kind).into_response().status(), kind.status());
        }
    }

    #[test]
    fn public_error_messages_contain_no_raw_internal_data() {
        let password = "password-not-in-public-error";
        for kind in [
            ErrorKind::InvalidInput,
            ErrorKind::Forbidden,
            ErrorKind::DatabaseUnavailable,
            ErrorKind::TimedOut,
            ErrorKind::Overloaded,
            ErrorKind::PayloadTooLarge,
            ErrorKind::TemplateFailure,
            ErrorKind::Internal,
        ] {
            let error = AppError::new(kind);
            assert!(!error.to_string().contains(password));
            assert!(!error.to_string().contains("postgresql://"));
            assert!(!format!("{error:?}").contains(password));
        }
    }
}
