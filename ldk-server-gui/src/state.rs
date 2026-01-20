use std::sync::Arc;
use std::time::Instant;
use tokio::task::JoinHandle;

use crate::config::{ChainSourceConfig, ChainSourceType};
use ldk_server_client::client::LdkServerClient;
use ldk_server_client::ldk_server_protos::api::{
    Bolt11ReceiveResponse, Bolt11SendResponse, Bolt12ReceiveResponse, Bolt12SendResponse,
    CloseChannelResponse, ConnectPeerResponse, ForceCloseChannelResponse, GetBalancesResponse,
    GetNodeInfoResponse, ListChannelsResponse, ListPaymentsResponse, OnchainReceiveResponse,
    OnchainSendResponse, OpenChannelResponse, SpliceInResponse, SpliceOutResponse,
    UpdateChannelConfigResponse,
};
use ldk_server_client::ldk_server_protos::types::PageToken;

pub type AsyncTaskResult<T> = Result<T, String>;

#[derive(Clone, PartialEq, Default)]
pub enum ConnectionStatus {
    #[default]
    Disconnected,
    Connected,
    Error(String),
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum ActiveTab {
    #[default]
    NodeInfo,
    Balances,
    Channels,
    Payments,
    Lightning,
    Onchain,
}

#[derive(Default, Clone)]
pub struct OpenChannelForm {
    pub node_pubkey: String,
    pub address: String,
    pub channel_amount_sats: String,
    pub push_to_counterparty_msat: String,
    pub announce_channel: bool,
    pub forwarding_fee_proportional_millionths: String,
    pub forwarding_fee_base_msat: String,
    pub cltv_expiry_delta: String,
}

#[derive(Default, Clone)]
pub struct Bolt11ReceiveForm {
    pub amount_msat: String,
    pub description: String,
    pub expiry_secs: String,
}

#[derive(Default, Clone)]
pub struct Bolt11SendForm {
    pub invoice: String,
    pub amount_msat: String,
}

#[derive(Default, Clone)]
pub struct Bolt12ReceiveForm {
    pub description: String,
    pub amount_msat: String,
    pub expiry_secs: String,
    pub quantity: String,
}

#[derive(Default, Clone)]
pub struct Bolt12SendForm {
    pub offer: String,
    pub amount_msat: String,
    pub quantity: String,
    pub payer_note: String,
}

#[derive(Default, Clone)]
pub struct OnchainSendForm {
    pub address: String,
    pub amount_sats: String,
    pub send_all: bool,
    pub fee_rate_sat_per_vb: String,
}

#[derive(Default, Clone)]
pub struct SpliceForm {
    pub user_channel_id: String,
    pub counterparty_node_id: String,
    pub splice_amount_sats: String,
    pub address: String,
}

#[derive(Default, Clone)]
pub struct UpdateChannelConfigForm {
    pub user_channel_id: String,
    pub counterparty_node_id: String,
    pub forwarding_fee_proportional_millionths: String,
    pub forwarding_fee_base_msat: String,
    pub cltv_expiry_delta: String,
}

#[derive(Default, Clone)]
pub struct CloseChannelForm {
    pub user_channel_id: String,
    pub counterparty_node_id: String,
    pub force_close_reason: String,
}

#[derive(Default, Clone)]
pub struct ConnectPeerForm {
    pub node_pubkey: String,
    pub address: String,
    pub persist: bool,
}

/// Editable chain source configuration
#[derive(Default, Clone)]
pub struct ChainSourceForm {
    pub source_type: ChainSourceType,
    // Bitcoind fields
    pub btc_rpc_address: String,
    pub btc_rpc_user: String,
    pub btc_rpc_password: String,
    // Electrum/Esplora field
    pub server_url: String,
}

impl ChainSourceForm {
    pub fn from_config(config: &ChainSourceConfig) -> Self {
        match config {
            ChainSourceConfig::None => Self::default(),
            ChainSourceConfig::Bitcoind { rpc_address, rpc_user, rpc_password } => Self {
                source_type: ChainSourceType::Bitcoind,
                btc_rpc_address: rpc_address.clone(),
                btc_rpc_user: rpc_user.clone(),
                btc_rpc_password: rpc_password.clone(),
                server_url: String::new(),
            },
            ChainSourceConfig::Electrum { server_url } => Self {
                source_type: ChainSourceType::Electrum,
                server_url: server_url.clone(),
                ..Default::default()
            },
            ChainSourceConfig::Esplora { server_url } => Self {
                source_type: ChainSourceType::Esplora,
                server_url: server_url.clone(),
                ..Default::default()
            },
        }
    }

