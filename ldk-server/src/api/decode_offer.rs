// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::str::FromStr;

use hex::prelude::*;
use ldk_node::lightning::bitcoin::blockdata::constants::ChainHash;
use ldk_node::lightning::bitcoin::Network;
use ldk_node::lightning::offers::offer::Offer;
use ldk_node::lightning_types::features::OfferFeatures;
use ldk_server_grpc::api::{DecodeOfferRequest, DecodeOfferResponse};
use ldk_server_grpc::types::offer_amount::Amount;
use ldk_server_grpc::types::offer_quantity::Quantity;
use ldk_server_grpc::types::{BlindedPath, CurrencyAmount, OfferAmount, OfferQuantity};

use crate::api::decode_features;
use crate::api::error::LdkServerError;
use crate::service::Context;
use std::sync::Arc;

pub(crate) async fn handle_decode_offer_request(
	_context: Arc<Context>, request: DecodeOfferRequest,
) -> Result<DecodeOfferResponse, LdkServerError> {
	let offer =
		Offer::from_str(request.offer.as_str()).map_err(|_| ldk_node::NodeError::InvalidOffer)?;

	let offer_id = offer.id().0.to_lower_hex_string();

	let description = offer.description().map(|d| d.to_string());

	let issuer = offer.issuer().map(|i| i.to_string());

	let amount = offer.amount().map(|a| match a {
		ldk_node::lightning::offers::offer::Amount::Bitcoin { amount_msats } => {
			OfferAmount { amount: Some(Amount::BitcoinAmountMsats(amount_msats)) }
		},
		ldk_node::lightning::offers::offer::Amount::Currency { iso4217_code, amount } => {
			OfferAmount {
				amount: Some(Amount::CurrencyAmount(CurrencyAmount {
					iso4217_code: iso4217_code.to_string(),
					amount,
				})),
			}
		},
	});

	let issuer_signing_pubkey = offer.issuer_signing_pubkey().map(|pk| pk.to_string());

	let absolute_expiry = offer.absolute_expiry().map(|d| d.as_secs());

	let quantity = Some(match offer.supported_quantity() {
		ldk_node::lightning::offers::offer::Quantity::One => {
			OfferQuantity { quantity: Some(Quantity::One(true)) }
		},
		ldk_node::lightning::offers::offer::Quantity::Bounded(max) => {
			OfferQuantity { quantity: Some(Quantity::Bounded(max.get())) }
		},
		ldk_node::lightning::offers::offer::Quantity::Unbounded => {
			OfferQuantity { quantity: Some(Quantity::Unbounded(true)) }
		},
	});

	let paths = offer
		.paths()
		.iter()
		.map(|path| {
			let (introduction_node_id, introduction_scid) = match path.introduction_node() {
				ldk_node::lightning::blinded_path::IntroductionNode::NodeId(pk) => {
					(Some(pk.to_string()), None)
				},
				ldk_node::lightning::blinded_path::IntroductionNode::DirectedShortChannelId(
					_dir,
					scid,
				) => (None, Some(*scid)),
			};
			BlindedPath {
				introduction_node_id,
				blinding_point: path.blinding_point().to_string(),
				num_hops: path.blinded_hops().len() as u32,
				introduction_scid,
			}
		})
		.collect();

	let features = decode_features(offer.offer_features().le_flags(), |bytes| {
		OfferFeatures::from_le_bytes(bytes).to_string()
	});

	let chains = offer.chains().into_iter().map(chain_hash_to_name).collect();

	let metadata = offer.metadata().map(|m| m.to_lower_hex_string());

	let is_expired = offer.is_expired();

	Ok(DecodeOfferResponse {
		offer_id,
		description,
		issuer,
		amount,
		issuer_signing_pubkey,
		absolute_expiry,
		quantity,
		paths,
		features,
		chains,
		metadata,
		is_expired,
	})
}

fn chain_hash_to_name(chain: ChainHash) -> String {
	if chain == ChainHash::using_genesis_block(Network::Bitcoin) {
		"bitcoin".to_string()
	} else if chain == ChainHash::using_genesis_block(Network::Testnet) {
		"testnet".to_string()
	} else if chain == ChainHash::using_genesis_block(Network::Regtest) {
		"regtest".to_string()
	} else if chain == ChainHash::using_genesis_block(Network::Signet) {
		"signet".to_string()
	} else {
		chain.to_string()
	}
}
