use std::{
    fs,
    fs::OpenOptions,
    io::{Cursor, Write},
    path::Path,
};

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
    #[serde(default)]
    pub bundled: bool,
}

pub fn import(source: &Path, skins_dir: &Path) -> Result<SkinDescriptor> {
    validate_extension(source)?;
    let metadata = fs::metadata(source)
        .with_context(|| format!("failed reading skin metadata: {}", source.display()))?;
    if metadata.len() > MAX_COMPRESSED_BYTES {
        bail!("skin exceeds the 25 MiB compressed size limit");
    }

    let bytes = fs::read(source)
        .with_context(|| format!("failed reading skin archive: {}", source.display()))?;
    install_bytes(
        source
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Imported skin"),
        &bytes,
        skins_dir,
    )
}

pub fn install_bytes(name: &str, bytes: &[u8], skins_dir: &Path) -> Result<SkinDescriptor> {
    if bytes.len() as u64 > MAX_COMPRESSED_BYTES {
        bail!("skin exceeds the 25 MiB compressed size limit");
    }

    let (id, files) = inspect(bytes)?;
    fs::create_dir_all(skins_dir)?;
    let target = skins_dir.join(format!("{id}.wsz"));
    if !target.exists() {
        let temporary = skins_dir.join(format!(".skin-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| -> Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .context("failed creating temporary skin archive")?;
            file.write_all(bytes)
                .context("failed writing temporary skin archive")?;
            file.sync_all()
                .context("failed syncing temporary skin archive")?;
            fs::rename(&temporary, &target)
                .with_context(|| format!("failed installing skin to {}", target.display()))?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result?;
    }

    let name = name
        .chars()
        .filter(|character| !character.is_control())
        .take(120)
        .collect::<String>();
    let name = name.trim();
    let descriptor = SkinDescriptor {
        id,
        name: if name.is_empty() {
            "Imported skin".to_owned()
        } else {
            name.to_owned()
        },
        files,
        bundled: false,
    };
    write_descriptor(skins_dir, &descriptor)?;
    Ok(descriptor)
}

pub fn list(skins_dir: &Path) -> Result<Vec<SkinDescriptor>> {
    if !skins_dir.exists() {
        return Ok(Vec::new());
    }
    let mut descriptors = Vec::new();
    for entry in fs::read_dir(skins_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) != Some("wsz") {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        if id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            continue;
        }
        let descriptor_path = skins_dir.join(format!("{id}.json"));
        let descriptor = fs::read(&descriptor_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<SkinDescriptor>(&bytes).ok())
            .filter(|descriptor| descriptor.id == id)
            .or_else(|| {
                let bytes = fs::read(&path).ok()?;
                let (_, files) = inspect(&bytes).ok()?;
                Some(SkinDescriptor {
                    id: id.to_owned(),
                    name: format!("Legacy skin {}", &id[..8]),
                    files,
                    bundled: false,
                })
            });
        if let Some(descriptor) = descriptor {
            descriptors.push(descriptor);
        }
    }
    descriptors.sort_by_cached_key(|descriptor| descriptor.name.to_lowercase());
    Ok(descriptors)
}

pub fn read_bytes(skins_dir: &Path, id: &str) -> Result<Vec<u8>> {
    if id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("invalid skin id");
    }
    fs::read(skins_dir.join(format!("{id}.wsz"))).context("failed reading imported skin")
}

pub fn mark_bundled(skins_dir: &Path, descriptor: &mut SkinDescriptor) -> Result<()> {
    descriptor.bundled = true;
    write_descriptor(skins_dir, descriptor)
}

pub fn remove(skins_dir: &Path, id: &str) -> Result<()> {
    if id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("invalid skin id");
    }
    let archive = skins_dir.join(format!("{id}.wsz"));
    let metadata = skins_dir.join(format!("{id}.json"));
    let descriptor = fs::read(&metadata)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<SkinDescriptor>(&bytes).ok());
    if descriptor.is_some_and(|descriptor| descriptor.bundled) {
        bail!("bundled skins cannot be deleted");
    }
    if !archive.exists() {
        bail!("skin is not installed");
    }

    let nonce = uuid::Uuid::new_v4();
    let archived = skins_dir.join(format!(".deleted-{nonce}.wsz"));
    let metadata_archived = skins_dir.join(format!(".deleted-{nonce}.json"));
    fs::rename(&archive, &archived).context("failed staging skin archive for deletion")?;
    if metadata.exists()
        && let Err(error) = fs::rename(&metadata, &metadata_archived)
    {
        let _ = fs::rename(&archived, &archive);
        return Err(error).context("failed staging skin metadata for deletion");
    }
    fs::remove_file(&archived).context("failed deleting skin archive")?;
    if metadata_archived.exists() {
        fs::remove_file(metadata_archived).context("failed deleting skin metadata")?;
    }
    Ok(())
}