    pub fn to_config(&self) -> ChainSourceConfig {
        match self.source_type {
            ChainSourceType::None => ChainSourceConfig::None,
            ChainSourceType::Bitcoind => ChainSourceConfig::Bitcoind {
                rpc_address: self.btc_rpc_address.clone(),
                rpc_user: self.btc_rpc_user.clone(),
                rpc_password: self.btc_rpc_password.clone(),
            },
            ChainSourceType::Electrum => ChainSourceConfig::Electrum {
                server_url: self.server_url.clone(),
            },
            ChainSourceType::Esplora => ChainSourceConfig::Esplora {
                server_url: self.server_url.clone(),
            },
        }
    }
}

#[derive(Default, Clone)]
pub struct Forms {
    pub open_channel: OpenChannelForm,
    pub bolt11_receive: Bolt11ReceiveForm,
    pub bolt11_send: Bolt11SendForm,
    pub bolt12_receive: Bolt12ReceiveForm,
    pub bolt12_send: Bolt12SendForm,
    pub onchain_send: OnchainSendForm,
    pub splice_in: SpliceForm,
    pub splice_out: SpliceForm,
    pub update_channel_config: UpdateChannelConfigForm,
    pub close_channel: CloseChannelForm,
    pub connect_peer: ConnectPeerForm,
    pub chain_source: ChainSourceForm,
}

pub struct StatusMessage {
    pub text: String,
    pub is_error: bool,
    pub timestamp: Instant,
}

impl StatusMessage {
    pub fn success(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: false,
            timestamp: Instant::now(),
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: true,
            timestamp: Instant::now(),
        }
    }
}

pub struct AsyncTasks {
    pub node_info: Option<JoinHandle<AsyncTaskResult<GetNodeInfoResponse>>>,
    pub balances: Option<JoinHandle<AsyncTaskResult<GetBalancesResponse>>>,
    pub channels: Option<JoinHandle<AsyncTaskResult<ListChannelsResponse>>>,
    pub payments: Option<JoinHandle<AsyncTaskResult<ListPaymentsResponse>>>,
    pub onchain_receive: Option<JoinHandle<AsyncTaskResult<OnchainReceiveResponse>>>,
    pub onchain_send: Option<JoinHandle<AsyncTaskResult<OnchainSendResponse>>>,
    pub bolt11_receive: Option<JoinHandle<AsyncTaskResult<Bolt11ReceiveResponse>>>,
    pub bolt11_send: Option<JoinHandle<AsyncTaskResult<Bolt11SendResponse>>>,
    pub bolt12_receive: Option<JoinHandle<AsyncTaskResult<Bolt12ReceiveResponse>>>,
    pub bolt12_send: Option<JoinHandle<AsyncTaskResult<Bolt12SendResponse>>>,
    pub open_channel: Option<JoinHandle<AsyncTaskResult<OpenChannelResponse>>>,
    pub close_channel: Option<JoinHandle<AsyncTaskResult<CloseChannelResponse>>>,
    pub force_close_channel: Option<JoinHandle<AsyncTaskResult<ForceCloseChannelResponse>>>,
    pub splice_in: Option<JoinHandle<AsyncTaskResult<SpliceInResponse>>>,
    pub splice_out: Option<JoinHandle<AsyncTaskResult<SpliceOutResponse>>>,
    pub update_channel_config: Option<JoinHandle<AsyncTaskResult<UpdateChannelConfigResponse>>>,
    pub connect_peer: Option<JoinHandle<AsyncTaskResult<ConnectPeerResponse>>>,
}

