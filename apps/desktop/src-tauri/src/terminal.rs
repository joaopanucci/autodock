//! The integrated terminal, on top of `autodock_core::terminal`.
//!
//! The shell runs in a Docker container (see the core module); this file keeps the open
//! terminals and carries what they print to the webview through a Tauri channel (as base64, so a
//! character split between two reads is never broken) and what is typed back.
use crate::localized;
use autodock_core::terminal::{self, ImageInfo, Session};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, Mutex,
    },
};
use tauri::{ipc::Channel, State};

/// Terminals open at once.
const MAX_SESSIONS: usize = 4;

#[derive(Default)]
pub struct Terminals {
    sessions: Mutex<HashMap<u32, Arc<Session>>>,
    next: AtomicU32,
    /// The containers a previous AutoDock left behind were already looked for.
    swept: AtomicBool,
}

impl Terminals {
    fn get(&self, id: u32) -> Result<Arc<Session>, String> {
        self.sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&id)
            .cloned()
            .ok_or_else(|| "This terminal is not open any more.".to_string())
    }

    /// Ends every terminal (the app is closing): their containers must not be left running.
    pub fn stop_all(&self) {
        let sessions: Vec<Arc<Session>> = self
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain()
            .map(|(_, session)| session)
            .collect();
        for session in sessions {
            session.stop();
        }
    }
}

/// What a terminal tells the webview.
#[derive(Clone, Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum TerminalEvent {
    /// Output, base64 encoded.
    Data { data: String },
    /// The shell ended.
    Exit { code: Option<u32> },
}

#[tauri::command]
pub fn terminal_images() -> Vec<ImageInfo> {
    terminal::images()
}

#[tauri::command]
pub async fn terminal_default_image(root: String, lang: Option<String>) -> Result<String, String> {
    localized(lang, move || terminal::default_image(&root)).await?
}

#[tauri::command]
pub async fn terminal_start(
    terminals: State<'_, Terminals>,
    root: String,
    image: String,
    cols: u16,
    rows: u16,
    on_event: Channel<TerminalEvent>,
    lang: Option<String>,
) -> Result<u32, String> {
    if terminals
        .sessions
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .len()
        >= MAX_SESSIONS
    {
        return Err(format!(
            "At most {MAX_SESSIONS} terminals can be open: close one first."
        ));
    }
    let id = terminals.next.fetch_add(1, Ordering::Relaxed) + 1;
    // Once per run, before the first terminal: a killed or crashed AutoDock leaves its containers.
    let sweep = !terminals.swept.swap(true, Ordering::Relaxed);
    let session = localized(lang, move || {
        if sweep {
            terminal::remove_orphans();
        }
        let plan = terminal::plan(&root, &image, id)?;
        let output = on_event.clone();
        Session::start(
            &plan,
            cols,
            rows,
            move |bytes| {
                let _ = output.send(TerminalEvent::Data {
                    data: STANDARD.encode(bytes),
                });
            },
            move |code| {
                let _ = on_event.send(TerminalEvent::Exit { code });
            },
        )
    })
    .await??;
    terminals
        .sessions
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(id, Arc::new(session));
    Ok(id)
}

/// What the person typed (or pasted).
#[tauri::command]
pub async fn terminal_write(
    terminals: State<'_, Terminals>,
    id: u32,
    data: String,
) -> Result<(), String> {
    terminals.get(id)?.write(data.as_bytes())
}

#[tauri::command]
pub fn terminal_resize(
    terminals: State<'_, Terminals>,
    id: u32,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    terminals.get(id)?.resize(cols, rows)
}

/// Closes a terminal and removes its container. Closing one that is gone is not an error.
#[tauri::command]
pub fn terminal_stop(terminals: State<'_, Terminals>, id: u32) {
    let session = terminals
        .sessions
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&id);
    if let Some(session) = session {
        session.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_events_are_tagged_and_carry_base64() {
        let data = serde_json::to_value(TerminalEvent::Data {
            data: STANDARD.encode("olá\r\n"),
        })
        .unwrap();
        assert_eq!(data["event"], "data");
        let bytes = STANDARD.decode(data["data"].as_str().unwrap()).unwrap();
        assert_eq!(String::from_utf8(bytes).unwrap(), "olá\r\n");
        let exit = serde_json::to_value(TerminalEvent::Exit { code: Some(2) }).unwrap();
        assert_eq!(
            (exit["event"].as_str(), exit["code"].as_u64()),
            (Some("exit"), Some(2))
        );
    }

    #[test]
    fn a_terminal_that_is_not_open_is_an_error_and_stopping_all_is_harmless() {
        let terminals = Terminals::default();
        assert!(terminals.get(1).is_err());
        terminals.stop_all();
        assert!(terminals.sessions.lock().unwrap().is_empty());
    }
}
