//! HTTP routing plus small, bounded request middleware.
//!
//! The limits apply to the handler future; streaming response bodies and later
//! database tasks must enforce their own byte/row/query deadlines as well.

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use axum::{
    Router,
    extract::{DefaultBodyLimit, MatchedPath, Request, State},
    http::{Method, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use tokio::{sync::Semaphore, time};

use crate::{
    config::Limits,
    error::{AppError, ErrorKind},
    web,
};

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

/// Router-level limits share permits across requests, not per handler.
#[derive(Debug, Clone)]
pub struct RequestPolicy {
    timeout: Duration,
    max_body_bytes: usize,
    in_flight: Arc<Semaphore>,
}

impl RequestPolicy {
    /// Limits have already been range-checked by the typed config parser.
    pub fn from_limits(limits: &Limits) -> Self {
        Self {
            timeout: limits.request_timeout,
            max_body_bytes: limits.max_request_body_bytes,
            in_flight: Arc::new(Semaphore::new(limits.max_in_flight_requests)),
        }
    }
}

/// Construct the default disconnected shell for isolated HTTP tests.
pub fn router() -> Router {
    let defaults = crate::config::Config::from_values(&[])
        .expect("constant default CuaDB configuration must parse");
    router_with_limits(&defaults.limits)
}

fn api_routes() -> Router<crate::state::AppState> {
    Router::new()
        .route("/", get(web::explorer))
        .route("/health", get(web::health))
        .route("/fragments/status", get(web::status))
        .route("/connection/status", get(web::connection_status))
        .route("/login", get(web::auth::login_form).post(web::auth::login))
        .route("/logout", post(web::auth::logout))
        .route("/account/password", post(web::auth::account_password))
        .route(
            "/admin/users",
            get(web::auth::admin_users).post(web::auth::admin_create_user),
        )
        .route("/admin/users/status", post(web::auth::admin_user_status))
        .route("/admin/users/role", post(web::auth::admin_user_role))
        .route("/admin/users/delete", post(web::auth::admin_user_delete))
        .route(
            "/admin/users/password",
            post(web::auth::admin_user_password),
        )
        .route(
            "/admin/grants",
            get(web::auth::admin_grants).post(web::auth::admin_grant_add),
        )
        .route("/admin/grants/remove", post(web::auth::admin_grant_remove))
        .route("/admin/audit", get(web::auth::admin_audit))
        .route("/admin", get(web::auth::admin_view))
        .route(
            "/admin/connections",
            get(web::connections::list).post(web::connections::create),
        )
        .route("/admin/connections/form", get(web::connections::form))
        .route("/admin/connections/reveal", get(web::connections::reveal))
        .route("/admin/connections/update", post(web::connections::update))
        .route("/admin/connections/delete", post(web::connections::delete))
        .route("/admin/connections/test", post(web::connections::test))
        .route("/connection/reconnect", post(web::connections::reconnect))
        .route("/connection/disconnect", post(web::connections::disconnect))
        .route("/connection/lifecycle", post(web::connections::lifecycle))
        .route("/object/actions", get(web::objects::dialog))
        .route("/object/actions/preview", post(web::objects::preview))
        .route("/object/actions/run", post(web::objects::run))
        .route("/admin/security", get(web::auth::admin_security))
        .route("/sql", get(web::sql::workspace_page))
        .route("/sql/hints", get(web::sql::hints))
        .route("/ai/schema", get(web::sql::ai_schema))
        .route("/ai/describe", get(web::sql::ai_describe))
        .route("/ai/search", get(web::sql::ai_search))
        .route("/ai/relations", get(web::sql::ai_relations))
        .route("/connection/profiles", get(web::connection_profiles))
        .route("/connection/databases", get(web::connection_databases))
        .route("/connection/use", post(web::connection_use))
        .route("/sql/execute", post(web::sql::execute))
        .route("/sql/cancel", post(web::sql::cancel))
        .route("/data/editor", get(web::sql::editor))
        .route("/data/lookup", get(web::lookup::lookup))
        .route("/data/insert", post(web::sql::insert))
        .route("/data/update", post(web::sql::update))
        .route("/data/delete", post(web::sql::delete))
        .route("/tx/begin", post(web::sql::tx_begin))
        .route("/tx/commit", post(web::sql::tx_commit))
        .route("/tx/rollback", post(web::sql::tx_rollback))
        .route("/tx/status", get(web::sql::tx_status))
        .route("/catalog/schemas", get(web::catalog::schemas))
        .route("/catalog/relations", get(web::catalog::relations))
        .route("/catalog/structure", get(web::catalog::structure))
        .route("/catalog/indexes", get(web::catalog::indexes))
        .route("/catalog/stats", get(web::catalog::stats))
        .route("/catalog/advanced", get(web::catalog::advanced))
        .route("/catalog/overview", get(web::catalog::overview))
        .route("/catalog/objects", get(web::catalog::objects))
        .route("/catalog/object", get(web::catalog::object))
        .route("/catalog/columns", get(web::catalog::columns))
        .route("/catalog/links", get(web::catalog::links))
        .route("/catalog/data", get(web::rows::data))
        .route("/catalog/cell", get(web::rows::cell))
        .route("/catalog/foreign", get(web::rows::foreign_row))
        .route("/catalog/export", get(web::export::export))
        .route("/sw.js", get(crate::assets::service_worker))
        .route("/static/{name}", get(crate::assets::serve))
        .fallback(get(web::explorer))
}

/// Construct the router for an unconfigured (disconnected) server.
pub fn router_with_limits(limits: &Limits) -> Router {
    router_with_state(limits, crate::state::AppState::disconnected())
}

/// Real server router shares one connection state across all request handlers.
/// The authentication layer is always installed: it either resolves a session or,
/// while auth is not enforced, supplies the shared public connection principal.
pub fn router_with_state(limits: &Limits, state: crate::state::AppState) -> Router {
    let routes = api_routes()
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(
            state,
            web::auth::authenticate,
        ));
    apply_policy(routes, RequestPolicy::from_limits(limits))
}

fn apply_policy(router: Router, policy: RequestPolicy) -> Router {
    let body_limit = policy.max_body_bytes;
    router
        // DefaultBodyLimit covers future Axum body extractors.
        .layer(DefaultBodyLimit::max(body_limit))
        .layer(middleware::from_fn_with_state(policy, bound_request))
        // Outermost: every completed request, including rejected ones, gets a
        // generated request ID. Never log a URI, query string, headers or body.
        .layer(middleware::from_fn(security_headers))
        .layer(middleware::from_fn(trace_request))
}

async fn security_headers(request: Request, next: Next) -> Response {
    let is_static = request.uri().path().starts_with("/static/");
    let is_html_route =
        !request.uri().path().starts_with("/static/") && request.uri().path() != "/health";
    let mut response = next.run(request).await;
    let no_store = is_html_route || request_is_html(&response);
    let headers = response.headers_mut();
    if !is_static {
        headers.insert("x-content-type-options", "nosniff".parse().unwrap());
        headers.insert("x-frame-options", "DENY".parse().unwrap());
        // `same-origin`, not `no-referrer`: with `no-referrer` browsers send `Origin: null`
        // on same-origin form posts, which the CSRF origin check must then refuse (login).
        headers.insert("referrer-policy", "same-origin".parse().unwrap());
        headers.insert(
            "content-security-policy",
            "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self' https://generativelanguage.googleapis.com https://api.openai.com https://api.anthropic.com https://openrouter.ai http://localhost:* http://127.0.0.1:*; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'"
                .parse()
                .unwrap(),
        );
        headers.insert(
            "cross-origin-resource-policy",
            "same-origin".parse().unwrap(),
        );
    }
    if no_store {
        headers.insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    }
    response
}

fn request_is_html(response: &Response) -> bool {
    response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/html"))
}

/// Immediately reject overload rather than allocating an unbounded wait queue.
async fn bound_request(
    State(policy): State<RequestPolicy>,
    request: Request,
    next: Next,
) -> Response {
    if let Some(header_value) = request.headers().get(header::CONTENT_LENGTH) {
        match header_value
            .to_str()
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
        {
            Some(size) if size <= policy.max_body_bytes as u64 => {}
            Some(_) => return AppError::new(ErrorKind::PayloadTooLarge).into_response(),
            None => return AppError::new(ErrorKind::InvalidInput).into_response(),
        }
    }

    let Ok(permit) = policy.in_flight.clone().try_acquire_owned() else {
        return AppError::new(ErrorKind::Overloaded).into_response();
    };

    // Dropping Next's future on timeout cancels in-flight handler work that
    // correctly respects cancellation; later detached SQL tasks need explicit
    // cancellation and independent query timeouts.
    let result = time::timeout(policy.timeout, next.run(request)).await;
    drop(permit);

    match result {
        Ok(response) => response,
        Err(_) => AppError::new(ErrorKind::TimedOut).into_response(),
    }
}

async fn trace_request(request: Request, next: Next) -> Response {
    let id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
    let request_id = format!("cuadb-{id:016x}");
    let method = if request.method() == Method::GET {
        "GET"
    } else if request.method() == Method::HEAD {
        "HEAD"
    } else if request.method() == Method::OPTIONS {
        "OPTIONS"
    } else {
        "OTHER"
    };
    // MatchedPath contains only the route pattern defined by this program.
    // Never substitute request.uri() because it may contain credentials.
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or("<unmatched>")
        .to_owned();
    let began = Instant::now();

    let mut response = next.run(request).await;
    let code = response
        .headers()
        .get("x-cuadb-error")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("none");
    eprintln!(
        concat!(
            "level=info event=http_request request_id={} method={} ",
            "route={} status={} error_code={} elapsed_ms={}"
        ),
        request_id,
        method,
        route,
        response.status().as_u16(),
        code,
        began.elapsed().as_millis()
    );

    // The value is generated internally and contains only hexadecimal digits.
    if let Ok(value) = request_id.parse() {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::{RequestPolicy, apply_policy, header, router};
    use axum::{
        Router,
        body::{Body, to_bytes},
        http::{Request, StatusCode},
        routing::get,
    };
    use std::{
        future::pending,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };
    use tokio::sync::Semaphore;
    use tower::ServiceExt;

    fn policy(timeout: Duration) -> RequestPolicy {
        RequestPolicy {
            timeout,
            max_body_bytes: 1024,
            in_flight: Arc::new(Semaphore::new(1)),
        }
    }

    #[tokio::test]
    async fn generates_request_id_without_echoing_query_values() {
        let response = router()
            .oneshot(
                Request::builder()
                    .uri("/health?secret=do_not_log_me")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().contains_key("x-request-id"));
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(body.as_ref(), b"ok");
    }

    #[tokio::test]
    async fn html_responses_are_no_store_and_have_browser_security_headers() {
        let response = router()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        assert_eq!(response.headers()["x-frame-options"], "DENY");
        assert!(
            response.headers()["content-security-policy"]
                .to_str()
                .unwrap()
                .contains("frame-ancestors 'none'")
        );
    }

    #[tokio::test]
    async fn rejects_oversized_requests() {
        let response = apply_policy(
            Router::new().route("/read", get(|| async { "ok" })),
            policy(Duration::from_millis(100)),
        )
        .oneshot(
            Request::builder()
                .uri("/read")
                .header("content-length", "1025")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn rejects_overloaded_requests_without_waiting() {
        let policy = policy(Duration::from_secs(1));
        let permit = policy.in_flight.clone().try_acquire_owned().unwrap();
        let response = apply_policy(Router::new().route("/read", get(|| async { "ok" })), policy)
            .oneshot(Request::builder().uri("/read").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        drop(permit);
    }

    #[tokio::test]
    async fn timeout_cancels_the_in_flight_handler_future() {
        struct OnDrop(Arc<AtomicBool>);
        impl Drop for OnDrop {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }

        let cancelled = Arc::new(AtomicBool::new(false));
        let marker = cancelled.clone();
        let route = Router::new().route(
            "/slow",
            get(move || {
                let marker = marker.clone();
                async move {
                    let _drop_guard = OnDrop(marker);
                    pending::<()>().await;
                    "unreachable"
                }
            }),
        );
        let response = apply_policy(route, policy(Duration::from_millis(10)))
            .oneshot(Request::builder().uri("/slow").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
        assert!(cancelled.load(Ordering::SeqCst));
    }
}
