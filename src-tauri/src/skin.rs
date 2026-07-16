use std::{fs, fs::File, io::Read, path::Path};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;
use zip::ZipArchive;

pub const MAX_COMPRESSED_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_EXPANDED_BYTES: u64 = 100 * 1024 * 1024;
pub const MAX_FILES: usize = 1_000;

#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SkinDescriptor {
    pub id: String,
    pub name: String,
    pub files: Vec<String>,
}

pub fn import(source: &Path, skins_dir: &Path) -> Result<SkinDescriptor> {
    validate_extension(source)?;
    let metadata = fs::metadata(source)
        .with_context(|| format!("failed reading skin metadata: {}", source.display()))?;
    if metadata.len() > MAX_COMPRESSED_BYTES {
        bail!("skin exceeds the 25 MiB compressed size limit");
    }

    let (id, files) = inspect(source)?;
    fs::create_dir_all(skins_dir)?;
    let target = skins_dir.join(format!("{id}.wsz"));
    if !target.exists() {
        fs::copy(source, &target)
            .with_context(|| format!("failed copying skin to {}", target.display()))?;
    }

    Ok(SkinDescriptor {
        id,
        name: source
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Imported skin")
            .to_owned(),
        files,
    })
}

pub fn read_bytes(skins_dir: &Path, id: &str) -> Result<Vec<u8>> {
    if id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("invalid skin id");
    }
    fs::read(skins_dir.join(format!("{id}.wsz"))).context("failed reading imported skin")
}

fn validate_extension(path: &Path) -> Result<()> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "wsz" | "zip") {
        bail!("only .wsz and .zip skin archives are supported");
    }
    Ok(())
}

fn inspect(path: &Path) -> Result<(String, Vec<String>)> {
    let mut hash_file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = hash_file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    let id = format!("{:x}", hasher.finalize());

    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file).context("skin is not a valid ZIP archive")?;
    if archive.len() > MAX_FILES {
        bail!("skin contains more than 1000 files");
    }

    let mut expanded = 0_u64;
    let mut files = Vec::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        if entry.enclosed_name().is_none() {
            bail!("skin contains an unsafe archive path");
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            bail!("skin contains a symbolic link");
        }
        expanded = expanded.saturating_add(entry.size());
        if expanded > MAX_EXPANDED_BYTES {
            bail!("skin exceeds the 100 MiB expanded size limit");
        }
        files.push(entry.name().replace('\\', "/").to_ascii_lowercase());
    }

    if !files.iter().any(|name| leaf(name) == "main.bmp") {
        bail!("skin does not contain MAIN.BMP");
    }
    files.sort();
    Ok((id, files))
}

fn leaf(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use zip::{ZipWriter, write::SimpleFileOptions};

    fn create_skin(path: &Path, names: &[&str]) {
        let file = File::create(path).unwrap();
        let mut writer = ZipWriter::new(file);
        for name in names {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"BMfixture").unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn accepts_nested_case_insensitive_skin() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("test.wsz");
        create_skin(&source, &["RET02/Main.BMP", "RET02/Pledit.txt"]);
        let descriptor = import(&source, &dir.path().join("installed")).unwrap();
        assert_eq!(descriptor.files[0], "ret02/main.bmp");
        assert_eq!(descriptor.id.len(), 64);
    }

    #[test]
    fn rejects_missing_main_bitmap() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("test.wsz");
        create_skin(&source, &["PLEDIT.BMP"]);
        assert!(import(&source, dir.path()).is_err());
    }

    #[test]
    #[ignore = "uses local, unlicensed compatibility samples"]
    fn validates_local_manual_samples() {
        let paths = std::env::var("TONELAG_LOCAL_SKINS")
            .expect("set TONELAG_LOCAL_SKINS to colon-separated .wsz paths");
        let directory = tempfile::tempdir().unwrap();
        for path in std::env::split_paths(&paths) {
            let descriptor = import(&path, directory.path()).unwrap();
            assert!(descriptor.files.iter().any(|name| leaf(name) == "main.bmp"));
        }
    }
}
