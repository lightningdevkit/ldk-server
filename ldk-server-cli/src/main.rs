use clap::{Parser, Subcommand};
use ldk_server_client::client::LdkServerClient;
use ldk_server_client::error::LdkServerError;
use ldk_server_client::error::LdkServerErrorCode::{
	AuthError, InternalError, InternalServerError, InvalidRequestError, LightningError,
};
use ldk_server_client::ldk_server_protos::api::{
	Bolt11ReceiveRequest, Bolt11SendRequest, Bolt12ReceiveRequest, Bolt12SendRequest,
	CloseChannelRequest, ForceCloseChannelRequest, GetBalancesRequest, GetNodeInfoRequest,
	ListChannelsRequest, ListPaymentsRequest, ListPaymentsResponse, OnchainReceiveRequest,
	OnchainSendRequest, OpenChannelRequest,
};
use ldk_server_client::ldk_server_protos::types::{
	bolt11_invoice_description, Bolt11InvoiceDescription, PageToken,
};
use std::fmt::Debug;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
	#[arg(short, long, default_value = "localhost:3000")]
	base_url: String,

	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
	GetNodeInfo,
	GetBalances,
	OnchainReceive,
	OnchainSend {
		#[arg(short, long)]
		address: String,
		#[arg(long)]
		amount_sats: Option<u64>,
		#[arg(long)]
		send_all: Option<bool>,
		#[arg(long)]
		fee_rate_sat_per_vb: Option<u64>,
	},
	Bolt11Receive {
		#[arg(short, long)]
		description: Option<String>,
		#[arg(long)]
		description_hash: Option<String>,
		#[arg(short, long)]
		expiry_secs: u32,
		#[arg(long)]
		amount_msat: Option<u64>,
	},
	Bolt11Send {
		#[arg(short, long)]
		invoice: String,
		#[arg(long)]
		amount_msat: Option<u64>,
	},
	Bolt12Receive {
		#[arg(short, long)]
		description: String,
		#[arg(long)]
		amount_msat: Option<u64>,
		#[arg(long)]
		expiry_secs: Option<u32>,
		#[arg(long)]
		quantity: Option<u64>,
	},
	Bolt12Send {
		#[arg(short, long)]
		offer: String,
		#[arg(long)]
		amount_msat: Option<u64>,
		#[arg(short, long)]
		quantity: Option<u64>,
		#[arg(short, long)]
		payer_note: Option<String>,
	},
	CloseChannel {
		#[arg(short, long)]
		user_channel_id: String,
		#[arg(short, long)]
		counterparty_node_id: String,
	},
	ForceCloseChannel {
		#[arg(short, long)]
		user_channel_id: String,
		#[arg(short, long)]
		counterparty_node_id: String,
		#[arg(long)]
		force_close_reason: Option<String>,
	},
	OpenChannel {
		#[arg(short, long)]
		node_pubkey: String,
		#[arg(short, long)]
		address: String,
		#[arg(long)]
		channel_amount_sats: u64,
		#[arg(long)]
		push_to_counterparty_msat: Option<u64>,
		#[arg(long)]
		announce_channel: bool,
	},
	ListChannels,
	ListPayments {
		#[arg(short, long)]
		#[arg(
			help = "Fetch at least this many payments by iterating through multiple pages. Returns combined results with the last page token. If not provided, returns only a single page."
		)]
		number_of_payments: Option<u64>,
		#[arg(long)]
		#[arg(help = "Page token value to continue from a previous page")]
		page_token_value: Option<String>,
		#[arg(long)]
		#[arg(help = "Page token index to continue from a previous page")]
		page_token_index: Option<i64>,
	},
}

