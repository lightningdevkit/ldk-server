// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::fmt::Write;
use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use config::{
	api_key_path_for_storage_dir, cert_path_for_storage_dir, get_default_api_key_path,
	get_default_cert_path, get_default_config_path, load_config, resolve_base_url,
	DEFAULT_GRPC_SERVICE_ADDRESS,
};
use hex_conservative::DisplayHex;
use ldk_server_client::client::LdkServerClient;
use ldk_server_client::error::LdkServerError;
use ldk_server_client::error::LdkServerErrorCode::{
	AuthError, InternalError, InternalServerError, InvalidRequestError, LightningError,
};
use ldk_server_client::ldk_server_protos::api::{
	Bolt11ClaimForHashRequest, Bolt11ClaimForHashResponse, Bolt11FailForHashRequest,
	Bolt11FailForHashResponse, Bolt11ReceiveForHashRequest, Bolt11ReceiveForHashResponse,
	Bolt11ReceiveRequest, Bolt11ReceiveResponse, Bolt11ReceiveVariableAmountViaJitChannelRequest,
	Bolt11ReceiveVariableAmountViaJitChannelResponse, Bolt11ReceiveViaJitChannelRequest,
	Bolt11ReceiveViaJitChannelResponse, Bolt11SendRequest, Bolt11SendResponse,
	Bolt12ReceiveRequest, Bolt12ReceiveResponse, Bolt12SendRequest, Bolt12SendResponse,
	CloseChannelRequest, CloseChannelResponse, ConnectPeerRequest, ConnectPeerResponse,
	DecodeInvoiceRequest, DecodeInvoiceResponse, DecodeOfferRequest, DecodeOfferResponse,
	DisconnectPeerRequest, DisconnectPeerResponse, ExportPathfindingScoresRequest,
	ForceCloseChannelRequest, ForceCloseChannelResponse, GetBalancesRequest, GetBalancesResponse,
	GetNodeInfoRequest, GetNodeInfoResponse, GetPaymentDetailsRequest, GetPaymentDetailsResponse,
	GraphGetChannelRequest, GraphGetChannelResponse, GraphGetNodeRequest, GraphGetNodeResponse,
	GraphListChannelsRequest, GraphListChannelsResponse, GraphListNodesRequest,
	GraphListNodesResponse, ListChannelsRequest, ListChannelsResponse,
	ListForwardedPaymentsRequest, ListPaymentsRequest, ListPeersRequest, ListPeersResponse,
	OnchainReceiveRequest, OnchainReceiveResponse, OnchainSendRequest, OnchainSendResponse,
	OpenChannelRequest, OpenChannelResponse, SignMessageRequest, SignMessageResponse,
	SpliceInRequest, SpliceInResponse, SpliceOutRequest, SpliceOutResponse, SpontaneousSendRequest,
	SpontaneousSendResponse, UnifiedSendRequest, UnifiedSendResponse, UpdateChannelConfigRequest,
	UpdateChannelConfigResponse, VerifySignatureRequest, VerifySignatureResponse,
};
use ldk_server_client::ldk_server_protos::types::{
	bolt11_invoice_description, Bolt11InvoiceDescription, ChannelConfig, PageToken,
	RouteParametersConfig,
};
use serde::Serialize;
use serde_json::{json, Value};
use types::{
	Amount, CliListForwardedPaymentsResponse, CliListPaymentsResponse, CliPaginatedResponse,
};

mod config;
mod types;

// Having these default values as constants in the Proto file and
// importing/reusing them here might be better, but Proto3 removed
// the ability to set default values.
const DEFAULT_MAX_TOTAL_CLTV_EXPIRY_DELTA: u32 = 1008;
const DEFAULT_MAX_PATH_COUNT: u32 = 10;
const DEFAULT_MAX_CHANNEL_SATURATION_POWER_OF_HALF: u32 = 2;
const DEFAULT_EXPIRY_SECS: u32 = 86_400;

const DEFAULT_DIR: &str = if cfg!(target_os = "macos") {
	"~/Library/Application Support/ldk-server"
} else if cfg!(target_os = "windows") {
	"%APPDATA%\\ldk-server"
} else {
	"~/.ldk-server"
};