impl Default for AsyncTasks {
    fn default() -> Self {
        Self {
            node_info: None,
            balances: None,
            channels: None,
            payments: None,
            onchain_receive: None,
            onchain_send: None,
            bolt11_receive: None,
            bolt11_send: None,
            bolt12_receive: None,
            bolt12_send: None,
            open_channel: None,
            close_channel: None,
            force_close_channel: None,
            splice_in: None,
            splice_out: None,
            update_channel_config: None,
            connect_peer: None,
        }
    }
}

impl AsyncTasks {
    pub fn any_pending(&self) -> bool {
        self.node_info.is_some()
            || self.balances.is_some()
            || self.channels.is_some()
            || self.payments.is_some()
            || self.onchain_receive.is_some()
            || self.onchain_send.is_some()
            || self.bolt11_receive.is_some()
            || self.bolt11_send.is_some()
            || self.bolt12_receive.is_some()
            || self.bolt12_send.is_some()
            || self.open_channel.is_some()
            || self.close_channel.is_some()
            || self.force_close_channel.is_some()
            || self.splice_in.is_some()
            || self.splice_out.is_some()
            || self.update_channel_config.is_some()
            || self.connect_peer.is_some()
    }
}

pub struct AppState {
    // Connection settings
    pub server_url: String,
    pub api_key: String,
    pub tls_cert_path: String,
    pub connection_status: ConnectionStatus,
    pub client: Option<Arc<LdkServerClient>>,

    // Config info (from loaded config file)
    pub config_file_path: Option<String>,
    pub network: String,
    pub chain_source: ChainSourceConfig,

    // Navigation
    pub active_tab: ActiveTab,

    // Cached API responses
    pub node_info: Option<GetNodeInfoResponse>,
    pub balances: Option<GetBalancesResponse>,
    pub channels: Option<ListChannelsResponse>,
    pub payments: Option<ListPaymentsResponse>,
    pub payments_page_token: Option<PageToken>,

    // Operation results
    pub onchain_address: Option<String>,
    pub generated_invoice: Option<String>,
    pub generated_offer: Option<String>,
    pub last_payment_id: Option<String>,
    pub last_txid: Option<String>,
    pub last_channel_id: Option<String>,

    // Async tasks
    pub tasks: AsyncTasks,

    // Form state
    pub forms: Forms,

    // UI state
    pub status_message: Option<StatusMessage>,
    pub show_open_channel_dialog: bool,
    pub show_close_channel_dialog: bool,
    pub show_splice_in_dialog: bool,
    pub show_splice_out_dialog: bool,
    pub show_update_config_dialog: bool,
    pub show_connect_peer_dialog: bool,
    pub lightning_tab: LightningTab,
    pub onchain_tab: OnchainTab,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum LightningTab {
    #[default]
    Bolt11Send,
    Bolt11Receive,
    Bolt12Send,
    Bolt12Receive,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum OnchainTab {
    #[default]
    Send,
    Receive,
    History,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            server_url: "localhost:3002".into(),
            api_key: String::new(),
            tls_cert_path: String::new(),
            connection_status: ConnectionStatus::Disconnected,
            client: None,

            config_file_path: None,
            network: String::new(),
            chain_source: ChainSourceConfig::default(),

            active_tab: ActiveTab::NodeInfo,

            node_info: None,
            balances: None,
            channels: None,
            payments: None,
            payments_page_token: None,

            onchain_address: None,
            generated_invoice: None,
            generated_offer: None,
            last_payment_id: None,
            last_txid: None,
            last_channel_id: None,

            tasks: AsyncTasks::default(),

            forms: Forms::default(),

            status_message: None,
            show_open_channel_dialog: false,
            show_close_channel_dialog: false,
            show_splice_in_dialog: false,
            show_splice_out_dialog: false,
            show_update_config_dialog: false,
            show_connect_peer_dialog: false,
            lightning_tab: LightningTab::default(),
            onchain_tab: OnchainTab::default(),
        }
    }
}
