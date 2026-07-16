use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, anyhow};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::{
    audio::{AudioCommand, AudioController, AudioEvent},
    eqf,
    model::{AppSnapshot, PlaybackStatus, PlayerCommand, QueueItem, QueueOrigin},
    persistence, playlist, skin, skin_catalog,
};

pub struct AppController {
    state: Arc<Mutex<AppSnapshot>>,
    data_dir: PathBuf,
    deferred_saver: persistence::DeferredSaver,
    audio: Mutex<Option<AudioController>>,
    system_media: Mutex<Option<std::sync::mpsc::Sender<crate::system_media::MediaUpdate>>>,
}

impl AppController {
    pub fn new(data_dir: PathBuf) -> Result<Self> {
        let snapshot = persistence::load(&data_dir).unwrap_or_else(|error| {
            log::warn!("Could not restore the previous session: {error:#}");
            AppSnapshot::default()
        });
        let state = Arc::new(Mutex::new(snapshot));
        let deferred_saver = persistence::DeferredSaver::new(data_dir.clone(), state.clone())?;
        Ok(Self {
            state,
            data_dir,
            deferred_saver,
            audio: Mutex::new(None),
            system_media: Mutex::new(None),
        })
    }

    pub fn snapshot(&self) -> AppSnapshot {
        self.state.lock().expect("state lock poisoned").clone()
    }

    pub fn save_before_exit(&self) -> Result<()> {
        persistence::save(&self.data_dir, &self.snapshot())
    }

    pub fn attach_system_media(
        &self,
        sender: std::sync::mpsc::Sender<crate::system_media::MediaUpdate>,
    ) {
        let snapshot = self.snapshot();
        let _ = sender.send((&snapshot).into());
        *self
            .system_media
            .lock()
            .expect("system media lock poisoned") = Some(sender);
    }

    pub fn initialize_audio(&self, app: &AppHandle) -> Result<()> {
        let mut audio = self.audio.lock().expect("audio lock poisoned");
        if audio.is_some() {
            return Ok(());
        }
        let settings = self
            .state
            .lock()
            .expect("state lock poisoned")
            .settings
            .clone();
        let (controller, mut events) =
            AudioController::new(settings.volume, settings.balance, settings.eq)?;
        *audio = Some(controller);
        drop(audio);

        let app_handle = app.clone();
        let event_controller = app.state::<Arc<AppController>>().inner().clone();
        tauri::async_runtime::spawn(async move {
            while let Some(event) = events.recv().await {
                event_controller.handle_audio_event(event, &app_handle);
            }
        });
        Ok(())
    }

    fn send_audio(&self, command: AudioCommand, app: &AppHandle) -> Result<()> {
        self.initialize_audio(app)?;
        self.send_audio_if_initialized(command)
    }

    fn send_audio_if_initialized(&self, command: AudioCommand) -> Result<()> {
        if let Some(audio) = self.audio.lock().expect("audio lock poisoned").as_ref() {
            audio.send(command)?;
        }
        Ok(())
    }

    pub fn add_paths(&self, paths: Vec<String>, app: &AppHandle) -> Result<AppSnapshot> {
        let mut items = Vec::new();
        for raw in paths {
            let path = PathBuf::from(&raw);
            if path.is_dir() {
                collect_audio_files(&path, &mut items)?;
            } else if has_extension(&path, "wsz") || has_extension(&path, "zip") {
                self.import_skin(&path, app)?;
            } else if has_extension(&path, "eqf") {
                self.import_eqf(&path, app)?;
            } else if is_playlist(&path) {
                items.extend(playlist::import(&path)?);
            } else {
                items.push(playlist::entry_to_queue_item(&raw, None));
            }
        }
        let snapshot = self.mutate_persistent(|state| state.queue.extend(items))?;
        emit_snapshot(app, &snapshot);
        Ok(snapshot)
    }

    pub fn add_url(
        &self,
        url: String,
        title: Option<String>,
        app: &AppHandle,
    ) -> Result<AppSnapshot> {
        let mut item = playlist::entry_to_queue_item(&url, None);
        if !matches!(item.origin, QueueOrigin::HttpStream { .. }) {
            return Err(anyhow!("invalid HTTP(S) stream URL"));
        }
        if let Some(title) = title.filter(|value| !value.trim().is_empty()) {
            item.title = title;
        }
        let snapshot = self.mutate_persistent(|state| state.queue.push(item))?;
        emit_snapshot(app, &snapshot);
        Ok(snapshot)
    }

