//! The modal dialog window: a backdrop that keeps the rest of the app from taking clicks, and a
//! window with no title bar that opens centred and moves by dragging its heading (the band from its
//! top edge down to the heading), as a title bar would. Where it was moved to stays, per window, for
//! the session, and the window never leaves the screen.

use egui::{Id, Rect, Vec2};

use crate::theme::{self, Tokens};

/// Show the modal dialog window `id` (`title` is its accessible name), `nudge` points below the
/// screen's centre until it is moved, with `margin` inside its frame. `add` draws the contents and
/// should start with [`heading`] (or [`drag_band`]) so the window can be moved.
pub(crate) fn show<R>(ctx: &egui::Context, title: &str, id: Id, nudge: f32, margin: i8, add: impl FnOnce(&mut egui::Ui) -> R) -> Option<R> {
    let t = Tokens::get(ctx);
    // Keep Tab navigation and keyboard activation inside the dialog as well as pointer clicks.
    ctx.memory_mut(|m| m.set_modal_layer(egui::LayerId::new(egui::Order::Foreground, id)));
    let dim = backdrop();
    egui::Area::new(dim.id).order(dim.order).fixed_pos(egui::Pos2::ZERO).show(ctx, |ui| {
        // Modal, but the canvas isn't dimmed so previews stay readable (as in the reference app).
        ui.allocate_rect(ctx.content_rect(), egui::Sense::click());
    });
    egui::Window::new(title)
        .id(id)
        .order(egui::Order::Foreground)
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(egui::Align2::CENTER_CENTER, offset(ctx, id, nudge))
        .frame(egui::Frame::window(&ctx.global_style()).fill(t.panel).inner_margin(egui::Margin::same(margin)))
        .show(ctx, add)
        .and_then(|r| r.inner)
}

/// The layer of the backdrop behind a modal dialog window, which takes the clicks the rest of
/// the app would get.
pub(crate) fn backdrop() -> egui::LayerId {
    egui::LayerId::new(egui::Order::Middle, Id::new("modal-dim"))
}

/// The dialog's heading, which also moves the window ([`drag_band`]).
pub(crate) fn heading(ui: &mut egui::Ui, text: &str) {
    let t = Tokens::get(ui.ctx());
    // Not selectable: a drag on it moves the window instead of selecting its text.
    let r = ui.add(egui::Label::new(egui::RichText::new(text).font(theme::semibold(16.0)).color(t.text)).selectable(false));
    drag_band(ui, r.rect.bottom());
}

/// Make the band of the dialog window [`show`] draws `ui` in, from its top edge down to `bottom`, a
/// handle that moves the window. Widgets added later on the band stay clickable (they sit above it).
pub(crate) fn drag_band(ui: &mut egui::Ui, bottom: f32) {
    let id = ui.layer_id().id;
    // Last frame's rectangle: none while the window measures itself the first time it shows.
    let Some(outer) = ui.ctx().memory(|m| m.area_rect(id)) else { return };
    let band = Rect::from_x_y_ranges(outer.x_range(), outer.top()..=bottom.max(outer.top()));
    let r = ui.interact(band, handle(id), egui::Sense::drag());
    if r.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if r.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
}

/// The drag handle of window `id`.
fn handle(id: Id) -> Id {
    id.with("modal-drag")
}

/// Where window `id` sits, from the screen's centre: `nudge` down until it is first moved. While
/// its [`drag_band`] is dragged, where it sat when the drag began plus how far the pointer has moved
/// since the press, kept on screen (applied before the window places itself, so it follows the
/// pointer without a frame's lag).
fn offset(ctx: &egui::Context, id: Id, nudge: f32) -> Vec2 {
    let (key, start_key) = (id.with("modal-offset"), id.with("modal-drag-start"));
    let offset = ctx.data(|m| m.get_temp::<Vec2>(key)).unwrap_or(egui::vec2(0.0, nudge));
    let dragged = ctx.read_response(handle(id)).is_some_and(|r| r.dragged());
    let moved = ctx.input(|i| Some(i.pointer.latest_pos()? - i.pointer.press_origin()?));
    let (true, Some(moved), Some(rect)) = (dragged, moved, ctx.memory(|m| m.area_rect(id))) else {
        ctx.data_mut(|m| m.remove::<Vec2>(start_key));
        return offset;
    };
    let start = ctx.data_mut(|m| *m.get_temp_mut_or(start_key, offset));
    let screen = ctx.content_rect();
    let to = on_screen(Rect::from_center_size(screen.center() + start + moved, rect.size()), screen).center() - screen.center();
    ctx.data_mut(|m| m.insert_temp(key, to));
    to
}

