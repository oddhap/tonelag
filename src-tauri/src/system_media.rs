use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};

use crate::model::{AppSnapshot, PlaybackStatus};

#[derive(Clone, Debug)]
pub struct MediaUpdate {
    title: Option<String>,
    artist: Option<String>,
    duration: Option<Duration>,
    position: Duration,
    status: PlaybackStatus,
}

impl From<&AppSnapshot> for MediaUpdate {
    fn from(snapshot: &AppSnapshot) -> Self {
        let current = snapshot
            .playback
            .current_item_id
            .and_then(|id| snapshot.queue.iter().find(|item| item.id == id));
        Self {
            title: snapshot
                .playback
                .now_playing_title
                .clone()
                .or_else(|| current.map(|item| item.title.clone())),
            artist: current.and_then(|item| item.artist.clone()),
            duration: snapshot.playback.duration_ms.map(Duration::from_millis),
            position: Duration::from_millis(snapshot.playback.position_ms),
            status: snapshot.playback.status,
        }
    }
}

pub fn spawn(hwnd: Option<usize>) -> (Sender<MediaUpdate>, Receiver<MediaControlEvent>) {
    let (update_tx, update_rx) = mpsc::channel::<MediaUpdate>();
    let (event_tx, event_rx) = mpsc::channel();
    thread::Builder::new()
        .name("tonelag-system-media".into())
        .spawn(move || run(hwnd, update_rx, event_tx))
        .expect("failed spawning system media thread");
    (update_tx, event_rx)
}

fn run(hwnd: Option<usize>, updates: Receiver<MediaUpdate>, events: Sender<MediaControlEvent>) {
    let config = PlatformConfig {
        display_name: "Tonelag",
        dbus_name: "no.oddhap.tonelag",
        hwnd: hwnd.map(|value| value as *mut std::ffi::c_void),
    };
    let mut controls = match MediaControls::new(config) {
        Ok(controls) => controls,
        Err(error) => {
            log::warn!("System media controls are unavailable: {error}");
            return;
        }
    };
    if let Err(error) = controls.attach(move |event| {
        let _ = events.send(event);
    }) {
        log::warn!("Could not attach system media commands: {error}");
        return;
    }

    let mut last_metadata: Option<(Option<String>, Option<String>, Option<Duration>)> = None;
    let mut last_playback: Option<(PlaybackStatus, u64)> = None;
    while let Ok(mut update) = updates.recv() {
        while let Ok(newer) = updates.try_recv() {
            update = newer;
        }
        let metadata = (update.title.clone(), update.artist.clone(), update.duration);
        if last_metadata.as_ref() != Some(&metadata) {
            if let Err(error) = controls.set_metadata(MediaMetadata {
                title: update.title.as_deref(),
                artist: update.artist.as_deref(),
                duration: update.duration,
                ..MediaMetadata::default()
            }) {
                log::warn!("Could not update system media metadata: {error}");
            }
            last_metadata = Some(metadata);
        }
        let playback_key = (update.status, update.position.as_secs());
        if last_playback != Some(playback_key) {
            let progress = Some(MediaPosition(update.position));
            let playback = match update.status {
                PlaybackStatus::Playing => MediaPlayback::Playing { progress },
                PlaybackStatus::Paused | PlaybackStatus::Buffering | PlaybackStatus::Loading => {
                    MediaPlayback::Paused { progress }
                }
                PlaybackStatus::Stopped | PlaybackStatus::Error => MediaPlayback::Stopped,
            };
            if let Err(error) = controls.set_playback(playback) {
                log::warn!("Could not update system media playback: {error}");
            }
            last_playback = Some(playback_key);
        }
    }
}