    pub fn remove_items(&self, ids: Vec<Uuid>, app: &AppHandle) -> Result<AppSnapshot> {
        let removing_current = self
            .snapshot()
            .playback
            .current_item_id
            .is_some_and(|id| ids.contains(&id));
        if removing_current {
            self.send_audio_if_initialized(AudioCommand::Stop)?;
        }
        let snapshot = self.mutate_persistent(|state| {
            state.queue.retain(|item| !ids.contains(&item.id));
            if state
                .playback
                .current_item_id
                .is_some_and(|id| ids.contains(&id))
            {
                state.playback.current_item_id = None;
                state.playback.status = PlaybackStatus::Stopped;
            }
        })?;
        emit_snapshot(app, &snapshot);
        Ok(snapshot)
    }

    pub fn clear_queue(&self, app: &AppHandle) -> Result<AppSnapshot> {
        self.send_audio_if_initialized(AudioCommand::Stop)?;
        let snapshot = self.mutate_persistent(|state| {
            state.queue.clear();
            state.playback = Default::default();
        })?;
        emit_snapshot(app, &snapshot);
        Ok(snapshot)
    }

    pub fn reorder(&self, from: usize, to: usize, app: &AppHandle) -> Result<AppSnapshot> {
        let snapshot = self.mutate_persistent(|state| {
            if from < state.queue.len() && to < state.queue.len() && from != to {
                let item = state.queue.remove(from);
                state.queue.insert(to, item);
            }
        })?;
        emit_snapshot(app, &snapshot);
        Ok(snapshot)
    }

    pub fn export_playlist(&self, path: &Path) -> Result<()> {
        let state = self.state.lock().expect("state lock poisoned");
        playlist::export(path, &state.queue)
    }

    pub fn import_eqf(&self, path: &Path, app: &AppHandle) -> Result<AppSnapshot> {
        let eq = eqf::import(path)?;
        self.send_audio_if_initialized(AudioCommand::SetEq(eq.clone()))?;
        let snapshot = self.mutate_persistent(|state| state.settings.eq = eq)?;
        emit_snapshot(app, &snapshot);
        Ok(snapshot)
    }

    pub fn export_eqf(&self, path: &Path) -> Result<()> {
        let state = self.state.lock().expect("state lock poisoned");
        let name = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Preset");
        eqf::export(path, name, &state.settings.eq)
    }

    pub fn import_skin(&self, path: &Path, app: &AppHandle) -> Result<skin::SkinDescriptor> {
        let descriptor = skin::import(path, &self.skins_dir())?;
        self.select_skin(descriptor, app)
    }

    pub fn install_bundled_skin(&self, path: &Path, name: &str) -> Result<skin::SkinDescriptor> {
        let bytes = std::fs::read(path)
            .with_context(|| format!("failed reading bundled skin: {}", path.display()))?;
        skin::install_bytes(name, &bytes, &self.skins_dir())
    }

    pub async fn install_catalog_skin(
        &self,
        md5: &str,
        name: &str,
        app: &AppHandle,
    ) -> Result<skin::SkinDescriptor> {
        let bytes = skin_catalog::download(md5).await?;
        let descriptor = skin::install_bytes(name, &bytes, &self.skins_dir())?;
        self.select_skin(descriptor, app)
    }

    fn select_skin(
        &self,
        descriptor: skin::SkinDescriptor,
        app: &AppHandle,
    ) -> Result<skin::SkinDescriptor> {
        let snapshot = self.mutate_persistent(|state| {
            state.settings.selected_skin = Some(descriptor.id.clone());
        })?;
        emit_snapshot(app, &snapshot);
        let _ = app.emit("skin://changed", &descriptor);
        Ok(descriptor)
    }

    pub fn skin_bytes(&self, id: &str) -> Result<Vec<u8>> {
        skin::read_bytes(&self.skins_dir(), id)
    }

    pub fn list_skins(&self) -> Result<Vec<skin::SkinDescriptor>> {
        skin::list(&self.skins_dir())
    }

    pub fn select_installed_skin(
        &self,
        id: Option<String>,
        app: &AppHandle,
    ) -> Result<AppSnapshot> {
        if let Some(id) = id.as_deref() {
            skin::read_bytes(&self.skins_dir(), id)?;
        }
        let snapshot = self.mutate_persistent(|state| state.settings.selected_skin = id)?;
        emit_snapshot(app, &snapshot);
        Ok(snapshot)
    }

