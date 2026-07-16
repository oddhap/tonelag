use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

pub const EQ_FREQUENCIES: [u32; 10] = [
    60, 170, 310, 600, 1_000, 3_000, 6_000, 12_000, 14_000, 16_000,
];

#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", tag = "kind")]
#[ts(export)]
pub enum QueueOrigin {
    LocalFile { path: String },
    HttpStream { url: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct QueueItem {
    pub id: Uuid,
    pub origin: QueueOrigin,
    pub title: String,
    pub artist: Option<String>,
    #[ts(type = "number | null")]
    pub duration_ms: Option<u64>,
    pub available: bool,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PlaybackStatus {
    #[default]
    Stopped,
    Loading,
    Playing,
    Paused,
    Buffering,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SourceCapabilities {
    pub seekable: bool,
    pub live: bool,
    pub dsp_allowed: bool,
    pub analysis_allowed: bool,
    pub supports_metadata_updates: bool,
}

impl Default for SourceCapabilities {
    fn default() -> Self {
        Self {
            seekable: false,
            live: false,
            dsp_allowed: true,
            analysis_allowed: true,
            supports_metadata_updates: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EqSettings {
    pub enabled: bool,
    pub preamp_db: f32,
    #[ts(type = "Array<number>")]
    pub bands_db: [f32; 10],
}

impl Default for EqSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            preamp_db: 0.0,
            bands_db: [0.0; 10],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlaybackSnapshot {
    #[ts(type = "number")]
    pub revision: u64,
    pub status: PlaybackStatus,
    pub current_item_id: Option<Uuid>,
    #[ts(type = "number")]
    pub position_ms: u64,
    #[ts(type = "number | null")]
    pub duration_ms: Option<u64>,
    #[ts(type = "number | null")]
    pub bitrate_kbps: Option<u32>,
    #[ts(type = "number | null")]
    pub sample_rate_hz: Option<u32>,
    #[ts(type = "number | null")]
    pub channels: Option<u16>,
    pub buffered_percent: Option<f32>,
    pub now_playing_title: Option<String>,
    pub error: Option<String>,
    pub capabilities: SourceCapabilities,
    pub spectrum: Vec<f32>,
}

impl Default for PlaybackSnapshot {
    fn default() -> Self {
        Self {
            revision: 0,
            status: PlaybackStatus::Stopped,
            current_item_id: None,
            position_ms: 0,
            duration_ms: None,
            bitrate_kbps: None,
            sample_rate_hz: None,
            channels: None,
            buffered_percent: None,
            now_playing_title: None,
            error: None,
            capabilities: SourceCapabilities::default(),
            spectrum: vec![0.0; 19],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Settings {
    pub volume: f32,
    pub balance: f32,
    pub shuffle: bool,
    pub repeat: bool,
    pub double_size: bool,
    pub always_on_top: bool,
    pub main_winshade: bool,
    pub language: String,
    pub selected_skin: Option<String>,
    pub eq: EqSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 0.8,
            balance: 0.0,
            shuffle: false,
            repeat: false,
            double_size: false,
            always_on_top: false,
            main_winshade: false,
            language: "en".into(),
            selected_skin: None,
            eq: EqSettings::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WindowPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WindowLayout {
    pub main: WindowPoint,
    pub equalizer: WindowPoint,
    pub playlist: WindowPoint,
    pub playlist_height: f64,
    pub equalizer_visible: bool,
    pub playlist_visible: bool,
    pub combined: bool,
}

impl Default for WindowLayout {
    fn default() -> Self {
        Self {
            main: WindowPoint { x: 100.0, y: 100.0 },
            equalizer: WindowPoint { x: 100.0, y: 216.0 },
            playlist: WindowPoint { x: 100.0, y: 332.0 },
            playlist_height: 232.0,
            equalizer_visible: true,
            playlist_visible: true,
            combined: false,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppSnapshot {
    #[ts(type = "number")]
    pub revision: u64,
    pub queue: Vec<QueueItem>,
    pub playback: PlaybackSnapshot,
    pub settings: Settings,
    pub layout: WindowLayout,
}

#[derive(Clone, Debug, Deserialize, TS)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "type"
)]
#[ts(export)]
pub enum PlayerCommand {
    Play,
    Pause,
    Stop,
    Next,
    Previous,
    Load {
        item_id: Uuid,
    },
    Seek {
        #[ts(type = "number")]
        position_ms: u64,
    },
    SetVolume {
        value: f32,
    },
    SetBalance {
        value: f32,
    },
    SetEqEnabled {
        enabled: bool,
    },
    SetPreamp {
        value_db: f32,
    },
    SetEqBand {
        index: usize,
        value_db: f32,
    },
    ToggleShuffle,
    ToggleRepeat,
    ToggleDoubleSize,
    ToggleAlwaysOnTop,
    ToggleWinshade,
    SetLanguage {
        language: String,
    },
}
