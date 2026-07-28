//! Quillforge core primitives (WPAI Software).
//!
//! Foundation of the workspace: IDs, versioning, preserved maps, commands,
//! operations, brand identity, and shared error types. This crate MUST NOT
//! depend on other `qf-*` crates.
//!
//! # Canonical mutation path
//!
//! ```text
//! Command → Operations → Journal → Kernel → Derived State
//! ```
//!
//! Only operations are durable and replayable. Commands are ephemeral intent.

mod brand;
mod command;
mod error;
mod hash;
mod ids;
mod operation;
mod preserved;
mod version;

pub use brand::*;
pub use command::*;
pub use error::*;
pub use hash::*;
pub use ids::*;
pub use operation::*;
pub use preserved::*;
pub use version::*;

/// Trait implemented by command handlers that expand intent into operations.
///
/// Handlers MUST NOT write the journal or mutate the kernel. They read state
/// (via closed-over handles or free functions) and return fully-specified
/// operations with all new IDs pre-generated.
pub trait CommandHandler: Send + Sync {
    fn execute(&self, command: Command) -> Result<Vec<Operation>, CommandError>;
}

/// Deterministic or production ID source. Kernel apply MUST NOT allocate IDs.
pub trait IdGen {
    fn new_uuid(&mut self) -> uuid::Uuid;
}

/// Production ID generator (UUID v4).
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
        self.next = self.next.saturating_add(1);
        uuid::Uuid::from_u128(n)
    }
}

/// Result of a successful commit through the coordinator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitResult {
    pub first_seq: OpSeq,
    pub last_seq: OpSeq,
}

/// Next sequence after `last` (first committed op is always 1).
pub fn next_seq(last: OpSeq) -> OpSeq {
    if last == OpSeq::ZERO {
        OpSeq(1)
    } else {
        last.next()
    }
}
