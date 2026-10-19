//! Explicit, asynchronous update checks. The frontend owns HTTP; the engine compares versions.
//! State is session-only. No release assets, installers, preferences or startup checks are used.

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use serde_json::{Value, json};
use vectorcraft_engine::cmd::help::version::{self, ReleaseCheck, ReleaseError};

use crate::{VectorcraftApp, dialogs};

const TIMEOUT_SECONDS: f64 = 30.0;
type HttpResult = Result<(u16, Vec<u8>), String>;
type Completion = Box<dyn FnOnce(HttpResult) + Send>;
type Fetch = Box<dyn FnMut(Completion)>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum UpdateError {
    Transport(String),
    Release(ReleaseError),
    Timeout,
    Disconnected,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(crate) enum Status {
    #[default]
    Idle,
    Checking,
    Complete(ReleaseCheck),
    Failed(UpdateError),
}

pub(crate) struct Checker {
    pub status: Status,
    pending: Option<Receiver<HttpResult>>,
    started_at: f64,
    fetch: Fetch,
}

impl Default for Checker {
    fn default() -> Self {
        Self { status: Status::Idle, pending: None, started_at: 0.0, fetch: Box::new(fetch_latest) }
    }
}

impl Checker {
    fn start(&mut self, now: f64) {
        if self.pending.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        self.started_at = now;
        self.status = Status::Checking;
        (self.fetch)(Box::new(move |response| {
            // Closing the app or timing out drops this request's receiver. Its late reply is
            // intentionally ignored, including if a later explicit check has started.
            let _ = tx.send(response);
        }));
    }

    pub(crate) fn poll(&mut self, now: f64) {
        let Some(rx) = &self.pending else { return };
        let result = match rx.try_recv() {
            Ok(Ok((status, body))) => Some(version::compare_latest(version::CURRENT_VERSION, status, &body).map_err(UpdateError::Release)),
            Ok(Err(error)) => Some(Err(UpdateError::Transport(error))),
            Err(TryRecvError::Disconnected) => Some(Err(UpdateError::Disconnected)),
            Err(TryRecvError::Empty) if now - self.started_at >= TIMEOUT_SECONDS => Some(Err(UpdateError::Timeout)),
            Err(TryRecvError::Empty) => None,
        };
        if let Some(result) = result {
            self.pending = None;
            self.status = match result {
                Ok(check) => Status::Complete(check),
                Err(error) => Status::Failed(error),
            };
        }
    }

    fn snapshot(&self) -> Value {
        match &self.status {
            Status::Complete(check) => json!({
                "status": check.status, "currentVersion": check.current_version,
                "latestVersion": check.latest_version, "releaseUrl": check.release_url,
            }),
            status => json!({
                "status": match status { Status::Idle => "idle", Status::Checking => "checking", _ => "error" },
                "currentVersion": version::CURRENT_VERSION,
                "releaseUrl": version::RELEASES_URL,
                "error": if let Status::Failed(error) = status { Some(error_text_in(error, crate::i18n::Lang::EN)) } else { None },
            }),
        }
    }
}

fn fetch_latest(complete: Completion) {
    let mut request = ehttp::Request::get(version::LATEST_RELEASE_API);
    request.headers.insert("Accept", "application/vnd.github+json");
    request.headers.insert("X-GitHub-Api-Version", "2022-11-28");
    // Browsers supply their own User-Agent and forbid setting it from WASM.
    #[cfg(not(target_arch = "wasm32"))]
    request.headers.insert("User-Agent", &format!("VectorCraft/{}", version::CURRENT_VERSION));
    ehttp::fetch(request, move |result| {
        complete(result.map(|response| (response.status, response.bytes)));
    });
}

/// Start a check and show its shared-frame dialog. Repeated invocations share in-flight work.
pub(crate) fn open(app: &mut VectorcraftApp) -> Result<Value, String> {
    if app.ui.dialog.as_ref().is_some_and(|d| d.kind != dialogs::updates::KIND) {
        return Err("Close current dialog before checking for updates.".into());
    }
    app.updates.start(app.last_time);
    app.ui.dialog = Some(crate::state::Dialog::new(dialogs::updates::KIND, Default::default()));
    Ok(app.updates.snapshot())
}

/// Query through the same UI-command surface used by the menu and control channel.
pub(crate) fn snapshot(app: &mut VectorcraftApp) -> Value {
    app.updates.poll(app.last_time);
    app.updates.snapshot()
}

/// Poll even after the dialog is closed, without blocking or scheduling startup checks.
pub(crate) fn frame(app: &mut VectorcraftApp, ctx: &egui::Context) {
    app.updates.poll(ctx.input(|i| i.time));
    if app.updates.pending.is_some() {
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

pub(crate) fn open_release(app: &mut VectorcraftApp) -> Value {
    let url = match &app.updates.status {
        Status::Complete(check) => check.release_url.clone(),
        _ => version::RELEASES_URL.to_string(),
    };
    app.open_url(&url);
    json!({"url": url})
}

/// Localized presentation; the HTTP transport error remains available as its own detail.
pub(crate) fn error_text(error: &UpdateError) -> String {
    error_text_in(error, crate::i18n::current())
}

fn error_text_in(error: &UpdateError, lang: crate::i18n::Lang) -> String {
    use ReleaseError as R;
    use UpdateError as E;
    let message = match error {
        E::Transport(error) => crate::i18n::fmt("Request failed: {error}", &[("error", error)]),
        E::Release(R::Http(status)) => crate::i18n::fmt("Release service returned HTTP {status}", &[("status", &status.to_string())]),
        E::Release(R::NoStableRelease) => "No stable release published".into(),
        E::Release(R::InvalidResponse | R::InvalidVersion) => "Invalid release information".into(),
        E::Release(R::NotStable) => "No stable release returned".into(),
        E::Release(R::InvalidCurrentVersion) => "Invalid installed version".into(),
        E::Timeout => "Check timed out. Try again.".into(),
        E::Disconnected => "No response".into(),
    };
    crate::i18n::message(lang, &message)
}

#[cfg(test)]
mod tests;
