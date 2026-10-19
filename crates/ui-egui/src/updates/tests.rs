//! Ordinary network mocks only: no live HTTP, downloads, installers, or platform assumptions.

use std::cell::RefCell;
use std::rc::Rc;

use egui::{Pos2, Shape};
use serde_json::json;
use vectorcraft_engine::Session;

use super::*;
use crate::{Services, menus, theme};

struct Mock {
    requests: Rc<RefCell<Vec<Completion>>>,
    count: Rc<RefCell<usize>>,
}

impl Mock {
    fn install(checker: &mut Checker) -> Self {
        let requests = Rc::new(RefCell::new(Vec::new()));
        let count = Rc::new(RefCell::new(0));
        let (pending, called) = (requests.clone(), count.clone());
        checker.fetch = Box::new(move |complete| {
            *called.borrow_mut() += 1;
            pending.borrow_mut().push(complete);
        });
        Self { requests, count }
    }

    fn respond(&self, response: HttpResult) {
        let complete = self.requests.borrow_mut().remove(0);
        complete(response);
    }
}

fn release(tag: &str) -> HttpResult {
    Ok((200, serde_json::to_vec(&json!({"tag_name": tag, "draft": false, "prerelease": false})).unwrap()))
}

#[test]
fn an_explicit_check_shares_in_flight_work_and_compares_a_stable_response() {
    let mut checker = Checker::default();
    let mock = Mock::install(&mut checker);
    checker.poll(1.0);
    assert_eq!(*mock.count.borrow(), 0, "no startup request");
    checker.start(2.0);
    checker.start(3.0);
    assert_eq!(*mock.count.borrow(), 1, "repeated commands share the request");
    assert_eq!(checker.snapshot()["status"], "checking");
    mock.respond(release("v0.8.0"));
    checker.poll(4.0);
    let expected = version::compare_latest(version::CURRENT_VERSION, 200, &release("v0.8.0").unwrap().1).unwrap();
    assert_eq!(checker.status, Status::Complete(expected));
    assert_eq!(checker.snapshot()["latestVersion"], "0.8.0");
    assert_eq!(checker.snapshot()["currentVersion"], version::CURRENT_VERSION);
}

#[test]
fn network_failure_retries_and_reports_the_current_release() {
    let mut checker = Checker::default();
    let mock = Mock::install(&mut checker);
    checker.start(0.0);
    mock.respond(Err("connection unavailable".into()));
    checker.poll(1.0);
    assert!(matches!(checker.status, Status::Failed(UpdateError::Transport(_))));
    assert_eq!(checker.snapshot()["status"], "error");
    checker.start(2.0);
    assert_eq!(checker.status, Status::Checking);
    mock.respond(release(&format!("v{}", version::CURRENT_VERSION)));
    checker.poll(3.0);
    assert_eq!(checker.snapshot()["status"], "upToDate");
    assert_eq!(*mock.count.borrow(), 2);
}

#[test]
fn timeout_can_retry_and_a_late_previous_reply_does_not_replace_it() {
    let mut checker = Checker::default();
    let mock = Mock::install(&mut checker);
    checker.start(0.0);
    checker.poll(TIMEOUT_SECONDS - 1.0);
    assert_eq!(checker.status, Status::Checking);
    checker.poll(TIMEOUT_SECONDS);
    assert_eq!(checker.status, Status::Failed(UpdateError::Timeout));
    checker.start(31.0);
    mock.respond(release("v0.8.0")); // Reply to the timed-out request.
    checker.poll(32.0);
    assert_eq!(checker.status, Status::Checking);
    mock.respond(release(&format!("v{}", version::CURRENT_VERSION)));
    checker.poll(33.0);
    assert_eq!(checker.snapshot()["status"], "upToDate");
}

#[test]
fn http_error_missing_release_and_dropped_callback_are_retryable() {
    let mut checker = Checker::default();
    let mock = Mock::install(&mut checker);
    for (status, error) in [(503, ReleaseError::Http(503)), (404, ReleaseError::NoStableRelease)] {
        checker.start(0.0);
        mock.respond(Ok((status, b"{}".to_vec())));
        checker.poll(1.0);
        assert_eq!(checker.status, Status::Failed(UpdateError::Release(error)));
    }
    checker.start(2.0);
    mock.requests.borrow_mut().clear();
    checker.poll(3.0);
    assert_eq!(checker.status, Status::Failed(UpdateError::Disconnected));
}

