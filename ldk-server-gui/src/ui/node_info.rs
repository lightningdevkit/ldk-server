use egui::Ui;

use crate::app::LdkServerApp;
use crate::state::ConnectionStatus;
use crate::ui::connection;

pub fn render(ui: &mut Ui, app: &mut LdkServerApp) {
    ui.heading("Node Information");
    ui.add_space(10.0);

    connection::render_settings(ui, app);
    ui.add_space(10.0);

    if !matches!(app.state.connection_status, ConnectionStatus::Connected) {
        ui.label("Connect to a server to view node information.");
        return;
    }

    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.heading("Node Details");
            if app.state.tasks.node_info.is_some() {
                ui.spinner();
            } else if ui.button("Refresh").clicked() {
                app.fetch_node_info();
            }
        });
        ui.add_space(5.0);

        if let Some(info) = &app.state.node_info {
            egui::Grid::new("node_info_grid").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
                ui.label("Node ID:");
                ui.horizontal(|ui| {
                    let node_id = &info.node_id;
                    ui.monospace(crate::ui::truncate_id(node_id, 24));
                    if ui.small_button("Copy").clicked() {
                        ui.output_mut(|o| o.copied_text = node_id.clone());
                    }
                });
                ui.end_row();

                if let Some(block) = &info.current_best_block {
                    ui.label("Best Block:");
                    ui.monospace(format!("{} (height: {})", crate::ui::truncate_id(&block.block_hash, 16), block.height));
                    ui.end_row();
                }

                if let Some(ts) = info.latest_lightning_wallet_sync_timestamp {
                    ui.label("Lightning Wallet Sync:");
                    ui.label(format_timestamp(ts));
                    ui.end_row();
                }

                if let Some(ts) = info.latest_onchain_wallet_sync_timestamp {
                    ui.label("On-chain Wallet Sync:");
                    ui.label(format_timestamp(ts));
                    ui.end_row();
                }

                if let Some(ts) = info.latest_fee_rate_cache_update_timestamp {
                    ui.label("Fee Rate Cache Update:");
                    ui.label(format_timestamp(ts));
                    ui.end_row();
                }

                if let Some(ts) = info.latest_rgs_snapshot_timestamp {
                    ui.label("RGS Snapshot:");
                    ui.label(format_timestamp(ts));
                    ui.end_row();
                }

                if let Some(ts) = info.latest_node_announcement_broadcast_timestamp {
                    ui.label("Node Announcement:");
                    ui.label(format_timestamp(ts));
                    ui.end_row();
                }
            });
        } else {
            ui.label("No node info available. Click Refresh to fetch.");
        }
    });
}

fn format_timestamp(ts: u64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let datetime = UNIX_EPOCH + Duration::from_secs(ts);
    format!("{:?}", datetime)
}