    pub fn player_command(&self, command: PlayerCommand, app: &AppHandle) -> Result<AppSnapshot> {
        match command {
            PlayerCommand::Play => {
                let item = {
                    let mut state = self.state.lock().expect("state lock poisoned");
                    if state.playback.current_item_id.is_none() {
                        state.playback.current_item_id = state.queue.first().map(|item| item.id);
                    }
                    current_item(&state).cloned()
                };
                if let Some(item) = item {
                    if self.snapshot().playback.status == PlaybackStatus::Stopped {
                        self.send_audio(AudioCommand::Load(item), app)?;
                    } else {
                        self.send_audio(AudioCommand::Play, app)?;
                    }
                }
            }
            PlayerCommand::Pause => self.send_audio(AudioCommand::Pause, app)?,
            PlayerCommand::Stop => self.send_audio_if_initialized(AudioCommand::Stop)?,
            PlayerCommand::Load { item_id } => {
                let item = {
                    let mut state = self.state.lock().expect("state lock poisoned");
                    state.playback.current_item_id = Some(item_id);
                    state.queue.iter().find(|item| item.id == item_id).cloned()
                }
                .context("queue item does not exist")?;
                self.send_audio(AudioCommand::Load(item), app)?;
            }
            PlayerCommand::Seek { position_ms } => {
                if self.snapshot().playback.capabilities.seekable {
                    self.send_audio(AudioCommand::Seek(position_ms), app)?;
                    let mut state = self.state.lock().expect("state lock poisoned");
                    state.playback.position_ms = position_ms;
                }
            }
            PlayerCommand::Next => self.load_relative(1)?,
            PlayerCommand::Previous => self.load_relative(-1)?,
            PlayerCommand::SetVolume { value } => {
                self.send_audio_if_initialized(AudioCommand::SetVolume(value))?;
                self.mutate_persistent(|state| state.settings.volume = value.clamp(0.0, 1.0))?;
            }
            PlayerCommand::SetBalance { value } => {
                self.send_audio_if_initialized(AudioCommand::SetBalance(value))?;
                self.mutate_persistent(|state| state.settings.balance = value.clamp(-1.0, 1.0))?;
            }
            PlayerCommand::SetEqEnabled { enabled } => {
                let snapshot =
                    self.mutate_persistent(|state| state.settings.eq.enabled = enabled)?;
                self.send_audio_if_initialized(AudioCommand::SetEq(snapshot.settings.eq))?;
            }
            PlayerCommand::SetPreamp { value_db } => {
                let snapshot = self.mutate_persistent(|state| {
                    state.settings.eq.preamp_db = value_db.clamp(-12.0, 12.0)
                })?;
                self.send_audio_if_initialized(AudioCommand::SetEq(snapshot.settings.eq))?;
            }
            PlayerCommand::SetEqBand { index, value_db } => {
                let snapshot = self.mutate_persistent(|state| {
                    if let Some(band) = state.settings.eq.bands_db.get_mut(index) {
                        *band = value_db.clamp(-12.0, 12.0);
                    }
                })?;
                self.send_audio_if_initialized(AudioCommand::SetEq(snapshot.settings.eq))?;
            }
            PlayerCommand::ToggleShuffle => {
                self.mutate_persistent(|state| state.settings.shuffle = !state.settings.shuffle)?;
            }
            PlayerCommand::ToggleRepeat => {
                self.mutate_persistent(|state| state.settings.repeat = !state.settings.repeat)?;
            }
            PlayerCommand::ToggleDoubleSize => {
                let snapshot = self.mutate_persistent(|state| {
                    state.settings.double_size = !state.settings.double_size
                })?;
                self.apply_window_sizes(app, &snapshot)?;
            }
            PlayerCommand::ToggleAlwaysOnTop => {
                let snapshot = self.mutate_persistent(|state| {
                    state.settings.always_on_top = !state.settings.always_on_top
                })?;
                for label in ["main", "equalizer", "playlist"] {
                    if let Some(window) = app.get_webview_window(label) {
                        window.set_always_on_top(snapshot.settings.always_on_top)?;
                    }
                }
            }
            PlayerCommand::ToggleWinshade => {
                let snapshot = self.mutate_persistent(|state| {
                    state.settings.main_winshade = !state.settings.main_winshade
                })?;
                self.apply_window_sizes(app, &snapshot)?;
            }
            PlayerCommand::SetLanguage { language } => {
                let language = if language == "nb" || language == "no" {
                    "nb"
                } else {
                    "en"
                };
                self.mutate_persistent(|state| state.settings.language = language.into())?;
            }
        }

        let snapshot = self.mutate_persistent(|_| {})?;
        emit_snapshot(app, &snapshot);
        self.update_system_media(&snapshot);
        Ok(snapshot)
    }

