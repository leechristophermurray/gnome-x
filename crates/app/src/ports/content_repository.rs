// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::AppError;
use gnomex_domain::{ContentCategory, ContentId, ContentItem, SearchResult};

/// Port: search and download themes/icons/cursors from gnome-look.org (OCS API).
#[async_trait::async_trait]
pub trait ContentRepository: Send + Sync {
    async fn search(
        &self,
        query: &str,
        category: ContentCategory,
        page: u32,
    ) -> Result<SearchResult<ContentItem>, AppError>;

    async fn get_info(&self, id: ContentId) -> Result<ContentItem, AppError>;

    async fn download(&self, id: ContentId, file_id: u64) -> Result<Vec<u8>, AppError>;

    /// List popular content items in a category.
    async fn list_popular(
        &self,
        category: ContentCategory,
        page: u32,
    ) -> Result<SearchResult<ContentItem>, AppError>;

    /// List recently updated content items in a category.
    async fn list_recent(
        &self,
        category: ContentCategory,
        page: u32,
    ) -> Result<SearchResult<ContentItem>, AppError>;
}
