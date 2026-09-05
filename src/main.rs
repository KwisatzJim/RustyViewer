#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use rusty_viewer::editor::{self, Adjustments, BatchOptions, BatchResult, Editor, View};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::{Emitter, Manager, State};
#[derive(Default, Clone)]
struct AppState(Arc<Mutex<Editor>>, Arc<Mutex<Option<arboard::Clipboard>>>);
async fn work<T: Send + 'static>(
    state: &AppState,
    f: impl FnOnce(&mut Editor) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let state = state.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut editor = state
            .0
            .lock()
            .map_err(|_| "Image worker failed; restart RustyViewer.".to_string())?;
        f(&mut editor)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn open_image(
    state: State<'_, AppState>,
    path: PathBuf,
    discard: bool,
) -> Result<View, String> {
    let view = work(&state, move |e| e.open(path, discard)).await?;
    #[cfg(debug_assertions)]
    eprintln!(
        "RustyViewer: image loaded ({} x {})",
        view.width, view.height
    );
    Ok(view)
}
#[tauri::command]
async fn adjust_image(
    state: State<'_, AppState>,
    adjustments: Adjustments,
) -> Result<View, String> {
    work(&state, move |e| e.adjust(adjustments)).await
}
#[tauri::command]
async fn edit_image(
    state: State<'_, AppState>,
    action: String,
    args: Vec<u32>,
) -> Result<View, String> {
    work(&state, move |e| e.edit(&action, &args)).await
}
#[tauri::command]
async fn export_image(state: State<'_, AppState>, path: PathBuf) -> Result<(), String> {
    work(&state, move |e| e.export(&path)).await
}
#[tauri::command]
async fn copy_image(state: State<'_, AppState>) -> Result<(), String> {
    let clipboard = state.1.clone();
    work(&state, move |e| {
        let image = e.rendered()?.into_rgba8();
        let mut guard = clipboard.lock().map_err(|e| e.to_string())?;
        if guard.is_none() {
            *guard = Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
        }
        let clipboard = guard.as_mut().unwrap();
        clipboard
            .set_image(arboard::ImageData {
                width: image.width() as usize,
                height: image.height() as usize,
                bytes: std::borrow::Cow::Owned(image.into_raw()),
            })
            .map_err(|e| e.to_string())
    })
    .await
}
#[tauri::command]
async fn paste_image(state: State<'_, AppState>, discard: bool) -> Result<View, String> {
    work(&state, move |e| {
        let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        let image = clipboard
            .get_image()
            .map_err(|e| format!("No image available on the clipboard: {e}"))?;
        let w = u32::try_from(image.width).map_err(|e| e.to_string())?;
        let h = u32::try_from(image.height).map_err(|e| e.to_string())?;
        editor::dimensions(w, h)?;
        let image = image::RgbaImage::from_raw(w, h, image.bytes.into_owned())
            .ok_or("Invalid clipboard image.")?;
        e.paste(image::DynamicImage::ImageRgba8(image), discard)
    })
    .await
}
#[tauri::command]
async fn run_batch(app: tauri::AppHandle, options: BatchOptions) -> Result<BatchResult, String> {
    rusty_viewer::settings::save_last_batch_output_dir(&options.directory.to_string_lossy());
    tauri::async_runtime::spawn_blocking(move || {
        editor::batch(options, |r| {
            let _ = app.emit("batch-progress", r);
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn startup(launch: State<'_, LaunchState>) -> (Option<String>, String) {
    #[cfg(debug_assertions)]
    eprintln!("RustyViewer: desktop interface initialized");
    let mut launch = launch.0.lock().expect("launch state");
    launch.0 = true;
    (
        launch.1.take().or_else(|| {
            std::env::args_os()
                .nth(1)
                .filter(|p| PathBuf::from(p).is_file())
                .map(|s| s.to_string_lossy().into_owned())
        }),
        rusty_viewer::settings::initial_batch_output_dir()
            .to_string_lossy()
            .into_owned(),
    )
}
#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.state::<Quitting>()
        .0
        .store(true, std::sync::atomic::Ordering::SeqCst);
    app.exit(0);
}
#[derive(Default)]
struct Quitting(std::sync::atomic::AtomicBool);
#[derive(Default)]
struct LaunchState(Mutex<(bool, Option<String>)>);
fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .manage(Quitting::default())
        .manage(LaunchState::default())
        .invoke_handler(tauri::generate_handler![
            open_image,
            adjust_image,
            edit_image,
            export_image,
            copy_image,
            paste_image,
            run_batch,
            startup,
            quit_app
        ])
        .build(tauri::generate_context!())
        .expect("Could not start RustyViewer");
    app.run(|app, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = &event
            && !app
                .state::<Quitting>()
                .0
                .load(std::sync::atomic::Ordering::SeqCst)
        {
            api.prevent_exit();
            let _ = app.emit("request-quit", ());
        }
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        if let tauri::RunEvent::Opened { urls } = event
            && let Some(path) = urls.into_iter().find_map(|u| u.to_file_path().ok())
        {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
            let state = app.state::<LaunchState>();
            let mut launch = state.0.lock().expect("launch state");
            let path = path.to_string_lossy().into_owned();
            if launch.0 {
                let _ = app.emit("open-file", path);
            } else {
                launch.1 = Some(path);
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        let _ = (app, event);
    });
}

#[cfg(test)]
mod ipc_tests {
    use super::*;
    use serde_json::{Value, json};
    use tauri::{
        ipc::{CallbackFn, InvokeBody},
        test::{INVOKE_KEY, get_ipc_response, mock_builder, mock_context, noop_assets},
        webview::InvokeRequest,
    };

    #[test]
    fn frontend_commands_open_adjust_undo_export_and_report_errors() {
        let app = mock_builder()
            .manage(AppState::default())
            .invoke_handler(tauri::generate_handler![
                open_image,
                adjust_image,
                edit_image,
                export_image
            ])
            .build(mock_context(noop_assets()))
            .unwrap();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let call = |command: &str, body: Value| {
            get_ipc_response(
                &window,
                InvokeRequest {
                    cmd: command.into(),
                    callback: CallbackFn(0),
                    error: CallbackFn(1),
                    url: if cfg!(target_os = "windows") {
                        "http://tauri.localhost"
                    } else {
                        "tauri://localhost"
                    }
                    .parse()
                    .unwrap(),
                    body: InvokeBody::Json(body),
                    headers: Default::default(),
                    invoke_key: INVOKE_KEY.into(),
                },
            )
            .map(|r| r.deserialize::<Value>().unwrap())
        };
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("test.png");
        image::DynamicImage::new_rgba8(30, 20).save(&input).unwrap();
        let loaded = call("open_image", json!({"path":input,"discard":false})).unwrap();
        assert_eq!(loaded["width"], 30);
        assert_eq!(loaded["dirty"], false);
        assert!(
            loaded["preview"]
                .as_str()
                .unwrap()
                .starts_with("data:image/png;base64,")
        );
        let adjusted = call("adjust_image", json!({"adjustments":{"brightness":0.2}})).unwrap();
        assert_eq!(adjusted["dirty"], true);
        let rotated = call("edit_image", json!({"action":"rotate_right","args":[]})).unwrap();
        assert_eq!(rotated["width"], 20);
        let undone = call("edit_image", json!({"action":"undo","args":[]})).unwrap();
        assert_eq!(undone["width"], 30);
        let output = dir.path().join("export.jpg");
        call("export_image", json!({"path":output})).unwrap();
        assert!(output.exists());
        assert!(
            call(
                "open_image",
                json!({"path":dir.path().join("missing.png"),"discard":true})
            )
            .is_err()
        );
    }
}
