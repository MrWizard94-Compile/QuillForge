//! Quillforge application library — editor controller wiring input to session.
//!
//! Canonical path remains:
//! `Key/Pointer → Command → DocumentSession → Journal → Kernel → Layout (derived)`.

mod editor;

pub use editor::{EditResult, EditorController, EditorError};
