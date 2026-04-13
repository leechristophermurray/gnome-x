// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use gnomex_app::ports::ExtensionRepository;
use gnomex_app::AppError;
use gnomex_domain::{Extension, ExtensionState, ExtensionUuid, SearchResult, ShellVersion};
use reqwest::Client;
use serde::Deserialize;

const EGO_BASE: &str = "https://extensions.gnome.org";
/// Concrete adapter: extensions.gnome.org (EGO) HTTP client.
pub struct EgoClient {
    http: Client,
}

impl EgoClient {
    pub fn new() -> Self {
        Self {
            http: Client::builder()
                .user_agent("GNOME-X/0.1")
                .build()
                .expect("failed to build HTTP client"),
        }
    }
}

// --- Search endpoint DTOs ---

#[derive(Debug, Deserialize)]
struct EgoSearchResponse {
    total: u32,
    extensions: Vec<EgoSearchResult>,
    numpages: u32,
}

#[derive(Debug, Deserialize)]
struct EgoSearchResult {
    uuid: String,
    name: String,
    description: String,
    creator: EgoCreator,
    #[serde(alias = "pk")]
    id: u64,
    link: String,
    screenshot: Option<String>,
    #[serde(default)]
    shell_version_map: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum EgoCreator {
    Object { username: String },
    Plain(String),
}

impl EgoCreator {
    fn name(&self) -> &str {
        match self {
            Self::Object { username } => username,
            Self::Plain(s) => s,
        }
    }
}

// --- Extension-info endpoint DTO ---

#[derive(Debug, Deserialize)]
struct EgoInfoResponse {
    uuid: String,
    name: String,
    description: String,
    creator: EgoCreator,
    pk: u64,
    link: String,
    screenshot: Option<String>,
    #[serde(default)]
    shell_version_map: serde_json::Value,
    version: Option<u32>,
    download_url: Option<String>,
}

// --- Mapping ---

fn parse_shell_versions_from_map(map: &serde_json::Value) -> Vec<ShellVersion> {
    let Some(obj) = map.as_object() else {
        return vec![];
    };
    obj.keys()
        .filter_map(|k| ShellVersion::parse(k).ok())
        .collect()
}

fn search_result_to_domain(e: &EgoSearchResult) -> Extension {
    Extension {
        uuid: ExtensionUuid::new(&e.uuid).unwrap_or_else(|_| {
            ExtensionUuid::new(&format!("{}@unknown", e.id)).unwrap()
        }),
        name: e.name.clone(),
        description: e.description.clone(),
        creator: e.creator.name().to_owned(),
        shell_versions: parse_shell_versions_from_map(&e.shell_version_map),
        version: 0,
        download_url: None,
        screenshot_url: e.screenshot.as_ref().map(|s| {
            if s.starts_with("http") {
                s.clone()
            } else {
                format!("{EGO_BASE}{s}")
            }
        }),
        homepage_url: Some(format!("{EGO_BASE}{}", e.link)),
        pk: Some(e.id),
        state: ExtensionState::Available,
    }
}

#[async_trait::async_trait]
impl ExtensionRepository for EgoClient {
    async fn search(
        &self,
        query: &str,
        shell_version: &ShellVersion,
        page: u32,
    ) -> Result<SearchResult<Extension>, AppError> {
        let url = format!(
            "{EGO_BASE}/extension-query/?search={query}&shell_version={shell}&page={page}",
            query = urlencoding(query),
            shell = shell_version.major,
            page = page,
        );

        tracing::debug!("EGO search: {url}");

        let resp: EgoSearchResponse = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Repository(e.to_string()))?
            .json()
            .await
            .map_err(|e| AppError::Repository(e.to_string()))?;

        Ok(SearchResult {
            items: resp.extensions.iter().map(search_result_to_domain).collect(),
            total: resp.total,
            page,
            pages: resp.numpages,
        })
    }

    async fn get_info(
        &self,
        uuid: &ExtensionUuid,
        shell_version: &ShellVersion,
    ) -> Result<Extension, AppError> {
        let url = format!(
            "{EGO_BASE}/extension-info/?uuid={uuid}&shell_version={shell}",
            uuid = urlencoding(uuid.as_str()),
            shell = shell_version.major,
        );

        tracing::debug!("EGO info: {url}");

        let info: EgoInfoResponse = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Repository(e.to_string()))?
            .json()
            .await
            .map_err(|e| AppError::Repository(e.to_string()))?;

        Ok(Extension {
            uuid: ExtensionUuid::new(&info.uuid)
                .map_err(|e| AppError::Repository(e.to_string()))?,
            name: info.name,
            description: info.description,
            creator: info.creator.name().to_owned(),
            shell_versions: parse_shell_versions_from_map(&info.shell_version_map),
            version: info.version.unwrap_or(0),
            download_url: info.download_url.map(|p| {
                if p.starts_with("http") {
                    p
                } else {
                    format!("{EGO_BASE}{p}")
                }
            }),
            screenshot_url: info.screenshot.map(|s| {
                if s.starts_with("http") {
                    s
                } else {
                    format!("{EGO_BASE}{s}")
                }
            }),
            homepage_url: Some(format!("{EGO_BASE}{}", info.link)),
            pk: Some(info.pk),
            state: ExtensionState::Available,
        })
    }

    async fn download(
        &self,
        uuid: &ExtensionUuid,
        shell_version: &ShellVersion,
    ) -> Result<Vec<u8>, AppError> {
        let ext = self.get_info(uuid, shell_version).await?;

        let download_url = ext
            .download_url
            .ok_or_else(|| AppError::Repository(format!("no download URL for {uuid}")))?;

        tracing::debug!("EGO download: {download_url}");

        let bytes = self
            .http
            .get(&download_url)
            .send()
            .await
            .map_err(|e| AppError::Repository(e.to_string()))?
            .bytes()
            .await
            .map_err(|e| AppError::Repository(e.to_string()))?;

        Ok(bytes.to_vec())
    }

    async fn list_popular(
        &self,
        shell_version: &ShellVersion,
        page: u32,
    ) -> Result<SearchResult<Extension>, AppError> {
        self.query_sorted("popularity", shell_version, page).await
    }

    async fn list_recent(
        &self,
        shell_version: &ShellVersion,
        page: u32,
    ) -> Result<SearchResult<Extension>, AppError> {
        self.query_sorted("recent", shell_version, page).await
    }
}

impl EgoClient {
    async fn query_sorted(
        &self,
        sort: &str,
        shell_version: &ShellVersion,
        page: u32,
    ) -> Result<SearchResult<Extension>, AppError> {
        let url = format!(
            "{EGO_BASE}/extension-query/?sort={sort}&shell_version={shell}&page={page}",
            sort = sort,
            shell = shell_version.major,
            page = page,
        );

        tracing::debug!("EGO {sort}: {url}");

        let resp: EgoSearchResponse = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Repository(e.to_string()))?
            .json()
            .await
            .map_err(|e| AppError::Repository(e.to_string()))?;

        Ok(SearchResult {
            items: resp.extensions.iter().map(search_result_to_domain).collect(),
            total: resp.total,
            page,
            pages: resp.numpages,
        })
    }
}

/// Minimal percent-encoding for URL query parameters.
fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push(char::from(HEX[(b >> 4) as usize]));
                out.push(char::from(HEX[(b & 0x0F) as usize]));
            }
        }
    }
    out
}

const HEX: &[u8; 16] = b"0123456789ABCDEF";
