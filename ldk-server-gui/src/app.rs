use std::sync::Arc;
use std::time::Duration;

use eframe::{egui, App, Frame};
use futures_util::FutureExt;
use tokio::runtime::Runtime;

use ldk_server_client::client::LdkServerClient;
use ldk_server_client::ldk_server_protos::api::{
    Bolt11ReceiveRequest, Bolt11SendRequest, Bolt12ReceiveRequest, Bolt12SendRequest,
    CloseChannelRequest, ConnectPeerRequest, ForceCloseChannelRequest, GetBalancesRequest,
    GetNodeInfoRequest, ListChannelsRequest, ListPaymentsRequest, OnchainReceiveRequest,
    OnchainSendRequest, OpenChannelRequest, SpliceInRequest, SpliceOutRequest,
    UpdateChannelConfigRequest,
};
use ldk_server_client::ldk_server_protos::types::{
    bolt11_invoice_description, Bolt11InvoiceDescription, ChannelConfig,
};

use crate::config;
use crate::state::{ActiveTab, AppState, ConnectionStatus, StatusMessage};
use crate::ui;

pub struct LdkServerApp {
    pub state: AppState,
    pub rt: Runtime,
}

impl LdkServerApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut state = AppState::default();

        // Try to load config from file and populate connection settings
        if let Some(gui_config) = config::find_and_load_config() {
            state.server_url = gui_config.server_url;
            state.api_key = gui_config.api_key;
            state.tls_cert_path = gui_config.tls_cert_path;
            state.network = gui_config.network;
            state.chain_source = gui_config.chain_source;
            state.status_message =
                Some(StatusMessage::success("Config loaded from ldk-server-config.toml"));
        }

        Self { state, rt: Runtime::new().expect("Failed to create tokio runtime") }
    }

    pub fn connect(&mut self) {
        let url = self.state.server_url.trim().to_string();
        let api_key = self.state.api_key.clone();
        let cert_path = self.state.tls_cert_path.trim().to_string();

        if url.is_empty() || api_key.is_empty() || cert_path.is_empty() {
            self.state.status_message =
                Some(StatusMessage::error("Please fill in all connection fields"));
            return;
        }

        let cert_data = match std::fs::read(&cert_path) {
            Ok(data) => data,
            Err(e) => {
                self.state.status_message =
                    Some(StatusMessage::error(format!("Failed to read TLS cert: {}", e)));
                return;
            }
        };

        match LdkServerClient::new(url.clone(), api_key, &cert_data) {
            Ok(client) => {
                self.state.client = Some(Arc::new(client));
                self.state.connection_status = ConnectionStatus::Connected;
                self.state.status_message = Some(StatusMessage::success("Connected"));
                self.fetch_node_info();
                self.fetch_balances();
                self.fetch_channels();
            }
            Err(e) => {
                self.state.connection_status = ConnectionStatus::Error(e.clone());
                self.state.status_message = Some(StatusMessage::error(e));
            }
        }
    }

    pub fn disconnect(&mut self) {
        self.state.client = None;
        self.state.connection_status = ConnectionStatus::Disconnected;
        self.state.node_info = None;
        self.state.balances = None;
        self.state.channels = None;
        self.state.payments = None;
        self.state.status_message = Some(StatusMessage::success("Disconnected"));
    }

    pub fn fetch_node_info(&mut self) {
        if self.state.tasks.node_info.is_some() {
            return;
        }
        if let Some(client) = &self.state.client {
            let client = client.clone();
            self.state.tasks.node_info = Some(self.rt.spawn(async move {
                client.get_node_info(GetNodeInfoRequest {}).await.map_err(|e| e.to_string())
            }));
        }
    }

    pub fn fetch_balances(&mut self) {
        if self.state.tasks.balances.is_some() {
            return;
        }
        if let Some(client) = &self.state.client {
            let client = client.clone();
            self.state.tasks.balances = Some(self.rt.spawn(async move {
                client.get_balances(GetBalancesRequest {}).await.map_err(|e| e.to_string())
            }));
        }
    }

    pub fn fetch_channels(&mut self) {
        if self.state.tasks.channels.is_some() {
            return;
        }
        if let Some(client) = &self.state.client {
            let client = client.clone();
            self.state.tasks.channels = Some(self.rt.spawn(async move {
                client.list_channels(ListChannelsRequest {}).await.map_err(|e| e.to_string())
            }));
        }
    }

    pub fn fetch_payments(&mut self) {
        if self.state.tasks.payments.is_some() {
            return;
        }
        if let Some(client) = &self.state.client {
            let client = client.clone();
            let page_token = self.state.payments_page_token.clone();
            self.state.tasks.payments = Some(self.rt.spawn(async move {
                client
                    .list_payments(ListPaymentsRequest { page_token })
                    .await
                    .map_err(|e| e.to_string())
            }));
        }
    }

    pub fn generate_onchain_address(&mut self) {
        if self.state.tasks.onchain_receive.is_some() {
            return;
        }
        if let Some(client) = &self.state.client {
            let client = client.clone();
            self.state.tasks.onchain_receive = Some(self.rt.spawn(async move {
                client.onchain_receive(OnchainReceiveRequest {}).await.map_err(|e| e.to_string())
            }));
        }
    }

    pub fn send_onchain(&mut self) {
        if self.state.tasks.onchain_send.is_some() {
            return;
        }
        if let Some(client) = &self.state.client {
            let form = &self.state.forms.onchain_send;
            let address = form.address.trim().to_string();
            let amount_sats = form.amount_sats.trim().parse::<u64>().ok();
            let send_all = if form.send_all { Some(true) } else { None };
            let fee_rate = form.fee_rate_sat_per_vb.trim().parse::<u64>().ok();

            if address.is_empty() {
                self.state.status_message = Some(StatusMessage::error("Address is required"));
                return;
            }

            let client = client.clone();
            self.state.tasks.onchain_send = Some(self.rt.spawn(async move {
                client
                    .onchain_send(OnchainSendRequest {
                        address,
                        amount_sats,
                        send_all,
                        fee_rate_sat_per_vb: fee_rate,
                    })
                    .await
                    .map_err(|e| e.to_string())
            }));
        }
    }

    pub fn generate_bolt11_invoice(&mut self) {
        if self.state.tasks.bolt11_receive.is_some() {
            return;
        }
        if let Some(client) = &self.state.client {
            let form = &self.state.forms.bolt11_receive;
            let amount_msat = form.amount_msat.trim().parse::<u64>().ok();
            let description = form.description.trim().to_string();
            let expiry_secs = form.expiry_secs.trim().parse::<u32>().unwrap_or(86400);

            let invoice_description = if !description.is_empty() {
                Some(Bolt11InvoiceDescription {
                    kind: Some(bolt11_invoice_description::Kind::Direct(description)),
                })
            } else {
                None
            };

            let client = client.clone();
            self.state.tasks.bolt11_receive = Some(self.rt.spawn(async move {
                client
                    .bolt11_receive(Bolt11ReceiveRequest {
                        amount_msat,
                        description: invoice_description,
                        expiry_secs,
                    })
                    .await
                    .map_err(|e| e.to_string())
            }));
        }
    }

    pub fn send_bolt11(&mut self) {
        if self.state.tasks.bolt11_send.is_some() {
            return;
        }
        if let Some(client) = &self.state.client {
            let form = &self.state.forms.bolt11_send;
            let invoice = form.invoice.trim().to_string();
            let amount_msat = form.amount_msat.trim().parse::<u64>().ok();

            if invoice.is_empty() {
                self.state.status_message = Some(StatusMessage::error("Invoice is required"));
                return;
            }

            let client = client.clone();
            self.state.tasks.bolt11_send = Some(self.rt.spawn(async move {
                client
                    .bolt11_send(Bolt11SendRequest { invoice, amount_msat, route_parameters: None })
                    .await
                    .map_err(|e| e.to_string())
            }));
        }
    }

    pub fn generate_bolt12_offer(&mut self) {
        if self.state.tasks.bolt12_receive.is_some() {
            return;
        }
        if let Some(client) = &self.state.client {
            let form = &self.state.forms.bolt12_receive;
            let description = form.description.trim().to_string();
            let amount_msat = form.amount_msat.trim().parse::<u64>().ok();
            let expiry_secs = form.expiry_secs.trim().parse::<u32>().ok();
            let quantity = form.quantity.trim().parse::<u64>().ok();

            if description.is_empty() {
                self.state.status_message = Some(StatusMessage::error("Description is required"));
                return;
            }

            let client = client.clone();
            self.state.tasks.bolt12_receive = Some(self.rt.spawn(async move {
                client
                    .bolt12_receive(Bolt12ReceiveRequest {
                        description,
                        amount_msat,
                        expiry_secs,
                        quantity,
                    })
                    .await
                    .map_err(|e| e.to_string())
            }));
        }
    }

    pub fn send_bolt12(&mut self) {
        if self.state.tasks.bolt12_send.is_some() {
            return;
        }
        if let Some(client) = &self.state.client {
            let form = &self.state.forms.bolt12_send;
            let offer = form.offer.trim().to_string();
            let amount_msat = form.amount_msat.trim().parse::<u64>().ok();
            let quantity = form.quantity.trim().parse::<u64>().ok();
            let payer_note = if form.payer_note.trim().is_empty() {
                None
            } else {
                Some(form.payer_note.trim().to_string())
            };

            if offer.is_empty() {
                self.state.status_message = Some(StatusMessage::error("Offer is required"));
                return;
            }

            let client = client.clone();
            self.state.tasks.bolt12_send = Some(self.rt.spawn(async move {
                client
                    .bolt12_send(Bolt12SendRequest {
                        offer,
                        amount_msat,
                        quantity,
                        payer_note,
                        route_parameters: None,
                    })
                    .await
                    .map_err(|e| e.to_string())
            }));
        }
    }

    pub fn open_channel(&mut self) {
        if self.state.tasks.open_channel.is_some() {
            return;
        }
        if let Some(client) = &self.state.client {
            let form = &self.state.forms.open_channel;
            let node_pubkey = form.node_pubkey.trim().to_string();
            let address = form.address.trim().to_string();
            let channel_amount_sats = match form.channel_amount_sats.trim().parse::<u64>() {
                Ok(v) => v,
                Err(_) => {
                    self.state.status_message =
                        Some(StatusMessage::error("Invalid channel amount"));
                    return;
                }
            };
            let push_to_counterparty_msat =
                form.push_to_counterparty_msat.trim().parse::<u64>().ok();
            let announce_channel = form.announce_channel;

            let channel_config = build_channel_config(
                form.forwarding_fee_proportional_millionths.trim(),
                form.forwarding_fee_base_msat.trim(),
                form.cltv_expiry_delta.trim(),
            );

            if node_pubkey.is_empty() || address.is_empty() {
                self.state.status_message =
                    Some(StatusMessage::error("Node pubkey and address are required"));
                return;
            }

            let client = client.clone();
            self.state.tasks.open_channel = Some(self.rt.spawn(async move {
                client
                    .open_channel(OpenChannelRequest {
                        node_pubkey,
                        address,
                        channel_amount_sats,
                        push_to_counterparty_msat,
                        channel_config,
                        announce_channel,
                    })
                    .await
                    .map_err(|e| e.to_string())
            }));
        }
    }

    pub fn close_channel(&mut self) {
        if self.state.tasks.close_channel.is_some() {
            return;
        }
        if let Some(client) = &self.state.client {
            let form = &self.state.forms.close_channel;
            let user_channel_id = form.user_channel_id.trim().to_string();
            let counterparty_node_id = form.counterparty_node_id.trim().to_string();

            if user_channel_id.is_empty() || counterparty_node_id.is_empty() {
                self.state.status_message = Some(StatusMessage::error(
                    "Channel ID and counterparty node ID are required",
                ));
                return;
            }

            let client = client.clone();
            self.state.tasks.close_channel = Some(self.rt.spawn(async move {
                client
                    .close_channel(CloseChannelRequest { user_channel_id, counterparty_node_id })
                    .await
                    .map_err(|e| e.to_string())
            }));
        }
    }

    pub fn force_close_channel(&mut self) {
        if self.state.tasks.force_close_channel.is_some() {
            return;
        }
        if let Some(client) = &self.state.client {
            let form = &self.state.forms.close_channel;
            let user_channel_id = form.user_channel_id.trim().to_string();
            let counterparty_node_id = form.counterparty_node_id.trim().to_string();
            let force_close_reason = if form.force_close_reason.trim().is_empty() {
                None
            } else {
                Some(form.force_close_reason.trim().to_string())
            };

            if user_channel_id.is_empty() || counterparty_node_id.is_empty() {
                self.state.status_message = Some(StatusMessage::error(
                    "Channel ID and counterparty node ID are required",
                ));
                return;
            }

            let client = client.clone();
            self.state.tasks.force_close_channel = Some(self.rt.spawn(async move {
                client
                    .force_close_channel(ForceCloseChannelRequest {
                        user_channel_id,
                        counterparty_node_id,
                        force_close_reason,
                    })
                    .await
                    .map_err(|e| e.to_string())
            }));
        }
    }

    pub fn splice_in(&mut self) {
        if self.state.tasks.splice_in.is_some() {
            return;
        }
        if let Some(client) = &self.state.client {
            let form = &self.state.forms.splice_in;
            let user_channel_id = form.user_channel_id.trim().to_string();
            let counterparty_node_id = form.counterparty_node_id.trim().to_string();
            let splice_amount_sats = match form.splice_amount_sats.trim().parse::<u64>() {
                Ok(v) => v,
                Err(_) => {
                    self.state.status_message =
                        Some(StatusMessage::error("Invalid splice amount"));
                    return;
                }
            };

            if user_channel_id.is_empty() || counterparty_node_id.is_empty() {
                self.state.status_message = Some(StatusMessage::error(
                    "Channel ID and counterparty node ID are required",
                ));
                return;
            }

            let client = client.clone();
            self.state.tasks.splice_in = Some(self.rt.spawn(async move {
                client
                    .splice_in(SpliceInRequest {
                        user_channel_id,
                        counterparty_node_id,
                        splice_amount_sats,
                    })
                    .await
                    .map_err(|e| e.to_string())
            }));
        }
    }

    pub fn splice_out(&mut self) {
        if self.state.tasks.splice_out.is_some() {
            return;
        }
        if let Some(client) = &self.state.client {
            let form = &self.state.forms.splice_out;
            let user_channel_id = form.user_channel_id.trim().to_string();
            let counterparty_node_id = form.counterparty_node_id.trim().to_string();
            let splice_amount_sats = match form.splice_amount_sats.trim().parse::<u64>() {
                Ok(v) => v,
                Err(_) => {
                    self.state.status_message =
                        Some(StatusMessage::error("Invalid splice amount"));
                    return;
                }
            };
            let address =
                if form.address.trim().is_empty() { None } else { Some(form.address.trim().to_string()) };

            if user_channel_id.is_empty() || counterparty_node_id.is_empty() {
                self.state.status_message = Some(StatusMessage::error(
                    "Channel ID and counterparty node ID are required",
                ));
                return;
            }

            let client = client.clone();
            self.state.tasks.splice_out = Some(self.rt.spawn(async move {
                client
                    .splice_out(SpliceOutRequest {
                        user_channel_id,
                        counterparty_node_id,
                        address,
                        splice_amount_sats,
                    })
                    .await
                    .map_err(|e| e.to_string())
            }));
        }
    }

    pub fn update_channel_config(&mut self) {
        if self.state.tasks.update_channel_config.is_some() {
            return;
        }
        if let Some(client) = &self.state.client {
            let form = &self.state.forms.update_channel_config;
            let user_channel_id = form.user_channel_id.trim().to_string();
            let counterparty_node_id = form.counterparty_node_id.trim().to_string();

            let channel_config = ChannelConfig {
                forwarding_fee_proportional_millionths: form
                    .forwarding_fee_proportional_millionths
                    .trim()
                    .parse()
                    .ok(),
                forwarding_fee_base_msat: form.forwarding_fee_base_msat.trim().parse().ok(),
                cltv_expiry_delta: form.cltv_expiry_delta.trim().parse().ok(),
                force_close_avoidance_max_fee_satoshis: None,
                accept_underpaying_htlcs: None,
                max_dust_htlc_exposure: None,
            };

            if user_channel_id.is_empty() || counterparty_node_id.is_empty() {
                self.state.status_message = Some(StatusMessage::error(
                    "Channel ID and counterparty node ID are required",
                ));
                return;
            }

            let client = client.clone();
            self.state.tasks.update_channel_config = Some(self.rt.spawn(async move {
                client
                    .update_channel_config(UpdateChannelConfigRequest {
                        user_channel_id,
                        counterparty_node_id,
                        channel_config: Some(channel_config),
                    })
                    .await
                    .map_err(|e| e.to_string())
            }));
        }
    }

    pub fn connect_peer(&mut self) {
        if self.state.tasks.connect_peer.is_some() {
            return;
        }
        if let Some(client) = &self.state.client {
            let form = &self.state.forms.connect_peer;
            let node_pubkey = form.node_pubkey.trim().to_string();
            let address = form.address.trim().to_string();
            let persist = form.persist;

            if node_pubkey.is_empty() || address.is_empty() {
                self.state.status_message =
                    Some(StatusMessage::error("Node pubkey and address are required"));
                return;
            }

            let client = client.clone();
            self.state.tasks.connect_peer = Some(self.rt.spawn(async move {
                client
                    .connect_peer(ConnectPeerRequest { node_pubkey, address, persist })
                    .await
                    .map_err(|e| e.to_string())
            }));
        }
    }

    fn poll_tasks(&mut self, ctx: &egui::Context) {
        macro_rules! poll_task {
            ($task:expr => |$val:ident| $handler:expr) => {
                if let Some(t) = &mut $task {
                    if let Some(res) = t.now_or_never() {
                        $task = None;
                        match res {
                            Ok(Ok($val)) => {
                                $handler
                            }
                            Ok(Err(e)) => {
                                self.state.status_message = Some(StatusMessage::error(e));
                            }
                            Err(join_err) => {
                                self.state.status_message =
                                    Some(StatusMessage::error(join_err.to_string()));
                            }
                        }
                    } else {
                        ctx.request_repaint();
                    }
                }
            };
        }

        poll_task!(self.state.tasks.node_info => |v| {
            self.state.node_info = Some(v);
        });

        poll_task!(self.state.tasks.balances => |v| {
            self.state.balances = Some(v);
        });

        poll_task!(self.state.tasks.channels => |v| {
            self.state.channels = Some(v);
        });

        poll_task!(self.state.tasks.payments => |v| {
            self.state.payments_page_token = v.next_page_token.clone();
            self.state.payments = Some(v);
        });

        poll_task!(self.state.tasks.onchain_receive => |v| {
            self.state.onchain_address = Some(v.address);
            self.state.status_message = Some(StatusMessage::success("Address generated"));
        });

        poll_task!(self.state.tasks.onchain_send => |v| {
            self.state.last_txid = Some(v.txid.clone());
            self.state.status_message = Some(StatusMessage::success(format!("Sent! TXID: {}", v.txid)));
            self.state.forms.onchain_send = Default::default();
        });

        poll_task!(self.state.tasks.bolt11_receive => |v| {
            self.state.generated_invoice = Some(v.invoice);
            self.state.status_message = Some(StatusMessage::success("Invoice generated"));
        });

        poll_task!(self.state.tasks.bolt11_send => |v| {
            self.state.last_payment_id = Some(v.payment_id.clone());
            self.state.status_message = Some(StatusMessage::success(format!("Payment sent! ID: {}", v.payment_id)));
            self.state.forms.bolt11_send = Default::default();
        });

        poll_task!(self.state.tasks.bolt12_receive => |v| {
            self.state.generated_offer = Some(v.offer);
            self.state.status_message = Some(StatusMessage::success("Offer generated"));
        });

        poll_task!(self.state.tasks.bolt12_send => |v| {
            self.state.last_payment_id = Some(v.payment_id.clone());
            self.state.status_message = Some(StatusMessage::success(format!("Payment sent! ID: {}", v.payment_id)));
            self.state.forms.bolt12_send = Default::default();
        });

        poll_task!(self.state.tasks.open_channel => |v| {
            self.state.last_channel_id = Some(v.user_channel_id.clone());
            self.state.status_message = Some(StatusMessage::success(format!("Channel opened! ID: {}", v.user_channel_id)));
            self.state.forms.open_channel = Default::default();
            self.state.show_open_channel_dialog = false;
            self.fetch_channels();
        });

        poll_task!(self.state.tasks.close_channel => |_v| {
            self.state.status_message = Some(StatusMessage::success("Channel close initiated"));
            self.state.forms.close_channel = Default::default();
            self.state.show_close_channel_dialog = false;
            self.fetch_channels();
        });

        poll_task!(self.state.tasks.force_close_channel => |_v| {
            self.state.status_message = Some(StatusMessage::success("Force close initiated"));
            self.state.forms.close_channel = Default::default();
            self.state.show_close_channel_dialog = false;
            self.fetch_channels();
        });

        poll_task!(self.state.tasks.splice_in => |_v| {
            self.state.status_message = Some(StatusMessage::success("Splice-in initiated"));
            self.state.forms.splice_in = Default::default();
            self.state.show_splice_in_dialog = false;
            self.fetch_channels();
        });

        poll_task!(self.state.tasks.splice_out => |v| {
            self.state.status_message = Some(StatusMessage::success(format!("Splice-out initiated to {}", v.address)));
            self.state.forms.splice_out = Default::default();
            self.state.show_splice_out_dialog = false;
            self.fetch_channels();
        });

        poll_task!(self.state.tasks.update_channel_config => |_v| {
            self.state.status_message = Some(StatusMessage::success("Channel config updated"));
            self.state.forms.update_channel_config = Default::default();
            self.state.show_update_config_dialog = false;
            self.fetch_channels();
        });

        poll_task!(self.state.tasks.connect_peer => |_v| {
            self.state.status_message = Some(StatusMessage::success("Peer connected successfully"));
            self.state.forms.connect_peer = Default::default();
            self.state.show_connect_peer_dialog = false;
        });
    }
}

