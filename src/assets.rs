//! The web assets (CSS, JavaScript, icons) are compiled into the binary so a release
//! binary or `cargo install` build is one self-contained file. Debug builds read the
//! files from `public/` when present so front-end edits show up without a rebuild.

use axum::{
    body::Body,
    extract::Path,
    http::{HeaderValue, StatusCode, header},
    response::Response,
};

struct Asset {
    name: &'static str,
    bytes: &'static [u8],
    mime: &'static str,
}

macro_rules! asset {
    ($name:literal, $mime:literal) => {
        Asset {
            name: $name,
            bytes: include_bytes!(concat!("../public/", $name)),
            mime: $mime,
        }
    };
}

static ASSETS: &[Asset] = &[
    asset!("app.css", "text/css; charset=utf-8"),
    asset!("app.js", "text/javascript; charset=utf-8"),
    asset!("theme.js", "text/javascript; charset=utf-8"),
    asset!("htmx.min.js", "text/javascript; charset=utf-8"),
    asset!("sw.js", "text/javascript; charset=utf-8"),
    asset!("site.webmanifest", "application/manifest+json"),
    asset!("favicon.svg", "image/svg+xml"),
    asset!("icon-192.png", "image/png"),
    asset!("icon-512.png", "image/png"),
];

fn respond(name: &str, root_scope: bool) -> Response {
    let Some(asset) = ASSETS.iter().find(|a| a.name == name) else {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap_or_default();
    };
    let bytes: Vec<u8> = if cfg!(debug_assertions) {
        std::fs::read(format!("public/{}", asset.name)).unwrap_or_else(|_| asset.bytes.to_vec())
    } else {
        asset.bytes.to_vec()
    };
    let mut response = Response::new(Body::from(bytes));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(asset.mime));
    if root_scope {
        headers.insert("service-worker-allowed", HeaderValue::from_static("/"));
    }
    response
}

/// `GET /static/{name}`
pub async fn serve(Path(name): Path<String>) -> Response {
    respond(&name, false)
}

/// `GET /sw.js` — the service worker must be served from the root to control the app.
pub async fn service_worker() -> Response {
    respond("sw.js", true)
}