    pub fn handle_audio_event(&self, event: AudioEvent, app: &AppHandle) {
        let mut load_next = false;
        let queue_changed = matches!(&event, AudioEvent::Loaded { .. });
        let media_changed = !matches!(&event, AudioEvent::Position(_) | AudioEvent::Spectrum(_));
        let (playback, queue) = {
            let mut state = self.state.lock().expect("state lock poisoned");
            match event {
                AudioEvent::Loading => {
                    state.playback.status = PlaybackStatus::Loading;
                    state.playback.error = None;
                    state.playback.duration_ms = None;
                    state.playback.bitrate_kbps = None;
                    state.playback.sample_rate_hz = None;
                    state.playback.channels = None;
                    state.playback.now_playing_title = None;
                }
                AudioEvent::Buffering => state.playback.status = PlaybackStatus::Buffering,
                AudioEvent::Loaded {
                    duration_ms,
                    bitrate_kbps,
                    sample_rate_hz,
                    channels,
                    title,
                    artist,
                    capabilities,
                } => {
                    state.playback.duration_ms = duration_ms;
                    state.playback.bitrate_kbps = bitrate_kbps;
                    state.playback.sample_rate_hz = Some(sample_rate_hz);
                    state.playback.channels = Some(channels);
                    state.playback.capabilities = capabilities;
                    state.playback.position_ms = 0;
                    if let Some(item) = state
                        .playback
                        .current_item_id
                        .and_then(|id| state.queue.iter_mut().find(|item| item.id == id))
                    {
                        item.duration_ms = duration_ms;
                        if let Some(title) = title {
                            item.title = title;
                        }
                        if artist.is_some() {
                            item.artist = artist;
                        }
                    }
                }
                AudioEvent::Playing => state.playback.status = PlaybackStatus::Playing,
                AudioEvent::Paused => state.playback.status = PlaybackStatus::Paused,
                AudioEvent::Stopped => {
                    state.playback.status = PlaybackStatus::Stopped;
                    state.playback.position_ms = 0;
                }
                AudioEvent::Position(position) => state.playback.position_ms = position,
                AudioEvent::Spectrum(spectrum) => state.playback.spectrum = spectrum,
                AudioEvent::Metadata(title) => state.playback.now_playing_title = Some(title),
                AudioEvent::Ended => load_next = true,
                AudioEvent::Error(error) => {
                    state.playback.status = PlaybackStatus::Error;
                    state.playback.error = Some(error);
                }
            }
            state.revision += 1;
            state.playback.revision = state.revision;
            let queue = queue_changed.then(|| state.queue.clone());
            (state.playback.clone(), queue)
        };
        let _ = app.emit("player://snapshot", &playback);
        if let Some(queue) = queue {
            let _ = app.emit_to("playlist", "queue://snapshot", &queue);
            if self
                .state
                .lock()
                .expect("state lock poisoned")
                .layout
                .combined
            {
                let _ = app.emit_to("main", "queue://snapshot", &queue);
            }
        }
        if media_changed {
            self.update_system_media(&self.snapshot());
        }
        if load_next && let Err(error) = self.advance_after_end() {
            log::warn!("Could not advance to next item: {error:#}");
        }
    }

    pub fn emit_position(&self, app: &AppHandle) {
        if self
            .state
            .lock()
            .expect("state lock poisoned")
            .playback
            .status
            != PlaybackStatus::Playing
        {
            return;
        }
        let position = self
            .audio
            .lock()
            .expect("audio lock poisoned")
            .as_ref()
            .map(AudioController::position_ms);
        if let Some(position) = position {
            self.handle_audio_event(AudioEvent::Position(position), app);
        }
    }

    pub fn configure_windows(&self, app: &AppHandle) -> Result<()> {
        let wayland = cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some();
        let main = app
            .get_webview_window("main")
            .context("main window missing")?;
        if wayland {
            main.set_size(tauri::LogicalSize::new(275.0, 464.0))?;
            let snapshot = self.mutate_persistent(|state| state.layout.combined = true)?;
            emit_snapshot(app, &snapshot);
        } else {
            let snapshot = self.snapshot();
            main.set_position(tauri::LogicalPosition::new(
                snapshot.layout.main.x,
                snapshot.layout.main.y,
            ))?;
        }
        let snapshot = self.snapshot();
        self.apply_window_sizes(app, &snapshot)?;
        for label in ["main", "equalizer", "playlist"] {
            if let Some(window) = app.get_webview_window(label) {
                window.set_always_on_top(snapshot.settings.always_on_top)?;
            }
        }
        Ok(())
    }

    pub fn restore_visible_windows(&self, app: &AppHandle) -> Result<()> {
        let snapshot = self.snapshot();
        if snapshot.layout.combined {
            return Ok(());
        }
        if snapshot.layout.equalizer_visible {
            self.ensure_panel_window(app, "equalizer")?.show()?;
        }
        if snapshot.layout.playlist_visible {
            self.ensure_panel_window(app, "playlist")?.show()?;
        }
        refresh_native_window_group(
            app,
            &snapshot.layout,
            snapshot.settings.double_size,
            snapshot.settings.main_winshade,
        );
        Ok(())
    }

