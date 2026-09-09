// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::sync::Arc;

use ldk_node::bitcoin::FeeRate;
use ldk_server_grpc::api::{OnchainBumpFeeRequest, OnchainBumpFeeResponse};

use crate::api::error::LdkServerError;
use crate::api::parse_payment_id;
use crate::service::Context;

pub(crate) async fn handle_onchain_bump_fee_request(
	context: Arc<Context>, request: OnchainBumpFeeRequest,
) -> Result<OnchainBumpFeeResponse, LdkServerError> {
	let payment_id = parse_payment_id(&request.payment_id)?;
	let fee_rate = parse_fee_rate(request.fee_rate_sat_per_vb)?;
	let txid = context.node.onchain_payment().bump_fee_rbf(payment_id, fee_rate)?;
	Ok(OnchainBumpFeeResponse { txid: txid.to_string() })
}

fn parse_fee_rate(rate: Option<u64>) -> Result<Option<FeeRate>, LdkServerError> {
	rate.map(|rate| {
		if rate == 0 {
			return Err(ldk_node::NodeError::InvalidFeeRate.into());
		}
		FeeRate::from_sat_per_vb(rate).ok_or_else(|| ldk_node::NodeError::InvalidFeeRate.into())
	})
	.transpose()
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::api::error::LdkServerErrorCode;

	#[test]
	fn fee_rate_is_optional_and_checked() {
		assert_eq!(parse_fee_rate(None).unwrap(), None);
		assert_eq!(parse_fee_rate(Some(2)).unwrap().unwrap().to_sat_per_kwu(), 500);
		assert!(parse_fee_rate(Some(u64::MAX / 250)).is_ok());
		for rate in [0, u64::MAX / 250 + 1, u64::MAX] {
			assert_eq!(
				parse_fee_rate(Some(rate)).unwrap_err().error_code,
				LdkServerErrorCode::InvalidRequestError
			);
		}
	}
}
