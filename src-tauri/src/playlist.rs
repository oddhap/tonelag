use std::{fs, path::Path};

use anyhow::{Context, Result, bail};
use url::Url;
use uuid::Uuid;

use crate::model::{QueueItem, QueueOrigin};

pub const PLAYLIST_LIMIT_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaylistFormat {
    M3u,
    Pls,
}

pub fn infer_format(path: &Path) -> Result<PlaylistFormat> {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "m3u" | "m3u8" => Ok(PlaylistFormat::M3u),
        "pls" => Ok(PlaylistFormat::Pls),
        extension => bail!("unsupported playlist extension: {extension}"),
    }
}

pub fn import(path: &Path) -> Result<Vec<QueueItem>> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("failed reading playlist metadata: {}", path.display()))?;
    if metadata.len() > PLAYLIST_LIMIT_BYTES {
        bail!("playlist exceeds the 2 MiB safety limit");
    }

    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed reading playlist: {}", path.display()))?;
    if contents
        .lines()
        .any(|line| line.trim_start().starts_with("#EXT-X-"))
    {
        bail!("HLS playlists are not supported in v1");
    }

    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let entries = match infer_format(path)? {
        PlaylistFormat::M3u => parse_m3u(&contents),
        PlaylistFormat::Pls => parse_pls(&contents),
    };

    Ok(entries
        .into_iter()
        .map(|entry| entry_to_queue_item(&entry, Some(base)))
        .collect())
}

pub fn export(path: &Path, items: &[QueueItem]) -> Result<()> {
    let format = infer_format(path)?;
    let body = match format {
        PlaylistFormat::M3u => render_m3u(items),
        PlaylistFormat::Pls => render_pls(items),
    };
    fs::write(path, body).with_context(|| format!("failed writing playlist: {}", path.display()))
}

pub fn parse_m3u(contents: &str) -> Vec<String> {
    contents
        .trim_start_matches('\u{feff}')
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

pub fn parse_pls(contents: &str) -> Vec<String> {
    let mut entries = contents
        .lines()
        .filter_map(|line| line.split_once('='))
        .filter_map(|(key, value)| {
            let key = key.trim().to_ascii_lowercase();
            key.strip_prefix("file")
                .and_then(|index| index.parse::<usize>().ok())
                .map(|index| (index, value.trim().to_owned()))
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|(index, _)| *index);
    entries.into_iter().map(|(_, value)| value).collect()
}

pub fn entry_to_queue_item(entry: &str, base: Option<&Path>) -> QueueItem {
    if let Ok(url) = Url::parse(entry)
        && matches!(url.scheme(), "http" | "https")
    {
        let title = url
            .path_segments()
            .and_then(|mut segments| segments.next_back())
            .filter(|value| !value.is_empty())
            .unwrap_or(url.host_str().unwrap_or("Internet stream"))
            .to_owned();
        return QueueItem {
            id: Uuid::new_v4(),
            origin: QueueOrigin::HttpStream {
                url: url.to_string(),
            },
            title,
            artist: None,
            duration_ms: None,
            available: true,
        };
    }

    let path = Path::new(entry);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.unwrap_or_else(|| Path::new(".")).join(path)
    };
    QueueItem {
        id: Uuid::new_v4(),
        origin: QueueOrigin::LocalFile {
            path: absolute.to_string_lossy().into_owned(),
        },
        title: absolute
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Unknown track")
            .to_owned(),
        artist: None,
        duration_ms: None,
        available: absolute.exists(),
    }
}

fn render_m3u(items: &[QueueItem]) -> String {
    let mut output = String::from("#EXTM3U\n");
    for item in items {
        let duration = item.duration_ms.map(|value| value / 1_000).unwrap_or(0);
        output.push_str(&format!("#EXTINF:{duration},{}\n", item.title));
        output.push_str(origin_value(&item.origin));
        output.push('\n');
    }
    output
}

fn render_pls(items: &[QueueItem]) -> String {
    let mut output = format!("[playlist]\nNumberOfEntries={}\n", items.len());
    for (index, item) in items.iter().enumerate() {
        let position = index + 1;
        let duration = item.duration_ms.map(|value| value / 1_000).unwrap_or(0);
        output.push_str(&format!(
            "File{position}={}\nTitle{position}={}\nLength{position}={duration}\n",
            origin_value(&item.origin),
            item.title
        ));
    }
    output.push_str("Version=2\n");
    output
}

fn origin_value(origin: &QueueOrigin) -> &str {
    match origin {
        QueueOrigin::LocalFile { path } => path,
        QueueOrigin::HttpStream { url } => url,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_extended_m3u_without_metadata_lines() {
        let result =
            parse_m3u("#EXTM3U\n#EXTINF:123,Example\ntrack.mp3\nhttps://radio.test/live\n");
        assert_eq!(result, ["track.mp3", "https://radio.test/live"]);
    }

    #[test]
    fn parses_pls_in_numeric_order() {
        let result = parse_pls("[playlist]\nFile2=b.mp3\nFile1=a.mp3\nTitle1=A\n");
        assert_eq!(result, ["a.mp3", "b.mp3"]);
    }

    #[test]
    fn rejects_hls_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stream.m3u8");
        fs::write(&path, "#EXTM3U\n#EXT-X-VERSION:3\n").unwrap();
        assert!(import(&path).unwrap_err().to_string().contains("HLS"));
    }
}
