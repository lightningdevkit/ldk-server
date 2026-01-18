use egui::Ui;

use crate::app::LdkServerApp;
use crate::state::{AppState, ConnectionStatus};

pub fn render_status(ui: &mut Ui, state: &AppState) {
    match &state.connection_status {
        ConnectionStatus::Disconnected => {
            ui.colored_label(egui::Color32::GRAY, "Disconnected");
        }
        ConnectionStatus::Connected => {
            ui.colored_label(egui::Color32::GREEN, "Connected");
        }
        ConnectionStatus::Error(e) => {
            ui.colored_label(egui::Color32::RED, format!("Error: {}", e));
        }
    }
}

pub fn render_settings(ui: &mut Ui, app: &mut LdkServerApp) {
    ui.group(|ui| {
        ui.heading("Connection Settings");
        ui.add_space(5.0);

        egui::Grid::new("connection_grid").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
            ui.label("Server URL:");
            ui.text_edit_singleline(&mut app.state.server_url);
            ui.end_row();

            ui.label("API Key:");
            ui.add(egui::TextEdit::singleline(&mut app.state.api_key).password(true));
            ui.end_row();

            ui.label("TLS Cert Path:");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut app.state.tls_cert_path);
                if ui.button("Browse...").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("PEM files", &["pem"])
                        .add_filter("All files", &["*"])
                        .pick_file()
                    {
                        app.state.tls_cert_path = path.display().to_string();
                    }
                }
            });
            ui.end_row();
        });

        ui.add_space(10.0);

        ui.horizontal(|ui| {
            let is_connected = matches!(app.state.connection_status, ConnectionStatus::Connected);
            if is_connected {
                if ui.button("Disconnect").clicked() {
                    app.disconnect();
                }
            } else if ui.button("Connect").clicked() {
                app.connect();
            }
        });
    });
}
