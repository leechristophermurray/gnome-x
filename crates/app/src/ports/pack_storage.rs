// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::AppError;
use gnomex_domain::ExperiencePack;

/// Summary of a stored experience pack (for listing without full load).
#[derive(Debug, Clone)]
pub struct PackSummary {
    pub id: String,
    pub name: String,
    pub author: String,
    pub description: String,
}

/// Port: persist and retrieve experience packs.
pub trait PackStorage: Send + Sync {
    fn save_pack(&self, pack: &ExperiencePack) -> Result<String, AppError>;
    fn load_pack(&self, id: &str) -> Result<ExperiencePack, AppError>;
    fn list_packs(&self) -> Result<Vec<PackSummary>, AppError>;
    fn delete_pack(&self, id: &str) -> Result<(), AppError>;

    /// Export a pack as a portable `.gnomex-pack.tar.gz` archive.
    /// The `screenshot` is optional PNG data to bundle alongside the manifest.
    /// Returns the raw archive bytes.
    fn export_pack(&self, id: &str, screenshot: Option<&[u8]>) -> Result<Vec<u8>, AppError>;

    /// Import a pack from a `.gnomex-pack.tar.gz` archive.
    /// Returns the pack ID and any bundled screenshot PNG data.
    fn import_pack(&self, archive: &[u8]) -> Result<(String, Option<Vec<u8>>), AppError>;
}
