use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

use anyhow::{Context, Result};

use crate::model::AppSnapshot;

const SESSION_FILE: &str = "session.json";

pub struct DeferredSaver {
    sender: mpsc::Sender<()>,
}

impl DeferredSaver {
    pub fn new(data_dir: PathBuf, state: Arc<Mutex<AppSnapshot>>) -> Result<Self> {
        Self::spawn(Duration::from_millis(250), move || {
            let snapshot = state.lock().expect("state lock poisoned").clone();
            if let Err(error) = save(&data_dir, &snapshot) {
                log::warn!("Could not save deferred session state: {error:#}");
            }
        })
    }

    fn spawn(delay: Duration, callback: impl Fn() + Send + 'static) -> Result<Self> {
        let (sender, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("tonelag-session-saver".into())
            .spawn(move || {
                while receiver.recv().is_ok() {
                    while receiver.recv_timeout(delay).is_ok() {}
                    callback();
                }
            })?;
        Ok(Self { sender })
    }

    pub fn schedule(&self) -> Result<()> {
        self.sender
            .send(())
            .context("deferred session saver stopped")
    }

    #[cfg(test)]
    fn with_callback(delay: Duration, callback: impl Fn() + Send + 'static) -> Self {
        Self::spawn(delay, callback).unwrap()
    }
}

pub fn load(data_dir: &Path) -> Result<AppSnapshot> {
    let path = data_dir.join(SESSION_FILE);
    if !path.exists() {
        return Ok(AppSnapshot::default());
    }

    let contents =
        fs::read_to_string(&path).with_context(|| format!("failed reading {}", path.display()))?;
    let mut snapshot: AppSnapshot = serde_json::from_str(&contents)
        .with_context(|| format!("failed parsing {}", path.display()))?;

    snapshot.playback.status = Default::default();
    snapshot.playback.position_ms = 0;
    snapshot.playback.error = None;
    snapshot.playback.spectrum.fill(0.0);
    Ok(snapshot)
}

pub fn save(data_dir: &Path, snapshot: &AppSnapshot) -> Result<()> {
    fs::create_dir_all(data_dir)
        .with_context(|| format!("failed creating {}", data_dir.display()))?;
    let target = data_dir.join(SESSION_FILE);
    let temp = data_dir.join(format!(".{SESSION_FILE}.tmp"));
    let bytes = serde_json::to_vec(snapshot)?;

    let mut file =
        fs::File::create(&temp).with_context(|| format!("failed creating {}", temp.display()))?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    fs::rename(&temp, &target).with_context(|| format!("failed replacing {}", target.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{sync::mpsc, time::Duration};

    use super::*;

    #[test]
    fn round_trip_resets_playback() {
        let dir = tempfile::tempdir().unwrap();
        let mut original = AppSnapshot::default();
        original.playback.status = crate::model::PlaybackStatus::Playing;
        original.playback.position_ms = 42_000;
        save(dir.path(), &original).unwrap();

        let loaded = load(dir.path()).unwrap();
        assert_eq!(
            loaded.playback.status,
            crate::model::PlaybackStatus::Stopped
        );
        assert_eq!(loaded.playback.position_ms, 0);
    }

    #[test]
    fn deferred_saver_coalesces_a_burst_to_the_latest_snapshot() {
        let (saved_tx, saved_rx) = mpsc::channel();
        let revision = Arc::new(Mutex::new(0));
        let callback_revision = revision.clone();
        let saver = DeferredSaver::with_callback(Duration::from_millis(20), move || {
            saved_tx.send(*callback_revision.lock().unwrap()).unwrap()
        });
        for next_revision in 1..=20 {
            *revision.lock().unwrap() = next_revision;
            saver.schedule().unwrap();
        }

        assert_eq!(saved_rx.recv_timeout(Duration::from_secs(1)).unwrap(), 20);
        assert!(saved_rx.recv_timeout(Duration::from_millis(60)).is_err());
    }
}