/// Painted text and its center: the custom dialog buttons draw a text galley centered in their
/// clickable rect, so these points drive the real pointer path without test hooks in the product.
fn frame(app: &mut VectorcraftApp, ctx: &egui::Context, events: Vec<egui::Event>) -> Vec<(String, Pos2)> {
    fn texts(shape: &Shape, out: &mut Vec<(String, Pos2)>) {
        match shape {
            Shape::Text(text) => out.push((text.galley.text().into(), text.pos + text.galley.size() / 2.0)),
            Shape::Vec(shapes) => shapes.iter().for_each(|s| texts(s, out)),
            _ => {}
        }
    }
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(640.0, 480.0))),
        time: Some(1.0),
        events,
        ..Default::default()
    };
    let mut output = ctx.run_ui(raw, |ui| dialogs::show(app, ui.ctx()));
    output.textures_delta.clear();
    let mut drawn = Vec::new();
    output.shapes.iter().for_each(|s| texts(&s.shape, &mut drawn));
    drawn
}

fn click(app: &mut VectorcraftApp, ctx: &egui::Context, label: &str) {
    let mut drawn = Vec::new();
    for _ in 0..3 {
        drawn = frame(app, ctx, vec![]);
    }
    let translated = crate::i18n::t(label);
    let pos = drawn.iter().find(|(text, _)| text == translated).unwrap_or_else(|| panic!("button {label} not drawn: {drawn:?}")).1;
    frame(
        app,
        ctx,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
        ],
    );
    frame(app, ctx, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
}

#[test]
fn help_command_and_dialog_retry_check_versions_and_open_only_the_release_page() {
    let opened = Rc::new(RefCell::new(Vec::<String>::new()));
    let links = opened.clone();
    let services = Services {
        open_url: Some(Box::new(move |url| links.borrow_mut().push(url.into()))),
        download: Some(Box::new(|_, _| panic!("update checking must not download"))),
        write: Some(Box::new(|_, _| panic!("update checking must not write files"))),
        ..Default::default()
    };
    let mut app = VectorcraftApp::new(Session::new(), services);
    let prefs_before = serde_json::to_value(&app.session.prefs).unwrap();
    let mock = Mock::install(&mut app.updates);
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    assert!(
        menus::menu_entries(&app)
            .iter()
            .any(|entry| entry.path == ["Help"] && entry.command.as_deref() == Some("help.checkUpdates") && entry.enabled)
    );
    assert_eq!(app.run("help.updateStatus", json!({})).unwrap()["status"], "idle");
    assert_eq!(*mock.count.borrow(), 0);
    assert_eq!(app.run("help.checkUpdates", json!({})).unwrap()["status"], "checking");
    assert_eq!(app.ui.dialog.as_ref().unwrap().kind, dialogs::updates::KIND);
    mock.respond(Err("connection unavailable".into()));
    let mut painted = Vec::new();
    for _ in 0..3 {
        painted = frame(&mut app, &ctx, vec![]);
    }
    assert!(painted.iter().any(|(s, _)| s == version::CURRENT_VERSION));
    assert!(painted.iter().any(|(s, _)| s == crate::i18n::t("Check failed")));
    click(&mut app, &ctx, "Try Again");
    assert_eq!(*mock.count.borrow(), 2, "the actual button started another request");
    mock.respond(release("v0.8.0"));
    for _ in 0..3 {
        painted = frame(&mut app, &ctx, vec![]);
    }
    assert!(painted.iter().any(|(s, _)| s == "0.8.0"));
    assert!(painted.iter().any(|(s, _)| s == crate::i18n::t("Update available")));
    assert!(opened.borrow().is_empty(), "checking never opens a browser automatically");
    click(&mut app, &ctx, "View Release");
    assert_eq!(*opened.borrow(), ["https://github.com/storytold/vectorcraft/releases/tag/v0.8.0"]);
    click(&mut app, &ctx, "Close");
    assert!(app.ui.dialog.is_none());
    assert_eq!(serde_json::to_value(&app.session.prefs).unwrap(), prefs_before);
    assert_eq!(app.session.documents().len(), 0);
}

