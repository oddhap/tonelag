use std::time::Duration;

use anyhow::{Context, Result, bail};
use futures_util::StreamExt;
use md5::{Digest, Md5};
use reqwest::{Client, Response, redirect::Policy};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::json;
use ts_rs::TS;
use url::Url;

use crate::skin::MAX_COMPRESSED_BYTES;

const GRAPHQL_ENDPOINT: &str = "https://skins.webamp.org/graphql";
const MAX_CATALOG_RESPONSE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_QUERY_CHARS: usize = 100;
const MAX_PAGE_SIZE: u32 = 48;

const BROWSE_QUERY: &str = r#"
query TonelagBrowseSkins($first: Int!, $offset: Int!) {
  skins(filter: APPROVED, first: $first, offset: $offset) {
    count
    nodes {
      __typename
      ... on ClassicSkin {
        md5
        filename(normalize_extension: true)
        screenshot_url
        museum_url
        nsfw
      }
    }
  }
}
"#;

const SEARCH_QUERY: &str = r#"
query TonelagSearchSkins($query: String!, $first: Int!, $offset: Int!) {
  search_classic_skins(query: $query, first: $first, offset: $offset) {
    md5
    filename(normalize_extension: true)
    screenshot_url
    museum_url
    nsfw
  }
}
"#;

#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SkinCatalogEntry {
    pub md5: String,
    pub name: String,
    pub screenshot_url: String,
    pub museum_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SkinCatalogPage {
    pub items: Vec<SkinCatalogEntry>,
    pub offset: u32,
    pub limit: u32,
    pub total_count: Option<u32>,
    pub has_more: bool,
}

#[derive(Debug, Deserialize)]
struct GraphQlResponse<T> {
    data: Option<T>,
    #[serde(default)]
    errors: Vec<GraphQlError>,
}

#[derive(Debug, Deserialize)]
struct GraphQlError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct BrowseData {
    skins: SkinConnection,
}

#[derive(Debug, Deserialize)]
struct SkinConnection {
    count: u32,
    nodes: Vec<SkinNode>,
}

#[derive(Debug, Deserialize)]
struct SearchData {
    search_classic_skins: Vec<SkinNode>,
}

#[derive(Debug, Deserialize)]
struct SkinNode {
    #[serde(rename = "__typename")]
    kind: Option<String>,
    md5: Option<String>,
    filename: Option<String>,
    screenshot_url: Option<String>,
    museum_url: Option<String>,
    nsfw: Option<bool>,
}

pub async fn browse(query: Option<String>, offset: u32, limit: u32) -> Result<SkinCatalogPage> {
    let limit = limit.clamp(1, MAX_PAGE_SIZE);
    let query = query.unwrap_or_default().trim().to_owned();
    if query.chars().count() > MAX_QUERY_CHARS {
        bail!("skin search is limited to {MAX_QUERY_CHARS} characters");
    }

    let client = client()?;
    if query.is_empty() {
        let response = client
            .post(GRAPHQL_ENDPOINT)
            .json(&json!({
                "query": BROWSE_QUERY,
                "variables": { "first": limit, "offset": offset },
            }))
            .send()
            .await
            .context("could not contact the Webamp Skin Museum")?
            .error_for_status()
            .context("the Webamp Skin Museum returned an error")?;
        let data: BrowseData = decode_graphql(response).await?;
        let raw_count = data.skins.nodes.len();
        Ok(SkinCatalogPage {
            items: catalog_entries(data.skins.nodes),
            offset,
            limit,
            total_count: Some(data.skins.count),
            has_more: offset.saturating_add(limit) < data.skins.count
                && raw_count == limit as usize,
        })
    } else {
        let response = client
            .post(GRAPHQL_ENDPOINT)
            .json(&json!({
                "query": SEARCH_QUERY,
                "variables": { "query": query, "first": limit, "offset": offset },
            }))
            .send()
            .await
            .context("could not contact the Webamp Skin Museum")?
            .error_for_status()
            .context("the Webamp Skin Museum returned an error")?;
        let data: SearchData = decode_graphql(response).await?;
        let raw_count = data.search_classic_skins.len();
        Ok(SkinCatalogPage {
            items: catalog_entries(data.search_classic_skins),
            offset,
            limit,
            total_count: None,
            has_more: raw_count == limit as usize,
        })
    }
}

pub async fn download(md5: &str) -> Result<Vec<u8>> {
    validate_md5(md5)?;
    let url = format!("https://r2.webampskins.org/skins/{md5}.wsz");
    let response = client()?
        .get(url)
        .send()
        .await
        .context("could not download the selected skin")?
        .error_for_status()
        .context("the selected skin could not be downloaded")?;
    let bytes = read_limited(response, MAX_COMPRESSED_BYTES).await?;
    let actual_md5 = Md5::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if actual_md5 != md5 {
        bail!("downloaded skin did not match its catalog checksum");
    }
    Ok(bytes)
}

fn client() -> Result<Client> {
    Client::builder()
        .user_agent("Tonelag/0.1 (+https://github.com/oddhap/tonelag)")
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(30))
        .redirect(Policy::none())
        .build()
        .context("could not create the skin catalog client")
}

async fn decode_graphql<T: DeserializeOwned>(response: Response) -> Result<T> {
    let bytes = read_limited(response, MAX_CATALOG_RESPONSE_BYTES).await?;
    let response: GraphQlResponse<T> =
        serde_json::from_slice(&bytes).context("the skin catalog returned invalid data")?;
    if let Some(error) = response.errors.first() {
        bail!("skin catalog request failed: {}", error.message);
    }
    response
        .data
        .context("the skin catalog returned no response data")
}

