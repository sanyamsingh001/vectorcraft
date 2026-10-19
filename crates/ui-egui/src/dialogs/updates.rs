//! The update check's result and retry controls in the existing modal frame.

use vectorcraft_engine::cmd::help::version::{CURRENT_VERSION, VersionStatus};

use super::DialogSpec;
use crate::theme::Tokens;
use crate::updates::{Status, error_text};
use crate::{VectorcraftApp, widgets};

pub(crate) const KIND: &str = "checkUpdates";

pub(super) const SPEC: DialogSpec =
    DialogSpec { heading: |_| tl!("Check for Updates").into(), body, ok: None, min_width: 360.0, max_width: Some(440.0), ..DialogSpec::FORM };

fn body(app: &mut VectorcraftApp, ui: &mut egui::Ui, _: &mut crate::state::Dialog) -> bool {
    crate::updates::frame(app, ui.ctx());
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(tl!("Current version")).color(t.text_dim));
        ui.label(CURRENT_VERSION);
    });
    ui.add_space(12.0);
    let status = app.updates.status.clone();
    match &status {
        Status::Idle | Status::Checking => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(tl!("Checking for updates…"));
            });
        }
        Status::Complete(check) => {
            let message = match check.status {
                VersionStatus::UpdateAvailable => tl!("Update available"),
                VersionStatus::UpToDate => tl!("Up to date"),
                VersionStatus::Ahead => tl!("Newer than latest stable"),
            };
            ui.label(egui::RichText::new(message).color(t.text));
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(tl!("Latest stable version")).color(t.text_dim));
                ui.label(&check.latest_version);
            });
        }
        Status::Failed(error) => {
            ui.label(egui::RichText::new(tl!("Check failed")).color(t.text));
            ui.label(egui::RichText::new(error_text(error)).color(t.text_dim));
        }
    }
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if matches!(status, Status::Failed(_))
            && widgets::secondary_button(ui, tl!("Try Again")).clicked()
            && let Err(error) = app.run("help.checkUpdates", serde_json::json!({}))
        {
            app.status(error);
        }
        if widgets::secondary_button(ui, "View Release").clicked()
            && let Err(error) = app.run("help.openRelease", serde_json::json!({}))
        {
            app.status(error);
        }
    });
    false
}
