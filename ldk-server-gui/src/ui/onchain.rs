use egui::Ui;

use crate::app::LdkServerApp;
use crate::state::{ConnectionStatus, OnchainTab};

pub fn render(ui: &mut Ui, app: &mut LdkServerApp) {
    ui.heading("On-chain Transactions");
    ui.add_space(10.0);

    if !matches!(app.state.connection_status, ConnectionStatus::Connected) {
        ui.label("Connect to a server to use on-chain transactions.");
        return;
    }

    ui.horizontal(|ui| {
        if ui.selectable_label(app.state.onchain_tab == OnchainTab::Send, "Send").clicked() {
            app.state.onchain_tab = OnchainTab::Send;
        }
        if ui.selectable_label(app.state.onchain_tab == OnchainTab::Receive, "Receive").clicked() {
            app.state.onchain_tab = OnchainTab::Receive;
        }
    });

    ui.separator();
    ui.add_space(10.0);

    match app.state.onchain_tab {
        OnchainTab::Send => render_send(ui, app),
        OnchainTab::Receive => render_receive(ui, app),
    }
}

fn render_send(ui: &mut Ui, app: &mut LdkServerApp) {
    ui.group(|ui| {
        ui.heading("Send On-chain");
        ui.add_space(5.0);

        let form = &mut app.state.forms.onchain_send;

        egui::Grid::new("onchain_send_grid")
            .num_columns(2)
            .spacing([10.0, 5.0])
            .show(ui, |ui| {
                ui.label("Address:");
                ui.text_edit_singleline(&mut form.address);
                ui.end_row();

                ui.label("Amount (sats):");
                ui.add_enabled(!form.send_all, egui::TextEdit::singleline(&mut form.amount_sats));
                ui.end_row();

                ui.label("Send All:");
                ui.checkbox(&mut form.send_all, "Send entire balance");
                ui.end_row();

                ui.label("Fee Rate (sat/vB, optional):");
                ui.text_edit_singleline(&mut form.fee_rate_sat_per_vb);
                ui.end_row();
            });

        ui.add_space(10.0);

        ui.horizontal(|ui| {
            let is_pending = app.state.tasks.onchain_send.is_some();
            if is_pending {
                ui.spinner();
                ui.label("Sending...");
            } else if ui.button("Send").clicked() {
                app.send_onchain();
            }
        });

        if let Some(txid) = &app.state.last_txid {
            ui.add_space(10.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Last TXID:");
                ui.monospace(crate::ui::truncate_id(txid, 24));
                if ui.small_button("Copy").clicked() {
                    ui.output_mut(|o| o.copied_text = txid.clone());
                }
            });
        }
    });
}

fn render_receive(ui: &mut Ui, app: &mut LdkServerApp) {
    ui.group(|ui| {
        ui.heading("Receive On-chain");
        ui.add_space(5.0);

        ui.horizontal(|ui| {
            let is_pending = app.state.tasks.onchain_receive.is_some();
            if is_pending {
                ui.spinner();
                ui.label("Generating...");
            } else if ui.button("Generate Address").clicked() {
                app.generate_onchain_address();
            }
        });

        if let Some(address) = &app.state.onchain_address {
            ui.add_space(10.0);
            ui.separator();
            ui.label("Address:");
            ui.add(egui::TextEdit::singleline(&mut address.as_str())
                .desired_width(f32::INFINITY)
                .interactive(false));
            if ui.button("Copy Address").clicked() {
                ui.output_mut(|o| o.copied_text = address.clone());
            }
        }
    });
}
