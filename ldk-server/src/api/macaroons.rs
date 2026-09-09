// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::sync::Arc;

use ldk_server_grpc::api::{
	CreateMacaroonRequest, CreateMacaroonResponse, GetPermissionsRequest, GetPermissionsResponse,
	ListMacaroonsRequest, ListMacaroonsResponse, MacaroonInfo as ProtoMacaroonInfo,
	RevokeMacaroonRequest, RevokeMacaroonResponse,
};

use super::error::{LdkServerError, LdkServerErrorCode};
use crate::macaroons::{MacaroonInfo, MacaroonStore};

pub(crate) async fn handle_create_macaroon_request(
	store: Arc<MacaroonStore>, issuer: Arc<MacaroonInfo>, request: CreateMacaroonRequest,
) -> Result<CreateMacaroonResponse, LdkServerError> {
	let created =
		run_blocking(move || store.create_root(&request.name, request.permissions, &issuer))
			.await?;
	Ok(CreateMacaroonResponse {
		macaroon: Some(macaroon_to_proto(created.info)),
		token: created.token,
	})
}

pub(crate) async fn handle_list_macaroons_request(
	store: Arc<MacaroonStore>, _request: ListMacaroonsRequest,
) -> Result<ListMacaroonsResponse, LdkServerError> {
	let macaroons = store.list_roots()?.into_iter().map(macaroon_to_proto).collect();
	Ok(ListMacaroonsResponse { macaroons })
}

pub(crate) async fn handle_revoke_macaroon_request(
	store: Arc<MacaroonStore>, issuer: Arc<MacaroonInfo>, request: RevokeMacaroonRequest,
) -> Result<RevokeMacaroonResponse, LdkServerError> {
	run_blocking(move || store.revoke_root(&request.id, &issuer)).await?;
	Ok(RevokeMacaroonResponse {})
}

pub(crate) async fn handle_get_permissions_request(
	issuer: Arc<MacaroonInfo>, _request: GetPermissionsRequest,
) -> Result<GetPermissionsResponse, LdkServerError> {
	Ok(GetPermissionsResponse { macaroon: Some(macaroon_to_proto((*issuer).clone())) })
}

fn macaroon_to_proto(info: MacaroonInfo) -> ProtoMacaroonInfo {
	ProtoMacaroonInfo {
		id: info.id,
		name: info.name,
		permissions: info.permissions.into_iter().collect(),
		caveats: info.caveats,
	}
}

async fn run_blocking<T: Send + 'static>(
	f: impl FnOnce() -> Result<T, LdkServerError> + Send + 'static,
) -> Result<T, LdkServerError> {
	tokio::task::spawn_blocking(f).await.map_err(|error| {
		LdkServerError::new(LdkServerErrorCode::InternalServerError, error.to_string())
	})?
}
