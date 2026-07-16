use std::{fs, io::Write, path::Path};

use anyhow::{Context, Result};

use crate::model::AppSnapshot;

const SESSION_FILE: &str = "session.json";

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
    let bytes = serde_json::to_vec_pretty(snapshot)?;

    let mut file =
        fs::File::create(&temp).with_context(|| format!("failed creating {}", temp.display()))?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    fs::rename(&temp, &target).with_context(|| format!("failed replacing {}", target.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
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
}