#[derive(Parser, Debug)]
#[command(
	name = "ldk-server-cli",
	version,
	about = "CLI for interacting with an LDK Server node",
	override_usage = "ldk-server-cli [OPTIONS] <COMMAND>"
)]
struct Cli {
	#[arg(
		short,
		long,
		help = format!(
			"Base URL of the server. Defaults to config file or {DEFAULT_GRPC_SERVICE_ADDRESS}"
		)
	)]
	base_url: Option<String>,

	#[arg(short, long, help = format!("API key for authentication. Defaults by reading {DEFAULT_DIR}/[network]/api_key"))]
	api_key: Option<String>,

	#[arg(short, long, help = format!("Path to the server's TLS certificate file (PEM format). Defaults to {DEFAULT_DIR}/tls.crt"))]
	tls_cert: Option<String>,

	#[arg(short, long, help = format!("Path to config file. Defaults to {DEFAULT_DIR}/config.toml"))]
	config: Option<String>,

	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
	#[command(about = "Retrieve the latest node info like node_id, current_best_block, etc")]
	GetNodeInfo,
	#[command(about = "Retrieve an overview of all known balances")]
	GetBalances,
	#[command(about = "Retrieve a new on-chain funding address")]
	OnchainReceive,
	#[command(about = "Send an on-chain payment to the given address")]
	OnchainSend {
		#[arg(help = "The address to send coins to")]
		address: String,
		#[arg(
			help = "The amount to send, e.g. 50sat or 50000msat, must be a whole sat amount, cannot send msats on-chain. Will respect any on-chain reserve needed for anchor channels"
		)]
		amount: Option<Amount>,
		#[arg(
			long,
			help = "Send full balance to the address. Warning: will not retain on-chain reserves for anchor channels"
		)]
		send_all: Option<bool>,
		#[arg(
			long,
			help = "Fee rate in satoshis per virtual byte. If not set, a reasonable estimate will be used"
		)]
		fee_rate_sat_per_vb: Option<u64>,
	},
	#[command(about = "Create a BOLT11 invoice to receive a payment")]
	Bolt11Receive {
		#[arg(
			help = "Amount to request, e.g. 50sat or 50000msat. If unset, a variable-amount invoice is returned"
		)]
		amount: Option<Amount>,
		#[arg(short, long, help = "Description to attach along with the invoice")]
		description: Option<String>,
		#[arg(
			long,
			help = "SHA-256 hash of the description (hex). Use instead of description for longer text"
		)]
		description_hash: Option<String>,
		#[arg(short, long, help = "Invoice expiry time in seconds (default: 86400)")]
		expiry_secs: Option<u32>,
	},
	#[command(
		about = "Create a BOLT11 hodl invoice for a given payment hash (manual claim required)"
	)]
	Bolt11ReceiveForHash {
		#[arg(help = "The hex-encoded 32-byte payment hash")]
		payment_hash: String,
		#[arg(
			help = "Amount to request, e.g. 50sat or 50000msat. If unset, a variable-amount invoice is returned"
		)]
		amount: Option<Amount>,
		#[arg(short, long, help = "Description to attach along with the invoice")]
		description: Option<String>,
		#[arg(
			long,
			help = "SHA-256 hash of the description (hex). Use instead of description for longer text"
		)]
		description_hash: Option<String>,
		#[arg(short, long, help = "Invoice expiry time in seconds (default: 86400)")]
		expiry_secs: Option<u32>,
	},
	#[command(about = "Claim a held payment by providing the preimage")]
	Bolt11ClaimForHash {
		#[arg(help = "The hex-encoded 32-byte payment preimage")]
		preimage: String,
		#[arg(
			short,
			long,
			help = "The claimable amount, e.g. 50sat or 50000msat, only used for verifying we are claiming the expected amount"
		)]
		claimable_amount: Option<Amount>,
		#[arg(
			short,
			long,
			help = "The hex-encoded 32-byte payment hash, used to verify the preimage matches"
		)]
		payment_hash: Option<String>,
	},
	#[command(about = "Fail/reject a held payment")]
	Bolt11FailForHash {
		#[arg(help = "The hex-encoded 32-byte payment hash")]
		payment_hash: String,
	},
	#[command(about = "Create a fixed-amount BOLT11 invoice to receive via an LSPS2 JIT channel")]
	Bolt11ReceiveViaJitChannel {
		#[arg(help = "Amount to request, e.g. 50sat or 50000msat")]
		amount: Amount,
		#[arg(short, long, help = "Description to attach along with the invoice")]
		description: Option<String>,
		#[arg(
			long,
			help = "SHA-256 hash of the description (hex). Use instead of description for longer text"
		)]
		description_hash: Option<String>,
		#[arg(short, long, help = "Invoice expiry time in seconds (default: 86400)")]
		expiry_secs: Option<u32>,
		#[arg(
			long,
			help = "Maximum total fee an LSP may deduct for opening the JIT channel, e.g. 50sat or 50000msat"
		)]
		max_total_lsp_fee_limit: Option<Amount>,
	},
	#[command(
		about = "Create a variable-amount BOLT11 invoice to receive via an LSPS2 JIT channel"
	)]
	Bolt11ReceiveVariableAmountViaJitChannel {
		#[arg(short, long, help = "Description to attach along with the invoice")]
		description: Option<String>,
		#[arg(
			long,
			help = "SHA-256 hash of the description (hex). Use instead of description for longer text"
		)]
		description_hash: Option<String>,
		#[arg(short, long, help = "Invoice expiry time in seconds (default: 86400)")]
		expiry_secs: Option<u32>,
		#[arg(long, help = "Maximum proportional fee the LSP may deduct in ppm-msat")]
		max_proportional_lsp_fee_limit_ppm_msat: Option<u64>,
	},
	#[command(about = "Pay a BOLT11 invoice")]
	Bolt11Send {
		#[arg(help = "A BOLT11 invoice for a payment within the Lightning Network")]
		invoice: String,
		#[arg(
			help = "Amount to send, e.g. 50sat or 50000msat. Required when paying a zero-amount invoice"
		)]
		amount: Option<Amount>,
		#[arg(
			long,
			help = "Maximum total routing fee, e.g. 50sat or 50000msat. Defaults to 1% of payment + 50 sats"
		)]
		max_total_routing_fee: Option<Amount>,
		#[arg(long, help = "Maximum total CLTV delta we accept for the route (default: 1008)")]
		max_total_cltv_expiry_delta: Option<u32>,
		#[arg(
			long,
			help = "Maximum number of paths that may be used by MPP payments (default: 10)"
		)]
		max_path_count: Option<u32>,
		#[arg(
			long,
			help = "Maximum share of a channel's total capacity to send over a channel, as a power of 1/2 (default: 2)"
		)]
		max_channel_saturation_power_of_half: Option<u32>,
	},
	#[command(about = "Return a BOLT12 offer for receiving payments")]
	Bolt12Receive {
		#[arg(help = "Description to attach along with the offer")]
		description: String,
		#[arg(
			help = "Amount to request, e.g. 50sat or 50000msat. If unset, a variable-amount offer is returned"
		)]
		amount: Option<Amount>,
		#[arg(long, help = "Offer expiry time in seconds")]
		expiry_secs: Option<u32>,
		#[arg(long, help = "Number of items requested. Can only be set for fixed-amount offers")]
		quantity: Option<u64>,
	},
	#[command(about = "Send a payment for a BOLT12 offer")]
	Bolt12Send {
		#[arg(help = "A BOLT12 offer for a payment within the Lightning Network")]
		offer: String,
		#[arg(
			help = "Amount to send, e.g. 50sat or 50000msat. Required when paying a zero-amount offer"
		)]
		amount: Option<Amount>,
		#[arg(short, long, help = "Number of items requested")]
		quantity: Option<u64>,
		#[arg(
			short,
			long,
			help = "Note to include for the payee. Will be seen by recipient and reflected back in the invoice"
		)]
		payer_note: Option<String>,
		#[arg(
			long,
			help = "Maximum total routing fee, e.g. 50sat or 50000msat. Defaults to 1% of the payment amount + 50 sats"
		)]
		max_total_routing_fee: Option<Amount>,
		#[arg(long, help = "Maximum total CLTV delta we accept for the route (default: 1008)")]
		max_total_cltv_expiry_delta: Option<u32>,
		#[arg(
			long,
			help = "Maximum number of paths that may be used by MPP payments (default: 10)"
		)]
		max_path_count: Option<u32>,
		#[arg(
			long,
			help = "Maximum share of a channel's total capacity to send over a channel, as a power of 1/2 (default: 2)"
		)]
		max_channel_saturation_power_of_half: Option<u32>,
	},
	#[command(about = "Send a spontaneous payment (keysend) to a node")]
	SpontaneousSend {
		#[arg(help = "The hex-encoded public key of the node to send the payment to")]
		node_id: String,
		#[arg(help = "The amount to send, e.g. 50sat or 50000msat")]
		amount: Amount,
		#[arg(
			long,
			help = "Maximum total routing fee, e.g. 50sat or 50000msat. Defaults to 1% of payment + 50 sats"
		)]
		max_total_routing_fee: Option<Amount>,
		#[arg(long, help = "Maximum total CLTV delta we accept for the route (default: 1008)")]
		max_total_cltv_expiry_delta: Option<u32>,
		#[arg(
			long,
			help = "Maximum number of paths that may be used by MPP payments (default: 10)"
		)]
		max_path_count: Option<u32>,
		#[arg(
			long,
			help = "Maximum share of a channel's total capacity to send over a channel, as a power of 1/2 (default: 2)"
		)]
		max_channel_saturation_power_of_half: Option<u32>,
	},
	#[command(
		about = "Pay a BIP 21 URI, BIP 353 Human-Readable Name, BOLT11 invoice, or BOLT12 offer"
	)]
	Pay {
		#[arg(help = "A BIP 21 URI, BIP 353 Human-Readable Name, BOLT11 invoice, or BOLT12 offer")]
		uri: String,
		#[arg(help = "Amount to send, e.g. 50sat or 50000msat. Required for variable-amount URIs")]
		amount: Option<Amount>,
		#[arg(
			long,
			help = "Maximum total routing fee, e.g. 50sat or 50000msat. Defaults to 1% of payment + 50 sats"
		)]
		max_total_routing_fee: Option<Amount>,
		#[arg(long, help = "Maximum total CLTV delta we accept for the route (default: 1008)")]
		max_total_cltv_expiry_delta: Option<u32>,
		#[arg(
			long,
			help = "Maximum number of paths that may be used by MPP payments (default: 10)"
		)]
		max_path_count: Option<u32>,
		#[arg(
			long,
			help = "Maximum share of a channel's total capacity to send over a channel, as a power of 1/2 (default: 2)"
		)]
		max_channel_saturation_power_of_half: Option<u32>,
	},
	#[command(about = "Decode a BOLT11 invoice and display its fields")]
	DecodeInvoice {
		#[arg(help = "The BOLT11 invoice string to decode")]
		invoice: String,
	},
	#[command(about = "Decode a BOLT12 offer and display its fields")]
	DecodeOffer {
		#[arg(help = "The BOLT12 offer string to decode")]
		offer: String,
	},
	#[command(about = "Cooperatively close the channel specified by the given channel ID")]
	CloseChannel {
		#[arg(help = "The local user_channel_id of this channel")]
		user_channel_id: String,
		#[arg(help = "The hex-encoded public key of the node to close a channel with")]
		counterparty_node_id: String,
	},
	#[command(about = "Force close the channel specified by the given channel ID")]
	ForceCloseChannel {
		#[arg(help = "The local user_channel_id of this channel")]
		user_channel_id: String,
		#[arg(help = "The hex-encoded public key of the node to close a channel with")]
		counterparty_node_id: String,
		#[arg(long, help = "The reason for force-closing, defaults to \"\"")]
		force_close_reason: Option<String>,
	},
	#[command(about = "Create a new outbound channel to the given remote node")]
	OpenChannel {
		#[arg(help = "The hex-encoded public key of the node to open a channel with")]
		node_pubkey: String,
		#[arg(
			help = "Address to connect to remote peer (IPv4:port, IPv6:port, OnionV3:port, or hostname:port)"
		)]
		address: String,
		#[arg(
			help = "The amount to commit to the channel, e.g. 100sat or 100000msat, must be a whole sat amount, cannot send msats on-chain."
		)]
		channel_amount: Amount,
		#[arg(long, help = "Amount to push to the remote side, e.g. 50sat or 50000msat")]
		push_to_counterparty: Option<Amount>,
		#[arg(long, help = "Whether the channel should be public")]
		announce_channel: bool,
		// Channel config options
		#[arg(
			long,
			help = "Amount (in millionths of a satoshi) charged per satoshi for payments forwarded outbound over the channel. This can be updated by using update-channel-config."
		)]
		forwarding_fee_proportional_millionths: Option<u32>,
		#[arg(
			long,
			help = "Amount (in milli-satoshi) charged for payments forwarded outbound over the channel, in excess of forwarding_fee_proportional_millionths. This can be updated by using update-channel-config."
		)]
		forwarding_fee_base_msat: Option<u32>,
		#[arg(
			long,
			help = "The difference in the CLTV value between incoming HTLCs and an outbound HTLC forwarded over the channel. This can be updated by using update-channel-config."
		)]
		cltv_expiry_delta: Option<u32>,
	},
	#[command(
		about = "Increase the channel balance by the given amount, funds will come from the node's on-chain wallet"
	)]
	SpliceIn {
		#[arg(help = "The local user_channel_id of the channel")]
		user_channel_id: String,
		#[arg(help = "The hex-encoded public key of the channel's counterparty node")]
		counterparty_node_id: String,
		#[arg(
			help = "The amount to splice into the channel, e.g. 50sat or 50000msat, must be a whole sat amount, cannot send msats on-chain."
		)]
		splice_amount: Amount,
	},
	#[command(about = "Decrease the channel balance by the given amount")]
	SpliceOut {
		#[arg(help = "The local user_channel_id of this channel")]
		user_channel_id: String,
		#[arg(help = "The hex-encoded public key of the channel's counterparty node")]
		counterparty_node_id: String,
		#[arg(
			help = "The amount to splice out of the channel, e.g. 50sat or 50000msat, must be a whole sat amount, cannot send msats on-chain."
		)]
		splice_amount: Amount,
		#[arg(
			short,
			long,
			help = "Bitcoin address to send the spliced-out funds. If not set, uses the node's on-chain wallet"
		)]
		address: Option<String>,
	},
	#[command(about = "Return a list of known channels")]
	ListChannels,
	#[command(about = "Retrieve list of all payments")]
	ListPayments {
		#[arg(short, long)]
		#[arg(
			help = "Fetch at least this many payments by iterating through multiple pages. Returns combined results with the last page token. If not provided, returns only a single page."
		)]
		number_of_payments: Option<u64>,
		#[arg(long)]
		#[arg(help = "Page token to continue from a previous page (format: token:index)")]
		page_token: Option<String>,
	},
	#[command(about = "Get details of a specific payment by its payment ID")]
	GetPaymentDetails {
		#[arg(help = "The payment ID in hex-encoded form")]
		payment_id: String,
	},
	#[command(about = "Retrieves list of all forwarded payments")]
	ListForwardedPayments {
		#[arg(
			short,
			long,
			help = "Fetch at least this many forwarded payments by iterating through multiple pages. Returns combined results with the last page token. If not provided, returns only a single page."
		)]
		number_of_payments: Option<u64>,
		#[arg(long, help = "Page token to continue from a previous page (format: token:index)")]
		page_token: Option<String>,
	},
	#[command(about = "Update the forwarding fees and CLTV expiry delta for an existing channel")]
	UpdateChannelConfig {
		#[arg(help = "The local user_channel_id of this channel")]
		user_channel_id: String,
		#[arg(
			help = "The hex-encoded public key of the counterparty node to update channel config with"
		)]
		counterparty_node_id: String,
		#[arg(
			long,
			help = "Amount (in millionths of a satoshi) charged per satoshi for payments forwarded outbound over the channel. This can be updated by using update-channel-config."
		)]
		forwarding_fee_proportional_millionths: Option<u32>,
		#[arg(
			long,
			help = "Amount (in milli-satoshi) charged for payments forwarded outbound over the channel, in excess of forwarding_fee_proportional_millionths. This can be updated by using update-channel-config."
		)]
		forwarding_fee_base_msat: Option<u32>,
		#[arg(
			long,
			help = "The difference in the CLTV value between incoming HTLCs and an outbound HTLC forwarded over the channel."
		)]
		cltv_expiry_delta: Option<u32>,
	},
	#[command(about = "Connect to a peer on the Lightning Network without opening a channel")]
	ConnectPeer {
		#[arg(
			help = "The peer to connect to in pubkey@address format, or just the pubkey if address is provided separately"
		)]
		node_pubkey: String,
		#[arg(
			help = "Address to connect to remote peer (IPv4:port, IPv6:port, OnionV3:port, or hostname:port). Optional if address is included in pubkey via @ separator."
		)]
		address: Option<String>,
		#[arg(
			long,
			default_value_t = false,
			help = "Whether to persist the connection for automatic reconnection on restart"
		)]
		persist: bool,
	},
	#[command(about = "Disconnect from a peer and remove it from the peer store")]
	DisconnectPeer {
		#[arg(help = "The hex-encoded public key of the node to disconnect from")]
		node_pubkey: String,
	},
	#[command(about = "Return a list of peers")]
	ListPeers,
	#[command(about = "Sign a message with the node's secret key")]
	SignMessage {
		#[arg(help = "The message to sign")]
		message: String,
	},
	#[command(about = "Verify a signature against a message and public key")]
	VerifySignature {
		#[arg(help = "The message that was signed")]
		message: String,
		#[arg(help = "The zbase32-encoded signature to verify")]
		signature: String,
		#[arg(help = "The hex-encoded public key of the signer")]
		public_key: String,
	},
	#[command(about = "Export the pathfinding scores used by the router")]
	ExportPathfindingScores,
	#[command(about = "List all known short channel IDs in the network graph")]
	GraphListChannels,
	#[command(about = "Get channel information from the network graph by short channel ID")]
	GraphGetChannel {
		#[arg(help = "The short channel ID to look up")]
		short_channel_id: u64,
	},
	#[command(about = "List all known node IDs in the network graph")]
	GraphListNodes,
	#[command(about = "Get node information from the network graph by node ID")]
	GraphGetNode {
		#[arg(help = "The hex-encoded node ID to look up")]
		node_id: String,
	},
	#[command(about = "Generate shell completions for the CLI")]
	Completions {
		#[arg(
			value_enum,
			help = "The shell to generate completions for (bash, zsh, fish, powershell, elvish)"
		)]
		shell: Shell,
	},
}

