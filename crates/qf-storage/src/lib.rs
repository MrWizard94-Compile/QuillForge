//! `.qfdoc` package format, atomic writes, recovery, migrations, and
//! [`DocumentSession`] — the production commit coordinator.
//!
//! # Hard recovery guarantees
//!
//! - Every committed operation survives power loss (journal fsync).
//! - Every package write is atomic (temp + fsync + rename + LKG).
//! - Recovery reconstructs from journal + last checkpoint + package metadata.
//! - Corruption never silently overwrites last-known-good.
//!
//! See `docs/06-recovery.md`, `docs/07-migration.md`, `docs/18-storage-qfdoc.md`.

mod atomic;
mod migration;
mod package;
mod session;
mod zip_io;

pub use atomic::{lkg_path, write_bytes_atomic};
pub use migration::{Migration, MigrationRegistry, Migrator};
pub use package::{
    CheckpointBlob, MigrationRecord, OpenOptions, OpenOutcome, Package, PackageMetadata,
    PackageStore, APP_VERSION, FORMAT_VERSION, MIME_TYPE,
};
pub use session::{CommitOutcome, DocumentSession, UndoService};
pub use zip_io::{
    quarantine_copy, read_qfdoc, read_qfdoc_with_limits, validate_member_name, write_qfdoc,
    ReadLimits, MAX_ENTRY_BYTES, MAX_UNCOMPRESSED_BYTES, MAX_ZIP_ENTRIES,
};
