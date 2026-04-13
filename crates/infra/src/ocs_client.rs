// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use gnomex_app::ports::ContentRepository;
use gnomex_app::AppError;
use gnomex_domain::{ContentCategory, ContentId, ContentItem, ContentState, SearchResult};
use reqwest::Client;
use serde::Deserialize;

const OCS_BASE: &str = "https://api.gnome-look.org/ocs/v1";
const PAGE_SIZE: u32 = 25;

/// Concrete adapter: gnome-look.org OCS (Open Collaboration Services) API client.
pub struct OcsClient {
    http: Client,
}

impl OcsClient {
    pub fn new() -> Self {
        Self {
            http: Client::builder()
                .user_agent("GNOME-X/0.1")
                .build()
                .expect("failed to build HTTP client"),
        }
    }
}

// --- OCS response DTOs (flat format, no envelope) ---

#[derive(Debug, Deserialize)]
struct OcsResponse {
    totalitems: u32,
    itemsperpage: u32,
    data: Vec<OcsContentEntry>,
}

#[derive(Debug, Deserialize)]
struct OcsContentEntry {
    id: u64,
    name: String,
    description: Option<String>,
    personid: Option<String>,
    #[serde(rename = "previewpic1")]
    preview_pic: Option<String>,
    #[serde(rename = "downloadlink1")]
    download_link: Option<String>,
    #[serde(default)]
    score: u32,
}

// --- Mapping ---

fn ocs_entry_to_domain(entry: &OcsContentEntry, category: ContentCategory) -> ContentItem {
    ContentItem {
        id: ContentId(entry.id),
        name: entry.name.clone(),
        description: entry.description.clone().unwrap_or_default(),
        creator: entry.personid.clone().unwrap_or_default(),
        category,
        download_url: entry.download_link.clone().filter(|s| !s.is_empty()),
        preview_url: entry.preview_pic.clone().filter(|s| !s.is_empty()),
        rating: if entry.score > 0 {
            Some(entry.score as f32 / 10.0) // OCS score is 0-100, normalise to 0-10
        } else {
            None
        },
        state: ContentState::Available,
    }
}

#[async_trait::async_trait]
impl ContentRepository for OcsClient {
    async fn search(
        &self,
        query: &str,
        category: ContentCategory,
        page: u32,
    ) -> Result<SearchResult<ContentItem>, AppError> {
        let url = format!(
            "{OCS_BASE}/content/data?search={query}&categories={cat}&page={page}&pagesize={PAGE_SIZE}&format=json",
            query = urlencoding(query),
            cat = category.ocs_id(),
            page = page.max(1) - 1, // OCS uses 0-based pages
        );

        tracing::debug!("OCS search: {url}");

        let resp: OcsResponse = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Repository(e.to_string()))?
            .json()
            .await
            .map_err(|e| AppError::Repository(e.to_string()))?;

        let total = resp.totalitems;
        let per_page = resp.itemsperpage.max(1);
        let pages = (total + per_page - 1) / per_page;

        Ok(SearchResult {
            items: resp
                .data
                .iter()
                .map(|e| ocs_entry_to_domain(e, category))
                .collect(),
            total,
            page,
            pages,
        })
    }

    async fn get_info(&self, id: ContentId) -> Result<ContentItem, AppError> {
        let url = format!(
            "{OCS_BASE}/content/data/{id}?format=json",
            id = id.0,
        );

        tracing::debug!("OCS info: {url}");

        let resp: OcsResponse = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Repository(e.to_string()))?
            .json()
            .await
            .map_err(|e| AppError::Repository(e.to_string()))?;

        let entry = resp
            .data
            .first()
            .ok_or_else(|| AppError::Repository(format!("no content found for id {}", id.0)))?;

        Ok(ocs_entry_to_domain(entry, ContentCategory::GtkTheme))
    }

    async fn download(&self, id: ContentId, file_id: u64) -> Result<Vec<u8>, AppError> {
        let url = format!(
            "{OCS_BASE}/content/download/{id}/{file_id}",
            id = id.0,
            file_id = file_id,
        );

        tracing::debug!("OCS download: {url}");

        let bytes = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Repository(e.to_string()))?
            .bytes()
            .await
            .map_err(|e| AppError::Repository(e.to_string()))?;

        Ok(bytes.to_vec())
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