#[tokio::main]
async fn main() {
	let cli = Cli::parse();

	// short-circuit if generating completions
	if let Commands::Completions { shell } = cli.command {
		generate(shell, &mut Cli::command(), "ldk-server-cli", &mut std::io::stdout());
		return;
	}

	let config_path = cli.config.map(PathBuf::from).or_else(get_default_config_path);
	let config = config_path.as_ref().and_then(|p| load_config(p).ok());
	let storage_dir =
		config.as_ref().and_then(|c| c.storage.as_ref()?.disk.as_ref()?.dir_path.as_deref());

	// Get API key from argument, then from api_key file in storage dir, then from default location
	let api_key = cli
		.api_key
		.or_else(|| {
			let network =
				config.as_ref().and_then(|c| c.network().ok()).unwrap_or("bitcoin".to_string());
			storage_dir
				.map(|dir| api_key_path_for_storage_dir(dir, &network))
				.and_then(|path| std::fs::read(&path).ok())
				.or_else(|| {
					get_default_api_key_path(&network)
						.and_then(|path| std::fs::read(&path).ok())
				})
				.map(|bytes| bytes.to_lower_hex_string())
		})
		.unwrap_or_else(|| {
			eprintln!("API key not provided. Use --api-key or ensure the api_key file exists at {DEFAULT_DIR}/[network]/api_key");
			std::process::exit(1);
		});

	// Get base URL from argument then from config file
	let base_url = resolve_base_url(cli.base_url, config.as_ref());

	// Get TLS cert path from argument, then from config tls.cert_path, then from storage dir,
	// then try default location.
	let tls_cert_path = cli.tls_cert.map(PathBuf::from).or_else(|| {
		config
			.as_ref()
			.and_then(|c| c.tls.as_ref().and_then(|t| t.cert_path.as_ref().map(PathBuf::from)))
			.or_else(|| {
				storage_dir.map(cert_path_for_storage_dir).filter(|path| path.exists())
			})
			.or_else(get_default_cert_path)
	})
		.unwrap_or_else(|| {
			eprintln!("TLS cert path not provided. Use --tls-cert or ensure config file exists at {DEFAULT_DIR}/config.toml");
			std::process::exit(1);
		});

	let server_cert_pem = std::fs::read(&tls_cert_path).unwrap_or_else(|e| {
		eprintln!("Failed to read server certificate file '{}': {}", tls_cert_path.display(), e);
		std::process::exit(1);
	});

	let client =
		LdkServerClient::new(base_url, api_key, &server_cert_pem).await.unwrap_or_else(|e| {
			eprintln!("Failed to create client: {e}");
			std::process::exit(1);
		});

	match cli.command {
		Commands::GetNodeInfo => {
			handle_response_result::<_, GetNodeInfoResponse>(
				client.get_node_info(GetNodeInfoRequest {}).await,
			);
		},
		Commands::GetBalances => {
			handle_response_result::<_, GetBalancesResponse>(
				client.get_balances(GetBalancesRequest {}).await,
			);
		},
		Commands::OnchainReceive => {
			handle_response_result::<_, OnchainReceiveResponse>(
				client.onchain_receive(OnchainReceiveRequest {}).await,
			);
		},
		Commands::OnchainSend { address, amount, send_all, fee_rate_sat_per_vb } => {
			let amount_sats = amount.map(|a| a.to_sat().unwrap_or_else(|e| handle_error_msg(&e)));
			handle_response_result::<_, OnchainSendResponse>(
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
		Commands::Bolt11Receive { description, description_hash, expiry_secs, amount } => {
			let amount_msat = amount.map(|a| a.to_msat());
			let invoice_description =
				parse_bolt11_invoice_description(description, description_hash);

			let expiry_secs = expiry_secs.unwrap_or(DEFAULT_EXPIRY_SECS);
			let request =
				Bolt11ReceiveRequest { description: invoice_description, expiry_secs, amount_msat };

			handle_response_result::<_, Bolt11ReceiveResponse>(
				client.bolt11_receive(request).await,
			);
		},
		Commands::Bolt11ReceiveForHash {
			payment_hash,
			amount,
			description,
			description_hash,
			expiry_secs,
		} => {
			let amount_msat = amount.map(|a| a.to_msat());
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

			let expiry_secs = expiry_secs.unwrap_or(DEFAULT_EXPIRY_SECS);
			let request = Bolt11ReceiveForHashRequest {
				description: invoice_description,
				expiry_secs,
				amount_msat,
				payment_hash,
			};

			handle_response_result::<_, Bolt11ReceiveForHashResponse>(
				client.bolt11_receive_for_hash(request).await,
			);
		},
		Commands::Bolt11ClaimForHash { preimage, claimable_amount, payment_hash } => {
			handle_response_result::<_, Bolt11ClaimForHashResponse>(
				client
					.bolt11_claim_for_hash(Bolt11ClaimForHashRequest {
						payment_hash,
						claimable_amount_msat: claimable_amount.map(|a| a.to_msat()),
						preimage,
					})
					.await,
			);
		},
		Commands::Bolt11FailForHash { payment_hash } => {
			handle_response_result::<_, Bolt11FailForHashResponse>(
				client.bolt11_fail_for_hash(Bolt11FailForHashRequest { payment_hash }).await,
			);
		},
		Commands::Bolt11ReceiveViaJitChannel {
			amount,
			description,
			description_hash,
			expiry_secs,
			max_total_lsp_fee_limit,
		} => {
			let request = Bolt11ReceiveViaJitChannelRequest {
				amount_msat: amount.to_msat(),
				description: parse_bolt11_invoice_description(description, description_hash),
				expiry_secs: expiry_secs.unwrap_or(DEFAULT_EXPIRY_SECS),
				max_total_lsp_fee_limit_msat: max_total_lsp_fee_limit.map(|a| a.to_msat()),
			};

			handle_response_result::<_, Bolt11ReceiveViaJitChannelResponse>(
				client.bolt11_receive_via_jit_channel(request).await,
			);
		},
		Commands::Bolt11ReceiveVariableAmountViaJitChannel {
			description,
			description_hash,
			expiry_secs,
			max_proportional_lsp_fee_limit_ppm_msat,
		} => {
			let request = Bolt11ReceiveVariableAmountViaJitChannelRequest {
				description: parse_bolt11_invoice_description(description, description_hash),
				expiry_secs: expiry_secs.unwrap_or(DEFAULT_EXPIRY_SECS),
				max_proportional_lsp_fee_limit_ppm_msat,
			};

			handle_response_result::<_, Bolt11ReceiveVariableAmountViaJitChannelResponse>(
				client.bolt11_receive_variable_amount_via_jit_channel(request).await,
			);
		},
		Commands::Bolt11Send {
			invoice,
			amount,
			max_total_routing_fee,
			max_total_cltv_expiry_delta,
			max_path_count,
			max_channel_saturation_power_of_half,
		} => {
			let amount_msat = amount.map(|a| a.to_msat());
			let max_total_routing_fee_msat = max_total_routing_fee.map(|a| a.to_msat());
			let route_parameters = RouteParametersConfig {
				max_total_routing_fee_msat,
				max_total_cltv_expiry_delta: max_total_cltv_expiry_delta
					.unwrap_or(DEFAULT_MAX_TOTAL_CLTV_EXPIRY_DELTA),
				max_path_count: max_path_count.unwrap_or(DEFAULT_MAX_PATH_COUNT),
				max_channel_saturation_power_of_half: max_channel_saturation_power_of_half
					.unwrap_or(DEFAULT_MAX_CHANNEL_SATURATION_POWER_OF_HALF),
			};
			handle_response_result::<_, Bolt11SendResponse>(
				client
					.bolt11_send(Bolt11SendRequest {
						invoice,
						amount_msat,
						route_parameters: Some(route_parameters),
					})
					.await,
			);
		},
		Commands::Bolt12Receive { description, amount, expiry_secs, quantity } => {
			let amount_msat = amount.map(|a| a.to_msat());
			handle_response_result::<_, Bolt12ReceiveResponse>(
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
		Commands::Bolt12Send {
			offer,
			amount,
			quantity,
			payer_note,
			max_total_routing_fee,
			max_total_cltv_expiry_delta,
			max_path_count,
			max_channel_saturation_power_of_half,
		} => {
			let amount_msat = amount.map(|a| a.to_msat());
			let max_total_routing_fee_msat = max_total_routing_fee.map(|a| a.to_msat());
			let route_parameters = RouteParametersConfig {
				max_total_routing_fee_msat,
				max_total_cltv_expiry_delta: max_total_cltv_expiry_delta
					.unwrap_or(DEFAULT_MAX_TOTAL_CLTV_EXPIRY_DELTA),
				max_path_count: max_path_count.unwrap_or(DEFAULT_MAX_PATH_COUNT),
				max_channel_saturation_power_of_half: max_channel_saturation_power_of_half
					.unwrap_or(DEFAULT_MAX_CHANNEL_SATURATION_POWER_OF_HALF),
			};

			handle_response_result::<_, Bolt12SendResponse>(
				client
					.bolt12_send(Bolt12SendRequest {
						offer,
						amount_msat,
						quantity,
						payer_note,
						route_parameters: Some(route_parameters),
					})
					.await,
			);
		},
		Commands::SpontaneousSend {
			node_id,
			amount,
			max_total_routing_fee,
			max_total_cltv_expiry_delta,
			max_path_count,
			max_channel_saturation_power_of_half,
		} => {
			let amount_msat = amount.to_msat();
			let max_total_routing_fee_msat = max_total_routing_fee.map(|a| a.to_msat());
			let route_parameters = RouteParametersConfig {
				max_total_routing_fee_msat,
				max_total_cltv_expiry_delta: max_total_cltv_expiry_delta
					.unwrap_or(DEFAULT_MAX_TOTAL_CLTV_EXPIRY_DELTA),
				max_path_count: max_path_count.unwrap_or(DEFAULT_MAX_PATH_COUNT),
				max_channel_saturation_power_of_half: max_channel_saturation_power_of_half
					.unwrap_or(DEFAULT_MAX_CHANNEL_SATURATION_POWER_OF_HALF),
			};

			handle_response_result::<_, SpontaneousSendResponse>(
				client
					.spontaneous_send(SpontaneousSendRequest {
						amount_msat,
						node_id,
						route_parameters: Some(route_parameters),
					})
					.await,
			);
		},
		Commands::Pay {
			uri,
			amount,
			max_total_routing_fee,
			max_total_cltv_expiry_delta,
			max_path_count,
			max_channel_saturation_power_of_half,
		} => {
			let amount_msat = amount.map(|a| a.to_msat());
			let max_total_routing_fee_msat = max_total_routing_fee.map(|a| a.to_msat());
			let route_parameters = RouteParametersConfig {
				max_total_routing_fee_msat,
				max_total_cltv_expiry_delta: max_total_cltv_expiry_delta
					.unwrap_or(DEFAULT_MAX_TOTAL_CLTV_EXPIRY_DELTA),
				max_path_count: max_path_count.unwrap_or(DEFAULT_MAX_PATH_COUNT),
				max_channel_saturation_power_of_half: max_channel_saturation_power_of_half
					.unwrap_or(DEFAULT_MAX_CHANNEL_SATURATION_POWER_OF_HALF),
			};
			handle_response_result::<_, UnifiedSendResponse>(
				client
					.unified_send(UnifiedSendRequest {
						uri,
						amount_msat,
						route_parameters: Some(route_parameters),
					})
					.await,
			);
		},
		Commands::DecodeInvoice { invoice } => {
			handle_response_result::<_, DecodeInvoiceResponse>(
				client.decode_invoice(DecodeInvoiceRequest { invoice }).await,
			);
		},
		Commands::DecodeOffer { offer } => {
			handle_response_result::<_, DecodeOfferResponse>(
				client.decode_offer(DecodeOfferRequest { offer }).await,
			);
		},
		Commands::CloseChannel { user_channel_id, counterparty_node_id } => {
			handle_response_result::<_, CloseChannelResponse>(
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
			handle_response_result::<_, ForceCloseChannelResponse>(
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
			channel_amount,
			push_to_counterparty,
			announce_channel,
			forwarding_fee_proportional_millionths,
			forwarding_fee_base_msat,
			cltv_expiry_delta,
		} => {
			let channel_amount_sats =
				channel_amount.to_sat().unwrap_or_else(|e| handle_error_msg(&e));
			let push_to_counterparty_msat = push_to_counterparty.map(|a| a.to_msat());
			let channel_config = build_open_channel_config(
				forwarding_fee_proportional_millionths,
				forwarding_fee_base_msat,
				cltv_expiry_delta,
			);

			handle_response_result::<_, OpenChannelResponse>(
				client
					.open_channel(OpenChannelRequest {
						node_pubkey,
						address,
						channel_amount_sats,
						push_to_counterparty_msat,
						channel_config,
						announce_channel,
					})
					.await,
			);
		},
		Commands::SpliceIn { user_channel_id, counterparty_node_id, splice_amount } => {
			let splice_amount_sats =
				splice_amount.to_sat().unwrap_or_else(|e| handle_error_msg(&e));
			handle_response_result::<_, SpliceInResponse>(
				client
					.splice_in(SpliceInRequest {
						user_channel_id,
						counterparty_node_id,
						splice_amount_sats,
					})
					.await,
			);
		},
		Commands::SpliceOut { user_channel_id, counterparty_node_id, address, splice_amount } => {
			let splice_amount_sats =
				splice_amount.to_sat().unwrap_or_else(|e| handle_error_msg(&e));
			handle_response_result::<_, SpliceOutResponse>(
				client
					.splice_out(SpliceOutRequest {
						user_channel_id,
						counterparty_node_id,
						address,
						splice_amount_sats,
					})
					.await,
			);
		},
		Commands::ListChannels => {
			handle_response_result::<_, ListChannelsResponse>(
				client.list_channels(ListChannelsRequest {}).await,
			);
		},
		Commands::ListPayments { number_of_payments, page_token } => {
			let page_token = page_token
				.map(|token_str| parse_page_token(&token_str).unwrap_or_else(|e| handle_error(e)));

			handle_response_result::<_, CliListPaymentsResponse>(
				fetch_paginated(
					number_of_payments,
					page_token,
					|pt| client.list_payments(ListPaymentsRequest { page_token: pt }),
					|r| (r.payments, r.next_page_token),
				)
				.await,
			);
		},
		Commands::GetPaymentDetails { payment_id } => {
			handle_response_result::<_, GetPaymentDetailsResponse>(
				client.get_payment_details(GetPaymentDetailsRequest { payment_id }).await,
			);
		},
		Commands::ListForwardedPayments { number_of_payments, page_token } => {
			let page_token = page_token
				.map(|token_str| parse_page_token(&token_str).unwrap_or_else(|e| handle_error(e)));

			handle_response_result::<_, CliListForwardedPaymentsResponse>(
				fetch_paginated(
					number_of_payments,
					page_token,
					|pt| {
						client.list_forwarded_payments(ListForwardedPaymentsRequest {
							page_token: pt,
						})
					},
					|r| (r.forwarded_payments, r.next_page_token),
				)
				.await,
			);
		},
		Commands::UpdateChannelConfig {
			user_channel_id,
			counterparty_node_id,
			forwarding_fee_proportional_millionths,
			forwarding_fee_base_msat,
			cltv_expiry_delta,
		} => {
			let channel_config = ChannelConfig {
				forwarding_fee_proportional_millionths,
				forwarding_fee_base_msat,
				cltv_expiry_delta,
				force_close_avoidance_max_fee_satoshis: None,
				accept_underpaying_htlcs: None,
				max_dust_htlc_exposure: None,
			};

			handle_response_result::<_, UpdateChannelConfigResponse>(
				client
					.update_channel_config(UpdateChannelConfigRequest {
						user_channel_id,
						counterparty_node_id,
						channel_config: Some(channel_config),
					})
					.await,
			);
		},
		Commands::ConnectPeer { node_pubkey, address, persist } => {
			let (node_pubkey, address) = if let Some(address) = address {
				(node_pubkey, address)
			} else if let Some((pubkey, addr)) = node_pubkey.split_once('@') {
				(pubkey.to_string(), addr.to_string())
			} else {
				eprintln!("Error: address is required. Provide it as pubkey@address or as a separate argument.");
				std::process::exit(1);
			};
			handle_response_result::<_, ConnectPeerResponse>(
				client.connect_peer(ConnectPeerRequest { node_pubkey, address, persist }).await,
			);
		},
		Commands::DisconnectPeer { node_pubkey } => {
			handle_response_result::<_, DisconnectPeerResponse>(
				client.disconnect_peer(DisconnectPeerRequest { node_pubkey }).await,
			);
		},
		Commands::ListPeers => {
			handle_response_result::<_, ListPeersResponse>(
				client.list_peers(ListPeersRequest {}).await,
			);
		},
		Commands::SignMessage { message } => {
			handle_response_result::<_, SignMessageResponse>(
				client
					.sign_message(SignMessageRequest { message: message.into_bytes().into() })
					.await,
			);
		},
		Commands::VerifySignature { message, signature, public_key } => {
			handle_response_result::<_, VerifySignatureResponse>(
				client
					.verify_signature(VerifySignatureRequest {
						message: message.into_bytes().into(),
						signature,
						public_key,
					})
					.await,
			);
		},
		Commands::ExportPathfindingScores => {
			handle_response_result::<_, Value>(
				client.export_pathfinding_scores(ExportPathfindingScoresRequest {}).await.map(
					|s| {
						let scores_hex = s.scores.as_hex().to_string();
						json!({ "pathfinding_scores": scores_hex })
					},
				),
			);
		},
		Commands::GraphListChannels => {
			handle_response_result::<_, GraphListChannelsResponse>(
				client.graph_list_channels(GraphListChannelsRequest {}).await,
			);
		},
		Commands::GraphGetChannel { short_channel_id } => {
			handle_response_result::<_, GraphGetChannelResponse>(
				client.graph_get_channel(GraphGetChannelRequest { short_channel_id }).await,
			);
		},
		Commands::GraphListNodes => {
			handle_response_result::<_, GraphListNodesResponse>(
				client.graph_list_nodes(GraphListNodesRequest {}).await,
			);
		},
		Commands::GraphGetNode { node_id } => {
			handle_response_result::<_, GraphGetNodeResponse>(
				client.graph_get_node(GraphGetNodeRequest { node_id }).await,
			);
		},
		Commands::Completions { .. } => unreachable!("Handled above"),
	}
}

fn build_open_channel_config(
	forwarding_fee_proportional_millionths: Option<u32>, forwarding_fee_base_msat: Option<u32>,
	cltv_expiry_delta: Option<u32>,
) -> Option<ChannelConfig> {
	// Only create a config if at least one field is set
	if forwarding_fee_proportional_millionths.is_none()
		&& forwarding_fee_base_msat.is_none()
		&& cltv_expiry_delta.is_none()
	{
		return None;
	}

	Some(ChannelConfig {
		forwarding_fee_proportional_millionths,
		forwarding_fee_base_msat,
		cltv_expiry_delta,
		force_close_avoidance_max_fee_satoshis: None,
		accept_underpaying_htlcs: None,
		max_dust_htlc_exposure: None,
	})
}

async fn fetch_paginated<T, R, Fut>(
	target_count: Option<u64>, initial_page_token: Option<PageToken>,
	fetch_page: impl Fn(Option<PageToken>) -> Fut,
	extract: impl Fn(R) -> (Vec<T>, Option<PageToken>),
) -> Result<CliPaginatedResponse<T>, LdkServerError>
where
	Fut: std::future::Future<Output = Result<R, LdkServerError>>,
{
	match target_count {
		Some(count) => {
			let mut items = Vec::with_capacity(count as usize);
			let mut page_token = initial_page_token;
			let mut next_page_token;

			loop {
				let response = fetch_page(page_token).await?;
				let (new_items, new_next_page_token) = extract(response);
				items.extend(new_items);
				next_page_token = new_next_page_token;

				if items.len() >= count as usize || next_page_token.is_none() {
					break;
				}
				page_token = next_page_token;
			}

			Ok(CliPaginatedResponse::new(items, next_page_token))
		},
		None => {
			let response = fetch_page(initial_page_token).await?;
			let (items, next_page_token) = extract(response);
			Ok(CliPaginatedResponse::new(items, next_page_token))
		},
	}
}

/// Escapes Unicode bidirectional control characters as `\uXXXX` so they are visible
/// in terminal output rather than silently reordering displayed text.
/// serde_json already escapes ASCII control characters (U+0000–U+001F), but bidi
/// overrides (U+200E–U+2069) pass through unescaped.
fn sanitize_for_terminal(s: String) -> String {
	fn is_bidi_control(c: char) -> bool {
		matches!(
			c,
			'\u{200E}' // LEFT-TO-RIGHT MARK
			| '\u{200F}' // RIGHT-TO-LEFT MARK
			| '\u{202A}' // LEFT-TO-RIGHT EMBEDDING
			| '\u{202B}' // RIGHT-TO-LEFT EMBEDDING
			| '\u{202C}' // POP DIRECTIONAL FORMATTING
			| '\u{202D}' // LEFT-TO-RIGHT OVERRIDE
			| '\u{202E}' // RIGHT-TO-LEFT OVERRIDE
			| '\u{2066}' // LEFT-TO-RIGHT ISOLATE
			| '\u{2067}' // RIGHT-TO-LEFT ISOLATE
			| '\u{2068}' // FIRST STRONG ISOLATE
			| '\u{2069}' // POP DIRECTIONAL ISOLATE
		)
	}
	if !s.chars().any(is_bidi_control) {
		return s;
	}
	let mut out = String::with_capacity(s.len());
	for c in s.chars() {
		if is_bidi_control(c) {
			write!(out, "\\u{:04X}", c as u32).unwrap();
		} else {
			out.push(c);
		}
	}
	out
}

fn handle_response_result<Rs, Js>(response: Result<Rs, LdkServerError>)
where
	Rs: Into<Js>,
	Js: Serialize + std::fmt::Debug,
{
	match response {
		Ok(response) => {
			let json_response: Js = response.into();
			match serde_json::to_string_pretty(&json_response) {
				Ok(json) => println!("{}", sanitize_for_terminal(json)),
				Err(e) => {
					eprintln!("Error serializing response ({json_response:?}) to JSON: {e}");
					std::process::exit(1);
				},
			}
		},
		Err(e) => {
			handle_error(e);
		},
	}
}

fn parse_bolt11_invoice_description(
	description: Option<String>, description_hash: Option<String>,
) -> Option<Bolt11InvoiceDescription> {
	match (description, description_hash) {
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
	}
}

fn parse_page_token(token_str: &str) -> Result<PageToken, LdkServerError> {
	let parts: Vec<&str> = token_str.split(':').collect();
	if parts.len() != 2 {
		return Err(LdkServerError::new(
			InternalError,
			"Page token must be in format 'token:index'".to_string(),
		));
	}
	let index = parts[1]
		.parse::<i64>()
		.map_err(|_| LdkServerError::new(InternalError, "Invalid page token index".to_string()))?;
	Ok(PageToken { token: parts[0].to_string(), index })
}

fn handle_error_msg(msg: &str) -> ! {
	eprintln!("Error: {msg}");
	std::process::exit(1);
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