#[test]
fn another_dialog_is_preserved_and_closing_a_check_does_not_start_new_work() {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    let mock = Mock::install(&mut app.updates);
    app.ui.dialog = Some(crate::state::Dialog::new("preferences", Default::default()));
    assert!(!menus::enabled(&app, "help.checkUpdates"));
    assert!(app.run("help.checkUpdates", json!({})).is_err());
    assert_eq!(app.ui.dialog.as_ref().unwrap().kind, "preferences");
    assert_eq!(*mock.count.borrow(), 0);
    crate::tests_synthetic::control(&mut app, &egui::Context::default(), "ui.dialog.cancel", json!({}));
    app.run("help.checkUpdates", json!({})).unwrap();
    crate::tests_synthetic::control(&mut app, &egui::Context::default(), "ui.dialog.cancel", json!({}));
    mock.respond(release(&format!("v{}", version::CURRENT_VERSION)));
    assert_eq!(app.run("help.updateStatus", json!({})).unwrap()["status"], "upToDate");
    assert!(app.ui.dialog.is_none());
    assert_eq!(*mock.count.borrow(), 1);
}

#[test]
fn current_and_ahead_results_show_installed_and_latest_versions_in_the_dialog() {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    let mock = Mock::install(&mut app.updates);
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    for (tag, headline, expected_status) in
        [(format!("v{}", version::CURRENT_VERSION), "Up to date", "upToDate"), ("v0.6.0".into(), "Newer than latest stable", "ahead")]
    {
        app.run("help.checkUpdates", json!({})).unwrap();
        mock.respond(release(&tag));
        let mut painted = Vec::new();
        for _ in 0..3 {
            painted = frame(&mut app, &ctx, vec![]);
        }
        assert!(painted.iter().any(|(s, _)| s == version::CURRENT_VERSION));
        assert!(painted.iter().any(|(s, _)| s == crate::i18n::t(headline)));
        assert!(painted.iter().any(|(s, _)| s == tag.trim_start_matches('v')));
        assert_eq!(app.run("help.updateStatus", json!({})).unwrap()["status"], expected_status);
        crate::tests_synthetic::control(&mut app, &ctx, "ui.dialog.cancel", json!({}));
    }
}

#[test]
fn a_late_reply_preserves_another_modal_opened_after_the_check_was_closed() {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    let mock = Mock::install(&mut app.updates);
    let prefs = app.session.prefs.clone();
    app.run("help.checkUpdates", json!({})).unwrap();
    crate::tests_synthetic::control(&mut app, &egui::Context::default(), "ui.dialog.cancel", json!({}));
    let fields = serde_json::from_value(json!({"page": "general", "testValue": "preserved"})).unwrap();
    app.ui.dialog = Some(crate::state::Dialog::new("preferences", fields));
    let before = app.ui.dialog.clone().unwrap();
    mock.respond(release(&format!("v{}", version::CURRENT_VERSION)));
    assert_eq!(app.run("help.updateStatus", json!({})).unwrap()["status"], "upToDate");
    let after = app.ui.dialog.as_ref().unwrap();
    assert_eq!(after.kind, before.kind);
    assert_eq!(after.fields, before.fields);
    assert_eq!(app.session.prefs, prefs);
    assert_eq!(*mock.count.borrow(), 1);
}

#[test]
fn a_late_reply_after_the_app_is_dropped_is_harmless() {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    let mock = Mock::install(&mut app.updates);
    app.run("help.checkUpdates", json!({})).unwrap();
    drop(app);
    mock.respond(release("v0.7.0"));
    assert!(mock.requests.borrow().is_empty());
    assert_eq!(*mock.count.borrow(), 1);
}

#[test]
fn update_errors_use_message_catalogs_but_status_queries_stay_in_english() {
    let errors = [
        UpdateError::Transport("offline".into()),
        UpdateError::Release(ReleaseError::Http(503)),
        UpdateError::Release(ReleaseError::NoStableRelease),
        UpdateError::Release(ReleaseError::InvalidResponse),
        UpdateError::Release(ReleaseError::InvalidVersion),
        UpdateError::Release(ReleaseError::NotStable),
        UpdateError::Release(ReleaseError::InvalidCurrentVersion),
        UpdateError::Timeout,
        UpdateError::Disconnected,
    ];
    let es = crate::i18n::Lang::from_code("es").unwrap();
    assert_eq!(error_text_in(&errors[0], es), "La solicitud falló: offline");
    assert_eq!(error_text_in(&errors[1], es), "El servicio de versiones devolvió HTTP 503");
    for code in ["cs", "es", "fr", "it", "ru", "ja", "pt-br", "zh-hans", "zh-hant", "uk"] {
        let lang = crate::i18n::Lang::from_code(code).unwrap();
        for error in &errors {
            let english = error_text_in(error, crate::i18n::Lang::EN);
            let translated = error_text_in(error, lang);
            assert_ne!(translated, english, "{code}: untranslated update error {error:?}");
            assert!(!translated.contains("{error}") && !translated.contains("{status}"));
        }
        assert!(error_text_in(&errors[0], lang).contains("offline"));
        assert!(error_text_in(&errors[1], lang).contains("503"));
    }
    let checker = Checker { status: Status::Failed(errors[0].clone()), ..Default::default() };
    assert_eq!(checker.snapshot()["error"], "Request failed: offline");
}