    fn apply_window_sizes(&self, app: &AppHandle, snapshot: &AppSnapshot) -> Result<()> {
        let scale = if snapshot.settings.double_size {
            2.0
        } else {
            1.0
        };
        if snapshot.layout.combined {
            let main_height = if snapshot.settings.main_winshade {
                14.0
            } else {
                116.0
            };
            let main = app
                .get_webview_window("main")
                .context("main window missing")?;
            main.set_size(tauri::LogicalSize::new(
                275.0 * scale,
                (main_height + 348.0) * scale,
            ))?;
        } else {
            if let Some(main) = app.get_webview_window("main") {
                let height = if snapshot.settings.main_winshade {
                    14.0
                } else {
                    116.0
                };
                main.set_size(tauri::LogicalSize::new(275.0 * scale, height * scale))?;
            }
            if let Some(equalizer) = app.get_webview_window("equalizer") {
                equalizer.set_size(tauri::LogicalSize::new(275.0 * scale, 116.0 * scale))?;
            }
            if let Some(playlist) = app.get_webview_window("playlist") {
                playlist.set_size(tauri::LogicalSize::new(
                    275.0 * scale,
                    snapshot.layout.playlist_height.max(116.0) * scale,
                ))?;
            }
        }
        Ok(())
    }

    pub fn set_panel_visible(&self, app: &AppHandle, panel: &str, visible: bool) -> Result<()> {
        if !matches!(panel, "equalizer" | "playlist" | "skins") {
            return Err(anyhow!("unknown panel"));
        }
        if panel == "skins" {
            if visible {
                let window = self.ensure_panel_window(app, panel)?;
                window.show()?;
                window.set_focus()?;
                let _ = app.emit_to("skins", "skin-browser://opened", ());
            } else if let Some(window) = app.get_webview_window(panel) {
                window.hide()?;
            }
            return Ok(());
        }
        if self.snapshot().layout.combined {
            return Ok(());
        }
        if visible {
            let window = self.ensure_panel_window(app, panel)?;
            window.show()?;
            window.set_focus()?;
        } else if let Some(window) = app.get_webview_window(panel) {
            window.hide()?;
        }
        let snapshot = self.mutate_persistent(|state| match panel {
            "equalizer" => state.layout.equalizer_visible = visible,
            "playlist" => state.layout.playlist_visible = visible,
            _ => {}
        })?;
        emit_snapshot(app, &snapshot);
        let _ = app.emit("layout://changed", &snapshot.layout);
        refresh_native_window_group(
            app,
            &snapshot.layout,
            snapshot.settings.double_size,
            snapshot.settings.main_winshade,
        );
        Ok(())
    }

    pub fn handle_window_moved(
        &self,
        app: &AppHandle,
        label: &str,
        position: tauri::PhysicalPosition<i32>,
        scale_factor: f64,
    ) {
        if !matches!(label, "main" | "equalizer" | "playlist") {
            return;
        }
        let position = position.to_logical::<f64>(scale_factor);
        let (previous, double_size, main_winshade) = {
            let state = self.state.lock().expect("state lock poisoned");
            (
                state.layout.clone(),
                state.settings.double_size,
                state.settings.main_winshade,
            )
        };
        if previous.combined {
            return;
        }
        let old = match label {
            "main" => &previous.main,
            "equalizer" => &previous.equalizer,
            "playlist" => &previous.playlist,
            _ => return,
        };
        if (old.x - position.x).abs() < 0.1 && (old.y - position.y).abs() < 0.1 {
            return;
        }

        let scale = if double_size { 2.0 } else { 1.0 };
        let main_height = if main_winshade { 14.0 } else { 116.0 } * scale;
        let eq_height = 116.0 * scale;
        let width = 275.0 * scale;
        let mut target = crate::model::WindowPoint {
            x: position.x,
            y: position.y,
        };
        let mut grouped_eq = None;
        let mut grouped_playlist = None;

        if label == "main" {
            let delta_x = target.x - previous.main.x;
            let delta_y = target.y - previous.main.y;
            if attached_below(&previous.main, main_height, &previous.equalizer) {
                grouped_eq = Some(crate::model::WindowPoint {
                    x: previous.equalizer.x + delta_x,
                    y: previous.equalizer.y + delta_y,
                });
            }
            let playlist_anchor = grouped_eq.as_ref().unwrap_or(&previous.equalizer);
            if attached_below(&previous.equalizer, eq_height, &previous.playlist) {
                grouped_playlist = Some(crate::model::WindowPoint {
                    x: playlist_anchor.x,
                    y: playlist_anchor.y + eq_height,
                });
            }
        } else {
            let anchors = if label == "equalizer" {
                vec![
                    (&previous.main, main_height),
                    (&previous.playlist, previous.playlist_height * scale),
                ]
            } else {
                vec![
                    (&previous.main, main_height),
                    (&previous.equalizer, eq_height),
                ]
            };
            target = snap_to_windows(
                target,
                width,
                if label == "playlist" {
                    previous.playlist_height * scale
                } else {
                    eq_height
                },
                &anchors,
            );
        }

        let layout = {
            let mut state = self.state.lock().expect("state lock poisoned");
            match label {
                "main" => state.layout.main = target,
                "equalizer" => state.layout.equalizer = target,
                "playlist" => state.layout.playlist = target,
                _ => {}
            }
            if let Some(point) = grouped_eq {
                state.layout.equalizer = point;
            }
            if let Some(point) = grouped_playlist {
                state.layout.playlist = point;
            }
            state.revision += 1;
            state.playback.revision = state.revision;
            state.layout.clone()
        };
        if let Err(error) = self.deferred_saver.schedule() {
            log::warn!("Could not schedule window position save: {error:#}");
        }

        if ((target.x - position.x).abs() >= 0.1 || (target.y - position.y).abs() >= 0.1)
            && let Some(window) = app.get_webview_window(label)
        {
            let _ = window.set_position(tauri::LogicalPosition::new(target.x, target.y));
        }
        for (window_label, point) in [("equalizer", grouped_eq), ("playlist", grouped_playlist)] {
            if let Some(point) = point
                && let Some(window) = app.get_webview_window(window_label)
            {
                #[cfg(not(target_os = "macos"))]
                let _ = window.set_position(tauri::LogicalPosition::new(point.x, point.y));
                #[cfg(target_os = "macos")]
                let _ = (window, point);
            }
        }
        let _ = app.emit("layout://changed", &layout);
        if label != "main" {
            refresh_native_window_group(app, &layout, double_size, main_winshade);
        }
    }