fn build_channel_config(fee_prop: &str, fee_base: &str, cltv: &str) -> Option<ChannelConfig> {
    let fee_prop = fee_prop.parse::<u32>().ok();
    let fee_base = fee_base.parse::<u32>().ok();
    let cltv = cltv.parse::<u32>().ok();

    if fee_prop.is_none() && fee_base.is_none() && cltv.is_none() {
        return None;
    }

    Some(ChannelConfig {
        forwarding_fee_proportional_millionths: fee_prop,
        forwarding_fee_base_msat: fee_base,
        cltv_expiry_delta: cltv,
        force_close_avoidance_max_fee_satoshis: None,
        accept_underpaying_htlcs: None,
        max_dust_htlc_exposure: None,
    })
}

impl App for LdkServerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        self.poll_tasks(ctx);

        if self.state.tasks.any_pending() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("LDK Server GUI");
                ui.separator();
                ui::connection::render_status(ui, &self.state);
            });
        });

        egui::SidePanel::left("nav_panel").resizable(false).default_width(140.0).show(ctx, |ui| {
            ui.add_space(10.0);
            ui.heading("Navigation");
            ui.separator();

            let tabs = [
                (ActiveTab::NodeInfo, "Node Info"),
                (ActiveTab::Balances, "Balances"),
                (ActiveTab::Channels, "Channels"),
                (ActiveTab::Payments, "Payments"),
                (ActiveTab::Lightning, "Lightning"),
                (ActiveTab::Onchain, "On-chain"),
            ];

            for (tab, label) in tabs {
                if ui.selectable_label(self.state.active_tab == tab, label).clicked() {
                    self.state.active_tab = tab;
                }
            }
        });

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(msg) = &self.state.status_message {
                    if msg.is_error {
                        ui.colored_label(egui::Color32::RED, &msg.text);
                    } else {
                        ui.colored_label(egui::Color32::GREEN, &msg.text);
                    }
                } else {
                    ui.label("Ready");
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.state.active_tab {
                ActiveTab::NodeInfo => ui::node_info::render(ui, self),
                ActiveTab::Balances => ui::balances::render(ui, self),
                ActiveTab::Channels => ui::channels::render(ui, self),
                ActiveTab::Payments => ui::payments::render(ui, self),
                ActiveTab::Lightning => ui::lightning::render(ui, self),
                ActiveTab::Onchain => ui::onchain::render(ui, self),
            }
        });

        ui::channels::render_dialogs(ctx, self);
    }
}
