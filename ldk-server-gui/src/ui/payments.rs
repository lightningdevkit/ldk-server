use egui::{ScrollArea, Ui};

use crate::app::LdkServerApp;
use crate::state::ConnectionStatus;
use crate::ui::{format_msat, truncate_id};

pub fn render(ui: &mut Ui, app: &mut LdkServerApp) {
    ui.heading("Payments");
    ui.add_space(10.0);

    if !matches!(app.state.connection_status, ConnectionStatus::Connected) {
        ui.label("Connect to a server to view payments.");
        return;
    }

    ui.horizontal(|ui| {
        if app.state.tasks.payments.is_some() {
            ui.spinner();
            ui.label("Loading...");
        } else {
            if ui.button("Refresh").clicked() {
                app.state.payments_page_token = None;
                app.fetch_payments();
            }
            if app.state.payments_page_token.is_some() && ui.button("Load More").clicked() {
                app.fetch_payments();
            }
        }
    });

    ui.add_space(10.0);

    if let Some(payments_response) = &app.state.payments {
        let payments = &payments_response.payments;
        if payments.is_empty() {
            ui.label("No payments found.");
        } else {
            ui.label(format!("{} payment(s)", payments.len()));
            ui.add_space(5.0);

            ScrollArea::both().max_height(500.0).show(ui, |ui| {
                egui::Grid::new("payments_grid")
                    .striped(true)
                    .min_col_width(80.0)
                    .show(ui, |ui| {
                        // Header
                        ui.strong("Payment ID");
                        ui.strong("Type");
                        ui.strong("Amount");
                        ui.strong("Fee");
                        ui.strong("Direction");
                        ui.strong("Status");
                        ui.strong("Timestamp");
                        ui.end_row();

                        for payment in payments {
                            // Payment ID
                            ui.horizontal(|ui| {
                                ui.monospace(truncate_id(&payment.id, 8));
                                if ui.small_button("Copy").clicked() {
                                    ui.output_mut(|o| o.copied_text = payment.id.clone());
                                }
                            });

                            // Type
                            let payment_type = payment
                                .kind
                                .as_ref()
                                .map(|k| format_payment_kind(k))
                                .unwrap_or_else(|| "Unknown".to_string());
                            ui.label(payment_type);

                            // Amount
                            if let Some(amount) = payment.amount_msat {
                                ui.label(format_msat(amount));
                            } else {
                                ui.label("-");
                            }

                            // Fee
                            if let Some(fee) = payment.fee_paid_msat {
                                ui.label(format_msat(fee));
                            } else {
                                ui.label("-");
                            }

                            // Direction (0 = Inbound, 1 = Outbound)
                            let direction = match payment.direction {
                                0 => "Inbound",
                                1 => "Outbound",
                                _ => "Unknown",
                            };
                            ui.label(direction);

                            // Status (0 = Pending, 1 = Succeeded, 2 = Failed)
                            match payment.status {
                                0 => {
                                    ui.colored_label(egui::Color32::YELLOW, "Pending");
                                }
                                1 => {
                                    ui.colored_label(egui::Color32::GREEN, "Succeeded");
                                }
                                2 => {
                                    ui.colored_label(egui::Color32::RED, "Failed");
                                }
                                _ => {
                                    ui.label("Unknown");
                                }
                            };

                            // Timestamp
                            ui.label(format_timestamp(payment.latest_update_timestamp));

                            ui.end_row();
                        }
                    });
            });

            if payments_response.next_page_token.is_some() {
                ui.add_space(5.0);
                ui.label("More payments available. Click 'Load More' to fetch.");
            }
        }
    } else {
        ui.label("No payment data available. Click Refresh to fetch.");
    }
}

fn format_payment_kind(kind: &ldk_server_client::ldk_server_protos::types::PaymentKind) -> String {
    use ldk_server_client::ldk_server_protos::types::payment_kind::Kind;

    match &kind.kind {
        Some(Kind::Onchain(_)) => "On-chain".to_string(),
        Some(Kind::Bolt11(_)) => "BOLT11".to_string(),
        Some(Kind::Bolt11Jit(_)) => "BOLT11 JIT".to_string(),
        Some(Kind::Bolt12Offer(_)) => "BOLT12 Offer".to_string(),
        Some(Kind::Bolt12Refund(_)) => "BOLT12 Refund".to_string(),
        Some(Kind::Spontaneous(_)) => "Spontaneous".to_string(),
        None => "Unknown".to_string(),
    }
}

fn format_timestamp(ts: u64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let datetime = UNIX_EPOCH + Duration::from_secs(ts);
    // Simple format - just show relative to now or absolute
    let now = std::time::SystemTime::now();
    if let Ok(elapsed) = now.duration_since(datetime) {
        let secs = elapsed.as_secs();
        if secs < 60 {
            format!("{}s ago", secs)
        } else if secs < 3600 {
            format!("{}m ago", secs / 60)
        } else if secs < 86400 {
            format!("{}h ago", secs / 3600)
        } else {
            format!("{}d ago", secs / 86400)
        }
    } else {
        format!("{}", ts)
    }
}