    pub fn handle_playlist_resized(&self, app: &AppHandle, height: u32, scale_factor: f64) {
        let (combined, double_size, playlist_height) = {
            let state = self.state.lock().expect("state lock poisoned");
            (
                state.layout.combined,
                state.settings.double_size,
                state.layout.playlist_height,
            )
        };
        if combined {
            return;
        }
        let interface_scale = if double_size { 2.0 } else { 1.0 };
        let logical_height = f64::from(height) / scale_factor / interface_scale;
        if (logical_height - playlist_height).abs() < 0.5 {
            return;
        }
        let layout = {
            let mut state = self.state.lock().expect("state lock poisoned");
            state.layout.playlist_height = logical_height.max(116.0);
            state.revision += 1;
            state.playback.revision = state.revision;
            state.layout.clone()
        };
        if let Err(error) = self.deferred_saver.schedule() {
            log::warn!("Could not schedule playlist size save: {error:#}");
        }
        let _ = app.emit("layout://changed", &layout);
    }

    fn load_relative(&self, offset: isize) -> Result<()> {
        let item = {
            let mut state = self.state.lock().expect("state lock poisoned");
            if state.queue.is_empty() {
                return Ok(());
            }
            let current = state
                .playback
                .current_item_id
                .and_then(|id| state.queue.iter().position(|item| item.id == id))
                .unwrap_or(0);
            let next = if state.settings.shuffle && state.queue.len() > 1 {
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .subsec_nanos() as usize;
                let candidate = nanos % (state.queue.len() - 1);
                if candidate >= current {
                    candidate + 1
                } else {
                    candidate
                }
            } else if offset.is_negative() {
                current
                    .checked_sub(offset.unsigned_abs())
                    .unwrap_or(state.queue.len() - 1)
            } else {
                (current + offset as usize) % state.queue.len()
            };
            let item = state.queue[next].clone();
            state.playback.current_item_id = Some(item.id);
            item
        };
        self.send_audio_if_initialized(AudioCommand::Load(item))
    }

    fn advance_after_end(&self) -> Result<()> {
        let should_stop = {
            let state = self.state.lock().expect("state lock poisoned");
            let current = state
                .playback
                .current_item_id
                .and_then(|id| state.queue.iter().position(|item| item.id == id));
            !state.settings.repeat
                && !state.settings.shuffle
                && current.is_some_and(|index| index + 1 >= state.queue.len())
        };
        if should_stop {
            self.send_audio_if_initialized(AudioCommand::Stop)
        } else {
            self.load_relative(1)
        }
    }

    fn mutate_persistent(&self, mutation: impl FnOnce(&mut AppSnapshot)) -> Result<AppSnapshot> {
        let snapshot = {
            let mut state = self.state.lock().expect("state lock poisoned");
            mutation(&mut state);
            state.revision += 1;
            state.playback.revision = state.revision;
            state.clone()
        };
        self.deferred_saver.schedule()?;
        Ok(snapshot)
    }

    fn skins_dir(&self) -> PathBuf {
        self.data_dir.join("skins")
    }

