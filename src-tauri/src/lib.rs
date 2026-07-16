mod audio;
mod controller;
mod eqf;
mod http_io;
mod model;
mod persistence;
mod playlist;
mod privacy;
mod skin;
mod skin_catalog;
mod stream;
mod system_media;

use std::{path::PathBuf, sync::Arc, time::Duration};

use controller::AppController;
use model::{AppSnapshot, PlayerCommand};
use skin::SkinDescriptor;
use skin_catalog::SkinCatalogPage;
use stream::ResolvedStream;
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

type CommandResult<T> = Result<T, String>;

#[tauri::command]
fn get_snapshot(
    window: tauri::WebviewWindow,
    controller: State<'_, Arc<AppController>>,
) -> AppSnapshot {
    snapshot_for_window(controller.snapshot(), window.label())
}

#[tauri::command]
fn player_command(
    command: PlayerCommand,
    controller: State<'_, Arc<AppController>>,
    app: AppHandle,
    window: tauri::WebviewWindow,
) -> CommandResult<AppSnapshot> {
    controller
        .player_command(command, &app)
        .map(|snapshot| snapshot_for_window(snapshot, window.label()))
        .map_err(format_error)
}

#[tauri::command]
fn queue_add_paths(
    paths: Vec<String>,
    controller: State<'_, Arc<AppController>>,
    app: AppHandle,
) -> CommandResult<AppSnapshot> {
    controller.add_paths(paths, &app).map_err(format_error)
}

#[tauri::command]
fn queue_add_url(
    url: String,
    title: Option<String>,
    controller: State<'_, Arc<AppController>>,
    app: AppHandle,
) -> CommandResult<AppSnapshot> {
    controller.add_url(url, title, &app).map_err(format_error)
}

#[tauri::command]
fn queue_remove(
    ids: Vec<Uuid>,
    controller: State<'_, Arc<AppController>>,
    app: AppHandle,
) -> CommandResult<AppSnapshot> {
    controller.remove_items(ids, &app).map_err(format_error)
}

#[tauri::command]
fn queue_clear(
    controller: State<'_, Arc<AppController>>,
    app: AppHandle,
) -> CommandResult<AppSnapshot> {
    controller.clear_queue(&app).map_err(format_error)
}

#[tauri::command]
fn queue_reorder(
    from: usize,
    to: usize,
    controller: State<'_, Arc<AppController>>,
    app: AppHandle,
) -> CommandResult<AppSnapshot> {
    controller.reorder(from, to, &app).map_err(format_error)
}

#[tauri::command]
fn playlist_export(path: String, controller: State<'_, Arc<AppController>>) -> CommandResult<()> {
    controller
        .export_playlist(PathBuf::from(path).as_path())
        .map_err(format_error)
}

#[tauri::command]
fn eqf_import(
    path: String,
    controller: State<'_, Arc<AppController>>,
    app: AppHandle,
    window: tauri::WebviewWindow,
) -> CommandResult<AppSnapshot> {
    controller
        .import_eqf(PathBuf::from(path).as_path(), &app)
        .map(|snapshot| snapshot_for_window(snapshot, window.label()))
        .map_err(format_error)
}

#[tauri::command]
fn eqf_export(path: String, controller: State<'_, Arc<AppController>>) -> CommandResult<()> {
    controller
        .export_eqf(PathBuf::from(path).as_path())
        .map_err(format_error)
}

#[tauri::command]
fn skin_import(
    path: String,
    controller: State<'_, Arc<AppController>>,
    app: AppHandle,
) -> CommandResult<SkinDescriptor> {
    controller
        .import_skin(PathBuf::from(path).as_path(), &app)
        .map_err(format_error)
}

#[tauri::command]
fn skin_bytes(id: String, controller: State<'_, Arc<AppController>>) -> CommandResult<Vec<u8>> {
    controller.skin_bytes(&id).map_err(format_error)
}

#[tauri::command]
fn skin_list(controller: State<'_, Arc<AppController>>) -> CommandResult<Vec<SkinDescriptor>> {
    controller.list_skins().map_err(format_error)
}

#[tauri::command]
fn skin_select(
    id: Option<String>,
    controller: State<'_, Arc<AppController>>,
    app: AppHandle,
    window: tauri::WebviewWindow,
) -> CommandResult<AppSnapshot> {
    controller
        .select_installed_skin(id, &app)
        .map(|snapshot| snapshot_for_window(snapshot, window.label()))
        .map_err(format_error)
}

#[tauri::command]
async fn skin_catalog_browse(
    query: Option<String>,
    offset: u32,
    limit: u32,
) -> CommandResult<SkinCatalogPage> {
    skin_catalog::browse(query, offset, limit)
        .await
        .map_err(format_error)
}

#[tauri::command]
async fn skin_catalog_install(
    md5: String,
    name: String,
    controller: State<'_, Arc<AppController>>,
    app: AppHandle,
) -> CommandResult<SkinDescriptor> {
    controller
        .install_catalog_skin(&md5, &name, &app)
        .await
        .map_err(format_error)
}

#[tauri::command]
async fn resolve_stream(url: String) -> CommandResult<ResolvedStream> {
    stream::resolve(&url).await.map_err(format_error)
}

