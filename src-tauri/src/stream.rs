use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use url::Url;

use crate::playlist::{PLAYLIST_LIMIT_BYTES, parse_m3u, parse_pls};

#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ResolvedStream {
    pub url: String,
    pub station_name: Option<String>,
    pub content_type: Option<String>,
}

pub async fn resolve(input: &str) -> Result<ResolvedStream> {
    let initial = Url::parse(input).context("invalid stream URL")?;
    if !matches!(initial.scheme(), "http" | "https") {
        bail!("only HTTP and HTTPS streams are supported");
    }

    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .user_agent("Tonelag/0.1")
        .build()?;
    resolve_inner(&client, initial, 0).await
}

async fn resolve_inner(client: &Client, url: Url, depth: u8) -> Result<ResolvedStream> {
    if depth > 2 {
        bail!("playlist nesting exceeds the safety limit");
    }
    let safe_url = crate::privacy::redact_url(url.as_str());
    let response = client
        .get(url.clone())
        .header("Icy-MetaData", "1")
        .header(header::RANGE, format!("bytes=0-{}", PLAYLIST_LIMIT_BYTES))
        .send()
        .await
        .map_err(|error| {
            anyhow::anyhow!("request to {safe_url} failed: {}", error.without_url())
        })?;
    let response = response.error_for_status().map_err(|error| {
        anyhow::anyhow!(
            "request to {safe_url} returned {}",
            error.status().map(|status| status.as_u16()).unwrap_or(0)
        )
    })?;
    let final_url = response.url().clone();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(';')
                .next()
                .unwrap_or(value)
                .trim()
                .to_ascii_lowercase()
        });
    let station_name = response
        .headers()
        .get("icy-name")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);

    let looks_like_playlist = content_type.as_deref().is_some_and(|value| {
        matches!(
            value,
            "audio/x-mpegurl"
                | "audio/mpegurl"
                | "application/x-mpegurl"
                | "application/vnd.apple.mpegurl"
                | "audio/x-scpls"
        )
    }) || matches!(
        final_url
            .path()
            .rsplit('.')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "m3u" | "m3u8" | "pls"
    );

    if !looks_like_playlist {
        if content_type.as_deref().is_some_and(|value| {
            !value.starts_with("audio/")
                && !matches!(value, "application/ogg" | "application/octet-stream")
        }) {
            bail!("server returned an unsupported audio MIME type");
        }
        return Ok(ResolvedStream {
            url: final_url.to_string(),
            station_name,
            content_type,
        });
    }

    let bytes = response.bytes().await?;
    if bytes.len() as u64 > PLAYLIST_LIMIT_BYTES {
        bail!("remote playlist exceeds the 2 MiB safety limit");
    }
    let body = String::from_utf8_lossy(&bytes);
    if body
        .lines()
        .any(|line| line.trim_start().starts_with("#EXT-X-"))
    {
        bail!("HLS playlists are not supported in v1");
    }
    let entries = if final_url.path().to_ascii_lowercase().ends_with(".pls")
        || content_type.as_deref() == Some("audio/x-scpls")
    {
        parse_pls(&body)
    } else {
        parse_m3u(&body)
    };
    let first = entries.first().context("remote playlist is empty")?;
    let nested = final_url
        .join(first)
        .context("invalid entry in remote playlist")?;
    Box::pin(resolve_inner(client, nested, depth + 1)).await
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    use super::*;

    fn fixture(
        requests: usize,
        responder: impl Fn(&str) -> (&'static str, &'static str, &'static str) + Send + 'static,
    ) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let task = thread::spawn(move || {
            for _ in 0..requests {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 4_096];
                let count = stream.read(&mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..count]);
                let path = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap_or("/");
                let (status, headers, body) = responder(path);
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        (format!("http://{address}"), task)
    }

    #[tokio::test]
    async fn follows_redirects_and_reads_icy_station_name() {
        let (base, task) = fixture(2, |path| match path {
            "/start" => ("302 Found", "Location: /live\r\n", ""),
            _ => (
                "200 OK",
                "Content-Type: audio/mpeg\r\nicy-name: Fixture Radio\r\n",
                "audio",
            ),
        });
        let resolved = resolve(&format!("{base}/start")).await.unwrap();
        assert_eq!(resolved.station_name.as_deref(), Some("Fixture Radio"));
        assert!(resolved.url.ends_with("/live"));
        task.join().unwrap();
    }

    #[tokio::test]
    async fn resolves_m3u_and_pls_entries() {
        for (extension, body) in [
            ("m3u", "#EXTM3U\n/live\n"),
            ("pls", "[playlist]\nFile1=/live\n"),
        ] {
            let (base, task) = fixture(2, move |path| {
                if path.ends_with(extension) {
                    (
                        "200 OK",
                        if extension == "pls" {
                            "Content-Type: audio/x-scpls\r\n"
                        } else {
                            "Content-Type: audio/x-mpegurl\r\n"
                        },
                        body,
                    )
                } else {
                    ("200 OK", "Content-Type: audio/aac\r\n", "audio")
                }
            });
            let resolved = resolve(&format!("{base}/list.{extension}")).await.unwrap();
            assert!(resolved.url.ends_with("/live"));
            assert_eq!(resolved.content_type.as_deref(), Some("audio/aac"));
            task.join().unwrap();
        }
    }

    #[tokio::test]
    async fn rejects_hls_manifests() {
        let (base, task) = fixture(1, |_| {
            (
                "200 OK",
                "Content-Type: application/vnd.apple.mpegurl\r\n",
                "#EXTM3U\n#EXT-X-VERSION:3\n",
            )
        });
        let error = resolve(&format!("{base}/live.m3u8")).await.unwrap_err();
        assert!(error.to_string().contains("HLS"));
        task.join().unwrap();
    }

    #[tokio::test]
    async fn rejects_non_audio_mime_types() {
        let (base, task) = fixture(1, |_| {
            (
                "200 OK",
                "Content-Type: text/html\r\n",
                "<html>not audio</html>",
            )
        });
        let error = resolve(&format!("{base}/live")).await.unwrap_err();
        assert!(error.to_string().contains("MIME"));
        task.join().unwrap();
    }
}