    fn ensure_panel_window(&self, app: &AppHandle, panel: &str) -> Result<tauri::WebviewWindow> {
        if let Some(window) = app.get_webview_window(panel) {
            return Ok(window);
        }
        let snapshot = self.snapshot();
        let (title, width, height, min_height, resizable, transparent) = match panel {
            "equalizer" => ("Tonelag Equalizer", 275.0, 116.0, 116.0, false, true),
            "playlist" => (
                "Tonelag Playlist",
                275.0,
                snapshot.layout.playlist_height.max(116.0),
                116.0,
                true,
                true,
            ),
            "skins" => ("Tonelag Skin Browser", 620.0, 520.0, 380.0, true, false),
            _ => return Err(anyhow!("unknown panel")),
        };
        let mut builder = tauri::WebviewWindowBuilder::new(
            app,
            panel,
            tauri::WebviewUrl::App(format!("index.html?panel={panel}").into()),
        )
        .title(title)
        .inner_size(width, height)
        .min_inner_size(if panel == "skins" { 480.0 } else { 275.0 }, min_height)
        .resizable(resizable)
        .decorations(false)
        .transparent(transparent)
        .shadow(false)
        .visible(false)
        .skip_taskbar(true);
        if panel == "skins" {
            builder = builder.center();
        }
        let window = builder.build()?;
        if panel != "skins" {
            let point = if panel == "equalizer" {
                &snapshot.layout.equalizer
            } else {
                &snapshot.layout.playlist
            };
            window.set_position(tauri::LogicalPosition::new(point.x, point.y))?;
            window.set_always_on_top(snapshot.settings.always_on_top)?;
        }
        Ok(window)
    }

    fn update_system_media(&self, snapshot: &AppSnapshot) {
        if let Some(sender) = self
            .system_media
            .lock()
            .expect("system media lock poisoned")
            .as_ref()
        {
            let _ = sender.send(snapshot.into());
        }
    }
}

fn current_item(state: &AppSnapshot) -> Option<&QueueItem> {
    let id = state.playback.current_item_id?;
    state.queue.iter().find(|item| item.id == id)
}

fn emit_snapshot(app: &AppHandle, snapshot: &AppSnapshot) {
    let lightweight = AppSnapshot {
        revision: snapshot.revision,
        queue: snapshot
            .playback
            .current_item_id
            .and_then(|id| snapshot.queue.iter().find(|item| item.id == id))
            .cloned()
            .into_iter()
            .collect(),
        playback: snapshot.playback.clone(),
        settings: snapshot.settings.clone(),
        layout: snapshot.layout.clone(),
    };
    for label in ["equalizer", "skins"] {
        let _ = app.emit_to(label, "app://snapshot", &lightweight);
    }
    if snapshot.layout.combined {
        let _ = app.emit_to("main", "app://snapshot", snapshot);
    } else {
        let _ = app.emit_to("main", "app://snapshot", &lightweight);
        let _ = app.emit_to("playlist", "app://snapshot", snapshot);
    }
    let _ = app.emit("player://snapshot", &snapshot.playback);
}

fn is_playlist(path: &Path) -> bool {
    ["m3u", "m3u8", "pls"]
        .into_iter()
        .any(|extension| has_extension(path, extension))
}

fn has_extension(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(expected))
}

fn collect_audio_files(directory: &Path, output: &mut Vec<QueueItem>) -> Result<()> {
    let mut entries = std::fs::read_dir(directory)
        .with_context(|| format!("failed reading directory: {}", directory.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_audio_files(&path, output)?;
        } else if is_supported_audio(&path) {
            output.push(playlist::entry_to_queue_item(&path.to_string_lossy(), None));
        }
    }
    Ok(())
}

fn is_supported_audio(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "mp2"
            | "mp3"
            | "aac"
            | "m4a"
            | "flac"
            | "alac"
            | "wma"
            | "ape"
            | "wv"
            | "mpc"
            | "ogg"
            | "opus"
            | "wav"
            | "aiff"
            | "mod"
            | "xm"
            | "s3m"
            | "it"
    )
}

const SNAP_DISTANCE: f64 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlaylistParent {
    Main,
    Equalizer,
}

fn native_group_edges(
    layout: &crate::model::WindowLayout,
    double_size: bool,
    main_winshade: bool,
) -> (bool, Option<PlaylistParent>) {
    let scale = if double_size { 2.0 } else { 1.0 };
    let main_height = if main_winshade { 14.0 } else { 116.0 } * scale;
    let equalizer_height = 116.0 * scale;
    let equalizer_to_main =
        layout.equalizer_visible && attached_below(&layout.main, main_height, &layout.equalizer);
    let playlist_parent = if !layout.playlist_visible {
        None
    } else if layout.equalizer_visible
        && attached_below(&layout.equalizer, equalizer_height, &layout.playlist)
    {
        Some(PlaylistParent::Equalizer)
    } else if attached_below(&layout.main, main_height, &layout.playlist) {
        Some(PlaylistParent::Main)
    } else {
        None
    };
    (equalizer_to_main, playlist_parent)
}

