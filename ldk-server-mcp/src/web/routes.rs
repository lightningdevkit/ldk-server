// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;

/// Builds the gateway's HTTP router. v1 only exposes `/healthz`; subsequent
/// PRs add `/api/*` (UI), `/mcp` (MCP Streamable HTTP), and the static UI.
pub fn build_router() -> Router {
	Router::new().route("/healthz", get(healthz))
}

async fn healthz() -> impl IntoResponse {
	(StatusCode::OK, "ok")
}

#[cfg(test)]
mod tests {
	use axum::body::Body;
	use axum::http::Request;
	use tower::ServiceExt;

	use super::*;

	#[tokio::test]
	async fn healthz_returns_ok() {
		let app = build_router();
		let response = app
			.oneshot(Request::builder().uri("/healthz").body(Body::empty()).unwrap())
			.await
			.unwrap();

		assert_eq!(response.status(), StatusCode::OK);
		let body = axum::body::to_bytes(response.into_body(), 1024).await.unwrap();
		assert_eq!(&body[..], b"ok");
	}

	#[tokio::test]
	async fn unknown_path_returns_404() {
		let app = build_router();
		let response = app
			.oneshot(Request::builder().uri("/nope").body(Body::empty()).unwrap())
			.await
			.unwrap();

		assert_eq!(response.status(), StatusCode::NOT_FOUND);
	}
}
