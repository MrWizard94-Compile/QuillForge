//! Quillforge core primitives.
//!
//! This crate is the foundation of the workspace: IDs, versioning, preserved maps,
//! commands, operations, and shared error types. It MUST NOT depend on other `qf-*` crates.

mod command;
mod error;
mod hash;
mod ids;
mod operation;
mod preserved;
mod version;

pub use command::*;
pub use error::*;
pub use hash::*;
pub use ids::*;
pub use operation::*;
pub use preserved::*;
pub use version::*;

/// Trait implemented by command handlers that expand intent into operations.
///
/// Handlers MUST NOT write the journal or mutate the kernel. They read state and
/// return fully-specified operations (all new IDs pre-generated).
pub trait CommandHandler: Send + Sync {
    fn execute(&self, command: Command) -> Result<Vec<Operation>, CommandError>;
}

/// Deterministic or production ID source. Kernel apply MUST NOT allocate IDs.
pub trait IdGen {
    fn new_uuid(&mut self) -> uuid::Uuid;
}

/// Production ID generator using UUID v7 when available, else v4.
#[derive(Debug, Default)]
pub struct UuidGen;

impl IdGen for UuidGen {
    fn new_uuid(&mut self) -> uuid::Uuid {
        uuid::Uuid::new_v4()
    }
}

/// Test ID generator with monotonic counter for determinism.
#[derive(Debug)]
pub struct SeqIdGen {
    next: u128,
}

impl Default for SeqIdGen {
    fn default() -> Self {
        Self { next: 1 }
    }
}

impl IdGen for SeqIdGen {
    fn new_uuid(&mut self) -> uuid::Uuid {
        let n = self.next;
        self.next += 1;
        uuid::Uuid::from_u128(n)
    }
}