#[cfg(target_os = "macos")]
fn refresh_native_window_group(
    app: &AppHandle,
    layout: &crate::model::WindowLayout,
    double_size: bool,
    main_winshade: bool,
) {
    use objc2_app_kit::{NSWindow, NSWindowOrderingMode};

    let Some(main) = app.get_webview_window("main") else {
        return;
    };
    let Some(equalizer) = app.get_webview_window("equalizer") else {
        return;
    };
    let Some(playlist) = app.get_webview_window("playlist") else {
        return;
    };
    let Ok(main_pointer) = main.ns_window() else {
        return;
    };
    let Ok(equalizer_pointer) = equalizer.ns_window() else {
        return;
    };
    let Ok(playlist_pointer) = playlist.ns_window() else {
        return;
    };
    let main_pointer = main_pointer as usize;
    let equalizer_pointer = equalizer_pointer as usize;
    let playlist_pointer = playlist_pointer as usize;
    let (equalizer_to_main, playlist_parent) =
        native_group_edges(layout, double_size, main_winshade);
    let _ = app.run_on_main_thread(move || unsafe {
        let main = &*(main_pointer as *const NSWindow);
        let equalizer = &*(equalizer_pointer as *const NSWindow);
        let playlist = &*(playlist_pointer as *const NSWindow);

        if let Some(parent) = equalizer.parentWindow() {
            parent.removeChildWindow(equalizer);
        }
        if let Some(parent) = playlist.parentWindow() {
            parent.removeChildWindow(playlist);
        }
        if equalizer_to_main {
            main.addChildWindow_ordered(equalizer, NSWindowOrderingMode::Above);
        }
        match playlist_parent {
            Some(PlaylistParent::Main) => {
                main.addChildWindow_ordered(playlist, NSWindowOrderingMode::Above)
            }
            Some(PlaylistParent::Equalizer) => {
                equalizer.addChildWindow_ordered(playlist, NSWindowOrderingMode::Above)
            }
            None => {}
        }
    });
}

#[cfg(not(target_os = "macos"))]
fn refresh_native_window_group(
    _app: &AppHandle,
    _layout: &crate::model::WindowLayout,
    _double_size: bool,
    _main_winshade: bool,
) {
}

fn attached_below(
    top: &crate::model::WindowPoint,
    top_height: f64,
    bottom: &crate::model::WindowPoint,
) -> bool {
    (top.x - bottom.x).abs() <= SNAP_DISTANCE
        && (top.y + top_height - bottom.y).abs() <= SNAP_DISTANCE
}

fn snap_to_windows(
    mut target: crate::model::WindowPoint,
    width: f64,
    height: f64,
    anchors: &[(&crate::model::WindowPoint, f64)],
) -> crate::model::WindowPoint {
    let original = target;
    let mut best_x = SNAP_DISTANCE + 1.0;
    let mut best_y = SNAP_DISTANCE + 1.0;
    for (anchor, anchor_height) in anchors {
        for candidate in [anchor.x, anchor.x + width, anchor.x - width] {
            let distance = (original.x - candidate).abs();
            if distance <= SNAP_DISTANCE && distance < best_x {
                target.x = candidate;
                best_x = distance;
            }
        }
        for candidate in [anchor.y, anchor.y + anchor_height, anchor.y - height] {
            let distance = (original.y - candidate).abs();
            if distance <= SNAP_DISTANCE && distance < best_y {
                target.y = candidate;
                best_y = distance;
            }
        }
    }
    target
}

#[cfg(test)]
mod window_tests {
    use super::*;

    #[test]
    fn snaps_panel_below_another_panel() {
        let anchor = crate::model::WindowPoint { x: 100.0, y: 100.0 };
        let result = snap_to_windows(
            crate::model::WindowPoint { x: 106.0, y: 222.0 },
            275.0,
            116.0,
            &[(&anchor, 116.0)],
        );
        assert_eq!(result, crate::model::WindowPoint { x: 100.0, y: 216.0 });
    }

    #[test]
    fn recognizes_associated_files_case_insensitively() {
        assert!(is_playlist(Path::new("MIX.M3U8")));
        assert!(has_extension(Path::new("theme.WSZ"), "wsz"));
        assert!(has_extension(Path::new("preset.EQF"), "eqf"));
    }

    #[test]
    fn builds_native_chain_for_a_classic_three_window_stack() {
        let layout = crate::model::WindowLayout::default();
        assert_eq!(
            native_group_edges(&layout, false, false),
            (true, Some(PlaylistParent::Equalizer))
        );
    }
}