/// `r` moved the least to lie inside `screen` (its top left inside when it is larger).
fn on_screen(r: Rect, screen: Rect) -> Rect {
    let x = r.left().min(screen.right() - r.width()).max(screen.left());
    let y = r.top().min(screen.bottom() - r.height()).max(screen.top());
    Rect::from_min_size(egui::pos2(x, y), r.size())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_moved_window_stays_on_screen() {
        let screen = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let r = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h));
        assert_eq!(on_screen(r(100.0, 100.0, 200.0, 100.0), screen), r(100.0, 100.0, 200.0, 100.0));
        assert_eq!(on_screen(r(-50.0, 550.0, 200.0, 100.0), screen), r(0.0, 500.0, 200.0, 100.0));
        assert_eq!(on_screen(r(700.0, -20.0, 200.0, 100.0), screen), r(600.0, 0.0, 200.0, 100.0));
        // Larger than the screen: its top left stays in view.
        assert_eq!(on_screen(r(-10.0, -10.0, 900.0, 700.0), screen), r(0.0, 0.0, 900.0, 700.0));
    }

    /// Document tabs use click senses too: Tab/Space must reach them only after the modal closes.
    #[test]
    fn a_modal_keeps_tab_and_space_away_from_background_document_tabs() {
        struct Frame {
            ctx: egui::Context,
            large_tabs: bool,
            modal: bool,
            focus_edit: bool,
            text: String,
            targets: [Id; 2],
            activated: [bool; 2],
        }
        impl Frame {
            fn new(large_tabs: bool, modal: bool) -> Self {
                let ctx = egui::Context::default();
                crate::theme::install_fonts(&ctx);
                Self { ctx, large_tabs, modal, focus_edit: true, text: "edit me".into(), targets: [Id::NULL; 2], activated: [false; 2] }
            }
            fn draw(&mut self, events: Vec<egui::Event>) {
                let ctx = self.ctx.clone();
                let mut out = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        // The same tab and close-target geometry and click senses as chrome::doc_tabs.
                        let (strip, _) =
                            ui.allocate_exact_size(egui::vec2(ui.available_width(), if self.large_tabs { 43.0 } else { 35.0 }), egui::Sense::hover());
                        let tab = Rect::from_min_size(strip.min, egui::vec2(180.0, strip.height() - 1.0));
                        let close = Rect::from_center_size(egui::pos2(tab.left() + 16.0, tab.center().y), egui::vec2(12.0, 12.0))
                            .expand(if self.large_tabs { 7.0 } else { 3.0 });
                        self.targets = [ui.id().with(("tab", 0)), ui.id().with(("tabx", 0))];
                        self.activated[0] |= ui.interact(tab, self.targets[0], egui::Sense::click()).clicked();
                        self.activated[1] |= ui.interact(close, self.targets[1], egui::Sense::click()).clicked();
                        if self.modal {
                            super::show(ui.ctx(), "Keyboard modal", Id::new("keyboard-modal"), 0.0, 20, |ui| {
                                super::heading(ui, "Keyboard modal");
                                let edit = ui.add(egui::TextEdit::singleline(&mut self.text).id(Id::new("modal-edit")));
                                if self.focus_edit {
                                    edit.request_focus();
                                }
                                let _ = ui.button("Keep editing");
                            });
                        }
                        self.focus_edit = false;
                    },
                );
                out.textures_delta.clear();
            }
            fn key(&mut self, key: egui::Key, reverse: bool) {
                let modifiers = egui::Modifiers { shift: reverse, ..Default::default() };
                self.draw([true, false].map(|pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers }).to_vec());
            }
        }
        let mut leaks = vec![];
        for large_tabs in [false, true] {
            // Positive control: actual Tab/Space input activates both background targets.
            let mut background = Frame::new(large_tabs, false);
            for _ in 0..4 {
                background.draw(vec![]);
            }
            for target in 0..2 {
                background.key(egui::Key::Tab, false);
                background.key(egui::Key::Space, false);
                assert!(background.activated[target], "background target {target} must accept keyboard activation");
            }
            for reverse in [false, true] {
                let mut f = Frame::new(large_tabs, true);
                for _ in 0..4 {
                    f.draw(vec![]);
                }
                assert_eq!(f.ctx.memory(|m| m.focused()), Some(Id::new("modal-edit")));
                f.draw(vec![egui::Event::Text("x".into())]);
                assert!(f.text.contains('x'), "the modal's field still accepts typing");
                let mut background_focus = false;
                for _ in 0..8 {
                    f.key(egui::Key::Tab, reverse);
                    if f.ctx.memory(|m| m.focused()).is_some_and(|id| f.targets.contains(&id)) {
                        background_focus = true;
                        f.key(egui::Key::Space, false);
                        break;
                    }
                }
                eprintln!("large_tabs={large_tabs}, reverse={reverse}: background_focus={background_focus}, activated={:?}", f.activated);
                if background_focus || f.activated.iter().any(|v| *v) {
                    leaks.push((large_tabs, reverse, f.activated));
                }
                // Closing the modal restores normal keyboard access to the tabs.
                f.modal = false;
                for _ in 0..4 {
                    f.draw(vec![]);
                }
                f.activated = [false; 2];
                for _ in 0..8 {
                    f.key(egui::Key::Tab, false);
                    if f.ctx.memory(|m| m.focused()).is_some_and(|id| f.targets.contains(&id)) {
                        f.key(egui::Key::Space, false);
                        break;
                    }
                }
                assert!(f.activated.iter().any(|v| *v), "closing the modal restores background keyboard activation");
            }
        }
        assert!(leaks.is_empty(), "Tab/Space reached document targets behind a modal: {leaks:?}");
    }
}
