use std::{fs, path::Path};

use anyhow::{Context, Result, bail};

use crate::model::EqSettings;

const HEADER: &[u8; 31] = b"Winamp EQ library file v1.1\x1a!--";
const NAME_BYTES: usize = 257;
const FILE_BYTES: usize = HEADER.len() + NAME_BYTES + 11;

pub fn import(path: &Path) -> Result<EqSettings> {
    let bytes =
        fs::read(path).with_context(|| format!("failed reading EQ preset: {}", path.display()))?;
    if bytes.len() < FILE_BYTES || &bytes[..HEADER.len()] != HEADER {
        bail!("file is not a supported Winamp EQF preset");
    }
    let values = &bytes[HEADER.len() + NAME_BYTES..HEADER.len() + NAME_BYTES + 11];
    let mut bands_db = [0.0_f32; 10];
    for (target, source) in bands_db.iter_mut().zip(&values[..10]) {
        *target = decode_gain(*source);
    }
    Ok(EqSettings {
        enabled: true,
        preamp_db: decode_gain(values[10]),
        bands_db,
    })
}

pub fn export(path: &Path, name: &str, settings: &EqSettings) -> Result<()> {
    let mut bytes = Vec::with_capacity(FILE_BYTES);
    bytes.extend_from_slice(HEADER);
    let mut preset_name = [0_u8; NAME_BYTES];
    let clean_name = name.as_bytes();
    let count = clean_name.len().min(NAME_BYTES - 1);
    preset_name[..count].copy_from_slice(&clean_name[..count]);
    bytes.extend_from_slice(&preset_name);
    bytes.extend(settings.bands_db.iter().map(|gain| encode_gain(*gain)));
    bytes.push(encode_gain(settings.preamp_db));
    fs::write(path, bytes).with_context(|| format!("failed writing EQ preset: {}", path.display()))
}

fn decode_gain(value: u8) -> f32 {
    ((31.5 - f32::from(value.min(63))) * (12.0 / 31.5)).clamp(-12.0, 12.0)
}

fn encode_gain(gain: f32) -> u8 {
    (31.5 - gain.clamp(-12.0, 12.0) * (31.5 / 12.0)).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_supported_gain_range() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("test.eqf");
        let settings = EqSettings {
            enabled: true,
            preamp_db: -3.0,
            bands_db: [-12.0, -9.0, -6.0, -3.0, 0.0, 3.0, 6.0, 9.0, 12.0, 1.5],
        };
        export(&path, "Test", &settings).unwrap();
        let result = import(&path).unwrap();
        assert!((result.preamp_db - settings.preamp_db).abs() < 0.25);
        for (actual, expected) in result.bands_db.iter().zip(settings.bands_db) {
            assert!((actual - expected).abs() < 0.25);
        }
    }
}