#[tokio::main]
async fn main() {
	let cli = Cli::parse();
	let client = LdkServerClient::new(cli.base_url);

	match cli.command {
		Commands::GetNodeInfo => {
			handle_response_result(client.get_node_info(GetNodeInfoRequest {}).await);
		},
		Commands::GetBalances => {
			handle_response_result(client.get_balances(GetBalancesRequest {}).await);
		},
		Commands::OnchainReceive => {
			handle_response_result(client.onchain_receive(OnchainReceiveRequest {}).await);
		},
		Commands::OnchainSend { address, amount_sats, send_all, fee_rate_sat_per_vb } => {
			handle_response_result(
				client
					.onchain_send(OnchainSendRequest {
						address,
						amount_sats,
						send_all,
						fee_rate_sat_per_vb,
					})
					.await,
			);
		},
		Commands::Bolt11Receive { description, description_hash, expiry_secs, amount_msat } => {
			let invoice_description = match (description, description_hash) {
				(Some(desc), None) => Some(Bolt11InvoiceDescription {
					kind: Some(bolt11_invoice_description::Kind::Direct(desc)),
				}),
				(None, Some(hash)) => Some(Bolt11InvoiceDescription {
					kind: Some(bolt11_invoice_description::Kind::Hash(hash)),
				}),
				(Some(_), Some(_)) => {
					handle_error(LdkServerError::new(
						InternalError,
						"Only one of description or description_hash can be set.".to_string(),
					));
				},
				(None, None) => None,
			};

			let request =
				Bolt11ReceiveRequest { description: invoice_description, expiry_secs, amount_msat };

			handle_response_result(client.bolt11_receive(request).await);
		},
		Commands::Bolt11Send { invoice, amount_msat } => {
			handle_response_result(
				client.bolt11_send(Bolt11SendRequest { invoice, amount_msat }).await,
			);
		},
		Commands::Bolt12Receive { description, amount_msat, expiry_secs, quantity } => {
			handle_response_result(
				client
					.bolt12_receive(Bolt12ReceiveRequest {
						description,
						amount_msat,
						expiry_secs,
						quantity,
					})
					.await,
			);
		},
		Commands::Bolt12Send { offer, amount_msat, quantity, payer_note } => {
			handle_response_result(
				client
					.bolt12_send(Bolt12SendRequest { offer, amount_msat, quantity, payer_note })
					.await,
			);
		},
		Commands::CloseChannel { user_channel_id, counterparty_node_id } => {
			handle_response_result(
				client
					.close_channel(CloseChannelRequest { user_channel_id, counterparty_node_id })
					.await,
			);
		},
		Commands::ForceCloseChannel {
			user_channel_id,
			counterparty_node_id,
			force_close_reason,
		} => {
			handle_response_result(
				client
					.force_close_channel(ForceCloseChannelRequest {
						user_channel_id,
						counterparty_node_id,
						force_close_reason,
					})
					.await,
			);
		},
		Commands::OpenChannel {
			node_pubkey,
			address,
			channel_amount_sats,
			push_to_counterparty_msat,
			announce_channel,
		} => {
			handle_response_result(
				client
					.open_channel(OpenChannelRequest {
						node_pubkey,
						address,
						channel_amount_sats,
						push_to_counterparty_msat,
						channel_config: None,
						announce_channel,
					})
					.await,
			);
		},
		Commands::ListChannels => {
			handle_response_result(client.list_channels(ListChannelsRequest {}).await);
		},
		Commands::ListPayments { number_of_payments, page_token_value, page_token_index } => {
			handle_response_result(
				handle_list_payments(
					client,
					number_of_payments,
					page_token_value,
					page_token_index,
				)
				.await,
			);
		},
	}
}

async fn handle_list_payments(
	client: LdkServerClient, number_of_payments: Option<u64>, page_token_value: Option<String>,
	page_token_index: Option<i64>,
) -> Result<ListPaymentsResponse, LdkServerError> {
	if page_token_value.is_some() != page_token_index.is_some() {
		return Err(LdkServerError::new(
			InternalError,
			"Both --page-token-value and --page-token-index must be provided together".to_string(),
		));
	}

	let initial_page_token = match (page_token_value, page_token_index) {
		(Some(token), Some(index)) => Some(PageToken { token, index }),
		_ => None,
	};

	if let Some(count) = number_of_payments {
		list_n_payments(client, count, initial_page_token).await
	} else {
		// Fetch single page
		client.list_payments(ListPaymentsRequest { page_token: initial_page_token }).await
	}
}

async fn list_n_payments(
	client: LdkServerClient, target_count: u64, initial_page_token: Option<PageToken>,
) -> Result<ListPaymentsResponse, LdkServerError> {
	let mut payments = Vec::with_capacity(target_count as usize);
	let mut page_token = initial_page_token;
	let mut next_page_token;

	loop {
		let response = client.list_payments(ListPaymentsRequest { page_token }).await?;

		payments.extend(response.payments);
		next_page_token = response.next_page_token;

		if payments.len() >= target_count as usize || next_page_token.is_none() {
			break;
		}
		page_token = next_page_token.clone();
	}

	Ok(ListPaymentsResponse { payments, next_page_token })
}

fn handle_response_result<Rs: Debug>(response: Result<Rs, LdkServerError>) {
	match response {
		Ok(response) => {
			println!("Success: {:?}", response);
		},
		Err(e) => {
			handle_error(e);
		},
	}
}

fn handle_error(e: LdkServerError) -> ! {
	let error_type = match e.error_code {
		InvalidRequestError => "Invalid Request",
		AuthError => "Authentication Error",
		LightningError => "Lightning Error",
		InternalServerError => "Internal Server Error",
		InternalError => "Internal Error",
	};
	eprintln!("Error ({}): {}", error_type, e.message);
	std::process::exit(1); // Exit with status code 1 on error.
}
