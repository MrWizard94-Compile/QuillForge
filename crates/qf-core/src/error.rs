//! Shared error types for the command path and apply path.

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CommandError {
    #[error("invalid command: {0}")]
    Invalid(String),
    #[error("invalid target")]
    InvalidTarget,
    #[error("stale target")]
    StaleTarget,
    #[error("forbidden")]
    Forbidden,
    #[error("region locked")]
    Locked,
    #[error("missing capability")]
    MissingCapability,
    #[error("document not writable")]
    NotWritable,
    #[error("nothing to undo")]
    NothingToUndo,
    #[error("nothing to redo")]
    NothingToRedo,
    #[error("handler error: {0}")]
    Handler(String),
    #[error("dry-run apply failed: {0}")]
    DryRun(String),
    #[error("journal error: {0}")]
    Journal(String),
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ApplyError {
    #[error("invariant violated: {0}")]
    Invariant(String),
    #[error("missing entity")]
    MissingEntity,
    #[error("type mismatch")]
    TypeMismatch,
    #[error("offset out of range")]
    OffsetOutOfRange,
    #[error("schema error: {0}")]
    Schema(String),
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum JournalError {
    #[error("io: {0}")]
    Io(String),
    #[error("corrupt record: {0}")]
    Corrupt(String),
    #[error("hash chain broken at seq {0}")]
    HashChain(u64),
    #[error("torn trailing frame truncated")]
    TornFrame,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MigrationError {
    #[error("no migration path from {from} to {to}")]
    NoPath { from: String, to: String },
    #[error("migration failed: {0}")]
    Failed(String),
    #[error("validation failed: {0}")]
    Validation(String),
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StorageError {
    #[error("io: {0}")]
    Io(String),
    #[error("corrupt package: {0}")]
    Corrupt(String),
    #[error("unsupported version {0}")]
    UnsupportedVersion(String),
    #[error("encryption: {0}")]
    Encryption(String),
    #[error("migration: {0}")]
    Migration(String),
}