fn write_descriptor(skins_dir: &Path, descriptor: &SkinDescriptor) -> Result<()> {
    let target = skins_dir.join(format!("{}.json", descriptor.id));
    if fs::read(&target)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<SkinDescriptor>(&bytes).ok())
        .as_ref()
        == Some(descriptor)
    {
        return Ok(());
    }
    let temporary = skins_dir.join(format!(".skin-meta-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&serde_json::to_vec_pretty(descriptor)?)?;
        file.sync_all()?;
        fs::rename(&temporary, &target)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
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

fn inspect(bytes: &[u8]) -> Result<(String, Vec<String>)> {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let id = format!("{:x}", hasher.finalize());

    let mut archive =
        ZipArchive::new(Cursor::new(bytes)).context("skin is not a valid ZIP archive")?;
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
    use std::{fs::File, io::Write};

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
    fn installs_downloaded_bytes_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("download.wsz");
        create_skin(&source, &["MAIN.BMP", "EQMAIN.BMP"]);
        let bytes = fs::read(source).unwrap();

        let descriptor = install_bytes("Downloaded skin", &bytes, dir.path()).unwrap();

        assert_eq!(descriptor.name, "Downloaded skin");
        assert_eq!(read_bytes(dir.path(), &descriptor.id).unwrap(), bytes);
        assert_eq!(
            fs::read_dir(dir.path())
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().is_some_and(|value| value == "tmp"))
                .count(),
            0
        );
        assert_eq!(list(dir.path()).unwrap(), vec![descriptor]);
    }

    #[test]
    fn lists_existing_archives_without_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("legacy.wsz");
        create_skin(&source, &["MAIN.BMP"]);
        let bytes = fs::read(&source).unwrap();
        let (id, _) = inspect(&bytes).unwrap();
        fs::rename(source, dir.path().join(format!("{id}.wsz"))).unwrap();

        let skins = list(dir.path()).unwrap();

        assert_eq!(skins.len(), 1);
        assert_eq!(skins[0].id, id);
        assert_eq!(skins[0].name, format!("Legacy skin {}", &id[..8]));
    }

    #[test]
    fn removes_an_installed_skin_and_its_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("remove.wsz");
        create_skin(&source, &["MAIN.BMP"]);
        let bytes = fs::read(source).unwrap();
        let descriptor = install_bytes("Remove me", &bytes, dir.path()).unwrap();

        remove(dir.path(), &descriptor.id).unwrap();

        assert!(!dir.path().join(format!("{}.wsz", descriptor.id)).exists());
        assert!(!dir.path().join(format!("{}.json", descriptor.id)).exists());
        assert!(list(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn refuses_to_remove_a_bundled_skin() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("bundled.wsz");
        create_skin(&source, &["MAIN.BMP"]);
        let bytes = fs::read(source).unwrap();
        let mut descriptor = install_bytes("Bundled", &bytes, dir.path()).unwrap();
        mark_bundled(dir.path(), &mut descriptor).unwrap();

        assert!(remove(dir.path(), &descriptor.id).is_err());
        assert!(dir.path().join(format!("{}.wsz", descriptor.id)).exists());
    }

    #[test]
    fn bundled_pastellplate_is_a_complete_classic_skin() {
        let bytes = include_bytes!("../../assets/default-skin/pastellplate.wsz");
        let (_, files) = inspect(bytes).unwrap();

        for required in [
            "main.bmp",
            "eqmain.bmp",
            "pledit.bmp",
            "cbuttons.bmp",
            "titlebar.bmp",
            "shufrep.bmp",
            "posbar.bmp",
            "text.bmp",
            "numbers.bmp",
            "pledit.txt",
            "viscolor.txt",
            "region.txt",
        ] {
            assert!(
                files.iter().any(|name| leaf(name) == required),
                "missing {required}"
            );
        }
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
