// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use gnomex_domain::DomainError;

/// Application-layer errors that wrap domain errors and port failures.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("domain error: {0}")]
    Domain(#[from] DomainError),

    #[error("repository error: {0}")]
    Repository(String),

    #[error("installation error: {0}")]
    Install(String),

    #[error("shell communication error: {0}")]
    Shell(String),

    #[error("settings error: {0}")]
    Settings(String),

    #[error("storage error: {0}")]
    Storage(String),
}
