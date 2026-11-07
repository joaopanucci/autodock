//! In-app updates, on top of the official Tauri updater.
//!
//! The release workflow signs every installer and uploads a `latest.json` next to them.
//! The app only accepts an update whose signature matches the public key built into it
//! (`plugins.updater.pubkey`), downloaded over HTTPS from the one endpoint in the config.
//! The webview never talks to the updater plugin directly (its commands are not granted
//! in the capabilities): it only calls the commands below, and nothing is installed
//! until the user clicks *Update*.
use crate::localized;
use autodock_core::update_prefs::{self, UpdatePrefs};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{ipc::Channel, AppHandle, State};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Release notes are shown as plain text; a huge body is cut.
const MAX_NOTES_CHARS: usize = 4000;

/// The update found by the last check, kept here so installing it needs nothing from the webview.
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

impl PendingUpdate {
    fn set(&self, update: Option<Update>) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = update;
    }
    fn get(&self) -> Option<Update> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOffer {
    pub current_version: String,
    pub version: String,
    /// Release notes, plain text.
    pub notes: Option<String>,
    /// Publication date, seconds since the Unix epoch.
    pub date: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckReport {
    /// A request was made. False when the automatic check is off or not due yet.
    pub checked: bool,
    pub offer: Option<UpdateOffer>,
    /// Why the check could not be done (offline, no release yet…). Shown only for a manual check.
    pub error: Option<String>,
}

impl CheckReport {
    fn idle() -> Self {
        Self {
            checked: false,
            offer: None,
            error: None,
        }
    }
    fn failed(error: impl ToString) -> Self {
        Self {
            checked: true,
            offer: None,
            error: Some(error.to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event")]
pub enum DownloadEvent {
    #[serde(rename = "started", rename_all = "camelCase")]
    Started { content_length: Option<u64> },
    #[serde(rename = "progress", rename_all = "camelCase")]
    Progress { chunk_length: usize },
    #[serde(rename = "finished")]
    Finished,
}

fn plain_notes(body: Option<String>) -> Option<String> {
    let body = body?;
    let body = body.trim();
    (!body.is_empty()).then(|| body.chars().take(MAX_NOTES_CHARS).collect())
}

async fn run_check(app: &AppHandle, pending: &PendingUpdate, automatic: bool) -> CheckReport {
    if automatic && !update_prefs::auto_check_due() {
        return CheckReport::idle();
    }
    let updater = match app.updater() {
        Ok(updater) => updater,
        Err(error) => return CheckReport::failed(error),
    };
    match updater.check().await {
        Err(error) => CheckReport::failed(error),
        Ok(found) => {
            // A failed save only means the next start checks again.
            let _ = update_prefs::record_check();
            let Some(update) = found else {
                pending.set(None);
                return CheckReport {
                    checked: true,
                    offer: None,
                    error: None,
                };
            };
            if automatic && update_prefs::is_skipped(&update.version) {
                pending.set(None);
                return CheckReport {
                    checked: true,
                    offer: None,
                    error: None,
                };
            }
            let offer = UpdateOffer {
                current_version: update.current_version.clone(),
                version: update.version.clone(),
                notes: plain_notes(update.body.clone()),
                date: update.date.map(|date| date.unix_timestamp()),
            };
            pending.set(Some(update));
            CheckReport {
                checked: true,
                offer: Some(offer),
                error: None,
            }
        }
    }
}

/// When the app opens: only reaches the network if the user left the check on and the last
/// one is over a day old; a skipped version is not offered again.
#[tauri::command]
pub async fn update_auto_check(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<CheckReport, String> {
    Ok(run_check(&app, &pending, true).await)
}

/// The user pressed *Check now*: always asks, and also offers a version they skipped.
#[tauri::command]
pub async fn update_check_now(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<CheckReport, String> {
    Ok(run_check(&app, &pending, false).await)
}

/// Downloads the update found by the last check, verifies its signature and installs it.
/// On Windows the app exits when the installer starts (and the installer relaunches it).
#[tauri::command]
pub async fn update_install(
    pending: State<'_, PendingUpdate>,
    on_event: Channel<DownloadEvent>,
) -> Result<(), String> {
    let update = pending
        .get()
        .ok_or_else(|| "There is no update to install: check for updates again.".to_string())?;
    let mut started = false;
    update
        .download_and_install(
            |chunk_length, content_length| {
                if !started {
                    started = true;
                    let _ = on_event.send(DownloadEvent::Started { content_length });
                }
                let _ = on_event.send(DownloadEvent::Progress { chunk_length });
            },
            || {
                let _ = on_event.send(DownloadEvent::Finished);
            },
        )
        .await
        .map_err(|error| error.to_string())
}

/// Restarts the app so the installed update runs (macOS and Linux).
#[tauri::command]
pub fn update_restart(app: AppHandle) {
    app.restart();
}

#[tauri::command]
pub async fn update_get_prefs(lang: Option<String>) -> Result<UpdatePrefs, String> {
    localized(lang, update_prefs::prefs).await?
}

#[tauri::command]
pub async fn update_set_check_on_start(
    enabled: bool,
    lang: Option<String>,
) -> Result<UpdatePrefs, String> {
    localized(lang, move || update_prefs::set_check_on_start(enabled)).await?
}

#[tauri::command]
pub async fn update_skip_version(
    version: String,
    lang: Option<String>,
) -> Result<UpdatePrefs, String> {
    localized(lang, move || update_prefs::skip_version(&version)).await?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_are_plain_text_trimmed_and_bounded() {
        assert_eq!(plain_notes(None), None);
        assert_eq!(plain_notes(Some("  \n ".into())), None);
        assert_eq!(
            plain_notes(Some("  Fixes a bug.\n".into())).as_deref(),
            Some("Fixes a bug.")
        );
        let long = "é".repeat(MAX_NOTES_CHARS + 500);
        assert_eq!(
            plain_notes(Some(long)).unwrap().chars().count(),
            MAX_NOTES_CHARS
        );
    }

    #[test]
    fn download_events_have_the_shape_the_interface_reads() {
        let json = |event: DownloadEvent| serde_json::to_string(&event).unwrap();
        assert_eq!(
            json(DownloadEvent::Started {
                content_length: Some(10)
            }),
            r#"{"event":"started","contentLength":10}"#
        );
        assert_eq!(
            json(DownloadEvent::Progress { chunk_length: 4 }),
            r#"{"event":"progress","chunkLength":4}"#
        );
        assert_eq!(json(DownloadEvent::Finished), r#"{"event":"finished"}"#);
    }

    #[test]
    fn a_failed_check_is_reported_as_checked_with_the_reason() {
        let report = CheckReport::failed("offline");
        assert!(report.checked && report.offer.is_none());
        assert_eq!(report.error.as_deref(), Some("offline"));
        let idle = CheckReport::idle();
        assert!(!idle.checked && idle.error.is_none());
    }
}