#[test]
fn command_enable_states_follow_modal_policy_through_check_error_and_completion() {
    let opened = Rc::new(RefCell::new(Vec::<String>::new()));
    let links = opened.clone();
    let services = Services { open_url: Some(Box::new(move |url| links.borrow_mut().push(url.into()))), ..Default::default() };
    let mut app = VectorcraftApp::new(Session::new(), services);
    let mock = Mock::install(&mut app.updates);
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    for expected_status in ["idle", "checking", "error", "upToDate"] {
        assert_eq!(app.run("help.updateStatus", json!({})).unwrap()["status"], expected_status);
        for kind in [None, Some(dialogs::updates::KIND), Some("preferences")] {
            app.ui.dialog = kind.map(|k| crate::state::Dialog::new(k, Default::default()));
            assert_eq!(menus::enabled(&app, "help.checkUpdates"), kind != Some("preferences"));
            assert!(menus::enabled(&app, "help.updateStatus"));
            assert!(menus::enabled(&app, "help.openRelease"));
            let entry = menus::menu_entries(&app).into_iter().find(|entry| entry.command.as_deref() == Some("help.checkUpdates")).unwrap();
            assert_eq!(entry.enabled, kind != Some("preferences"));
        }
        let before = app.ui.dialog.clone().unwrap();
        let requests_before = *mock.count.borrow();
        assert!(app.run("help.checkUpdates", json!({})).is_err());
        app.run("help.openRelease", json!({})).unwrap();
        assert_eq!(app.ui.dialog.as_ref().unwrap().kind, before.kind);
        assert_eq!(app.ui.dialog.as_ref().unwrap().fields, before.fields);
        assert_eq!(*mock.count.borrow(), requests_before);
        crate::tests_synthetic::control(&mut app, &ctx, "ui.dialog.cancel", json!({}));
        match expected_status {
            "idle" => {
                app.run("help.checkUpdates", json!({})).unwrap();
                // Shared read-only result dialogs do not confirm on Enter.
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }],
                );
                assert_eq!(app.ui.dialog.as_ref().unwrap().kind, dialogs::updates::KIND);
                assert_eq!(*mock.count.borrow(), 1);
            }
            "checking" => mock.respond(Err("offline".into())),
            "error" => {
                app.run("help.checkUpdates", json!({})).unwrap();
                mock.respond(release(&format!("v{}", version::CURRENT_VERSION)));
            }
            _ => {}
        }
    }
    assert_eq!(*mock.count.borrow(), 2);
    assert_eq!(opened.borrow().len(), 4, "release-page actions were explicit and mocked");
    assert_eq!(opened.borrow()[0], version::RELEASES_URL);
    assert_eq!(opened.borrow()[3], format!("https://github.com/storytold/vectorcraft/releases/tag/v{}", version::CURRENT_VERSION));
    // The global Escape policy closes a check without another request or a modal replacement.
    app.run("help.checkUpdates", json!({})).unwrap();
    let raw = egui::RawInput {
        events: vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }],
        ..Default::default()
    };
    ctx.run_ui(raw, |ui| {
        crate::shortcuts::handle(&mut app, ui.ctx());
        dialogs::show(&mut app, ui.ctx());
    })
    .textures_delta
    .clear();
    assert!(app.ui.dialog.is_none());
    assert_eq!(*mock.count.borrow(), 3);
    mock.respond(release(&format!("v{}", version::CURRENT_VERSION)));
    assert_eq!(app.run("help.updateStatus", json!({})).unwrap()["status"], "upToDate");
    assert!(app.ui.dialog.is_none());
}
