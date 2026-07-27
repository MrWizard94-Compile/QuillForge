//! Stable identity types for documents, blocks, operations, and actors.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! id_newtype {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new(gen: &mut dyn crate::IdGen) -> Self {
                Self(gen.new_uuid())
            }

            pub fn from_uuid(u: Uuid) -> Self {
                Self(u)
            }

            pub fn as_uuid(&self) -> Uuid {
                self.0
            }
        }
    };
}

id_newtype!(DocumentId);
id_newtype!(BlockId);
id_newtype!(SpanId);
id_newtype!(SectionId);
id_newtype!(OpId);
id_newtype!(ActorId);
id_newtype!(SessionId);
id_newtype!(AssetId);
id_newtype!(CommentId);
id_newtype!(SuggestionId);
id_newtype!(RevisionId);
id_newtype!(SourceId);
id_newtype!(CommandId);
id_newtype!(PrincipalId);

/// Monotonic per-document journal sequence number. Starts at 1 for the first op.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct OpSeq(pub u64);

impl OpSeq {
    pub const ZERO: Self = Self(0);

    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

/// Reverse-DNS plugin identifier, e.g. `com.example.math`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginId(pub String);

/// Capability string grant, e.g. `block.custom.register`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Capability(pub String);

/// Unicode scalar offset within a block's projected editable text.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct TextOffset(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OffsetRange {
    pub start: TextOffset,
    pub end: TextOffset,
}

/// Wall-clock timestamp for audit only — never used in apply semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Timestamp {
    /// Unix epoch milliseconds.
    pub unix_ms: i64,
}

/// Stable color token id (theme resolves to RGBA).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ColorId(pub u32);
