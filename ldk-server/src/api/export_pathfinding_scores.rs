// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use ldk_server_protos::api::{ExportPathfindingScoresRequest, ExportPathfindingScoresResponse};

use crate::api::error::LdkServerError;
use crate::service::Context;

pub(crate) fn handle_export_pathfinding_scores_request(
	context: Context, _request: ExportPathfindingScoresRequest,
) -> Result<ExportPathfindingScoresResponse, LdkServerError> {
	let scores = context.node.export_pathfinding_scores()?;

	let response = ExportPathfindingScoresResponse { scores: scores.into() };
	Ok(response)
}