async fn read_limited(response: Response, limit: u64) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > limit)
    {
        bail!("download exceeds the allowed size limit");
    }
    let mut output =
        Vec::with_capacity(response.content_length().unwrap_or_default().min(limit) as usize);
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("download was interrupted")?;
        if output.len().saturating_add(chunk.len()) > limit as usize {
            bail!("download exceeds the allowed size limit");
        }
        output.extend_from_slice(&chunk);
    }
    Ok(output)
}

fn catalog_entries(nodes: Vec<SkinNode>) -> Vec<SkinCatalogEntry> {
    nodes.into_iter().filter_map(catalog_entry).collect()
}

fn catalog_entry(node: SkinNode) -> Option<SkinCatalogEntry> {
    if node.nsfw != Some(false)
        || node
            .kind
            .as_deref()
            .is_some_and(|kind| kind != "ClassicSkin")
    {
        return None;
    }
    let md5 = node.md5?;
    if validate_md5(&md5).is_err() {
        return None;
    }
    let screenshot_url = validated_catalog_url(
        node.screenshot_url?,
        "r2.webampskins.org",
        &format!("/screenshots/{md5}.png"),
    )?;
    let museum_url = validated_catalog_url(
        node.museum_url?,
        "skins.webamp.org",
        &format!("/skin/{md5}"),
    )?;
    Some(SkinCatalogEntry {
        md5,
        name: display_name(node.filename.as_deref().unwrap_or("Webamp skin")),
        screenshot_url,
        museum_url,
    })
}

fn validated_catalog_url(value: String, host: &str, path: &str) -> Option<String> {
    let url = Url::parse(&value).ok()?;
    if url.scheme() != "https"
        || url.host_str() != Some(host)
        || url.path() != path
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    Some(value)
}

fn validate_md5(value: &str) -> Result<()> {
    if value.len() != 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        bail!("invalid skin catalog identifier");
    }
    Ok(())
}

fn display_name(filename: &str) -> String {
    let leaf = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    let without_extension = leaf
        .strip_suffix(".wsz")
        .or_else(|| leaf.strip_suffix(".WSZ"))
        .unwrap_or(leaf);
    let cleaned = without_extension
        .chars()
        .filter(|character| !character.is_control())
        .take(120)
        .collect::<String>();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        "Webamp skin".to_owned()
    } else {
        cleaned.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_nsfw_and_untrusted_catalog_urls() {
        let safe = SkinNode {
            kind: Some("ClassicSkin".into()),
            md5: Some("5eddd4551ab639a85951323a6df7463e".into()),
            filename: Some("224_Receiver.wsz".into()),
            screenshot_url: Some(
                "https://r2.webampskins.org/screenshots/5eddd4551ab639a85951323a6df7463e.png"
                    .into(),
            ),
            museum_url: Some(
                "https://skins.webamp.org/skin/5eddd4551ab639a85951323a6df7463e".into(),
            ),
            nsfw: Some(false),
        };
        assert_eq!(catalog_entry(safe).unwrap().name, "224_Receiver");

        let unsafe_host = SkinNode {
            kind: Some("ClassicSkin".into()),
            md5: Some("5eddd4551ab639a85951323a6df7463e".into()),
            filename: Some("unsafe.wsz".into()),
            screenshot_url: Some(
                "https://example.com/screenshots/5eddd4551ab639a85951323a6df7463e.png".into(),
            ),
            museum_url: Some(
                "https://skins.webamp.org/skin/5eddd4551ab639a85951323a6df7463e".into(),
            ),
            nsfw: Some(false),
        };
        assert!(catalog_entry(unsafe_host).is_none());

        let nsfw = SkinNode {
            kind: None,
            md5: Some("5eddd4551ab639a85951323a6df7463e".into()),
            filename: Some("hidden.wsz".into()),
            screenshot_url: Some(
                "https://r2.webampskins.org/screenshots/5eddd4551ab639a85951323a6df7463e.png"
                    .into(),
            ),
            museum_url: Some(
                "https://skins.webamp.org/skin/5eddd4551ab639a85951323a6df7463e".into(),
            ),
            nsfw: Some(true),
        };
        assert!(catalog_entry(nsfw).is_none());
    }

    #[test]
    fn validates_catalog_identifiers_and_cleans_names() {
        assert!(validate_md5("5eddd4551ab639a85951323a6df7463e").is_ok());
        assert!(validate_md5("../not-a-hash").is_err());
        assert_eq!(display_name("folder\\Receiver.WSZ"), "Receiver");
        assert_eq!(display_name("\0.wsz"), "Webamp skin");
    }

    #[tokio::test]
    #[ignore = "requires the public Webamp Skin Museum"]
    async fn public_catalog_smoke_test() {
        let page = browse(Some("zelda".into()), 0, 3).await.unwrap();
        assert!(!page.items.is_empty());
        assert!(page.items.iter().all(|item| item.md5.len() == 32));
        let first = &page.items[0];
        let bytes = download(&first.md5).await.unwrap();
        let directory = tempfile::tempdir().unwrap();
        let installed = crate::skin::install_bytes(&first.name, &bytes, directory.path()).unwrap();
        assert!(
            installed
                .files
                .iter()
                .any(|name| name.ends_with("main.bmp"))
        );
    }
}