#[tauri::command]
fn set_panel_visible(
    app: AppHandle,
    panel: String,
    visible: bool,
    controller: State<'_, Arc<AppController>>,
) -> CommandResult<()> {
    controller
        .set_panel_visible(&app, &panel, visible)
        .map_err(format_error)
}

#[tauri::command]
fn frontend_ready(app: AppHandle, controller: State<'_, Arc<AppController>>) -> CommandResult<()> {
    controller
        .restore_visible_windows(&app)
        .map_err(format_error)?;
    let audio_controller = controller.inner().clone();
    let audio_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) = audio_controller.initialize_audio(&audio_app) {
            log::warn!("Could not initialize audio after startup: {error:#}");
        }
    });
    Ok(())
}

#[tauri::command]
fn quit_app(app: AppHandle, controller: State<'_, Arc<AppController>>) -> CommandResult<()> {
    controller.save_before_exit().map_err(format_error)?;
    app.exit(0);
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let paths = args
                .into_iter()
                .skip(1)
                .filter(|arg| !arg.starts_with('-'))
                .collect::<Vec<_>>();
            if !paths.is_empty() {
                let controller = app.state::<Arc<AppController>>();
                if let Err(error) = controller.add_paths(paths, app) {
                    log::warn!("Could not open files from second instance: {error:#}");
                }
            }
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .on_window_event(|window, event| {
            let Some(controller) = window.app_handle().try_state::<Arc<AppController>>() else {
                return;
            };
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } if window.label() == "skins" => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                tauri::WindowEvent::Moved(position) => {
                    let scale = window.scale_factor().unwrap_or(1.0);
                    controller.handle_window_moved(
                        window.app_handle(),
                        window.label(),
                        *position,
                        scale,
                    );
                }
                tauri::WindowEvent::Resized(size) if window.label() == "playlist" => {
                    let scale = window.scale_factor().unwrap_or(1.0);
                    controller.handle_playlist_resized(window.app_handle(), size.height, scale);
                }
                _ => {}
            }
        })
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let controller = Arc::new(AppController::new(data_dir)?);
            let bundled_skin = app
                .path()
                .resource_dir()?
                .join("default-skin")
                .join("pastellplate.wsz");
            if bundled_skin.exists()
                && let Err(error) = controller.install_bundled_skin(&bundled_skin, "Pastellplate")
            {
                log::warn!("Could not install bundled Pastellplate skin: {error:#}");
            }
            app.manage(controller.clone());
            controller.configure_windows(app.handle())?;
            let initial_paths = std::env::args()
                .skip(1)
                .filter(|argument| !argument.starts_with('-'))
                .collect::<Vec<_>>();
            if !initial_paths.is_empty() {
                controller.add_paths(initial_paths, app.handle())?;
            }

            #[cfg(target_os = "windows")]
            let media_hwnd = app
                .get_webview_window("main")
                .and_then(|window| window.hwnd().ok())
                .map(|handle| handle.0 as usize);
            #[cfg(not(target_os = "windows"))]
            let media_hwnd = None;
            let (media_updates, media_events) = system_media::spawn(media_hwnd);
            controller.attach_system_media(media_updates);

            let media_app = app.handle().clone();
            let media_controller = controller.clone();
            std::thread::Builder::new()
                .name("classic-system-media-events".into())
                .spawn(move || {
                    while let Ok(event) = media_events.recv() {
                        let command = match event {
                            souvlaki::MediaControlEvent::Play => Some(PlayerCommand::Play),
                            souvlaki::MediaControlEvent::Pause => Some(PlayerCommand::Pause),
                            souvlaki::MediaControlEvent::Toggle => {
                                if media_controller.snapshot().playback.status
                                    == model::PlaybackStatus::Playing
                                {
                                    Some(PlayerCommand::Pause)
                                } else {
                                    Some(PlayerCommand::Play)
                                }
                            }
                            souvlaki::MediaControlEvent::Next => Some(PlayerCommand::Next),
                            souvlaki::MediaControlEvent::Previous => Some(PlayerCommand::Previous),
                            souvlaki::MediaControlEvent::Stop => Some(PlayerCommand::Stop),
                            _ => None,
                        };
                        if let Some(command) = command
                            && let Err(error) = media_controller.player_command(command, &media_app)
                        {
                            log::warn!("Could not handle system media command: {error:#}");
                        }
                    }
                })?;

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut ticker = tokio::time::interval(Duration::from_millis(100));
                loop {
                    ticker.tick().await;
                    let controller = app_handle.state::<Arc<AppController>>();
                    controller.emit_position(&app_handle);
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            player_command,
            queue_add_paths,
            queue_add_url,
            queue_remove,
            queue_clear,
            queue_reorder,
            playlist_export,
            eqf_import,
            eqf_export,
            skin_import,
            skin_bytes,
            skin_list,
            skin_select,
            skin_catalog_browse,
            skin_catalog_install,
            resolve_stream,
            set_panel_visible,
            frontend_ready,
            quit_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tonelag");
}

fn format_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn snapshot_for_window(mut snapshot: AppSnapshot, label: &str) -> AppSnapshot {
    if label != "playlist" && !(label == "main" && snapshot.layout.combined) {
        let current = snapshot.playback.current_item_id;
        snapshot.queue.retain(|item| Some(item.id) == current);
    }
    snapshot
}
