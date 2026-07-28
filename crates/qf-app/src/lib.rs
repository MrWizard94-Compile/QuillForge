//! Quillforge application library — editor controller + Word-class product surface.
//!
//! Canonical path remains:
//! `Key/Pointer → Command → DocumentSession → Journal → Kernel → Layout (derived)`.
//!
//! Product priority: Word-class document editor UX without Microsoft branding
//! (`docs/28-product-surface.md`, [`product_surface`]).

mod editor;
mod product_surface;

pub use editor::{EditResult, EditorController, EditorError};
pub use product_surface::{
    CommandAction, CommandBarTab, CommandGroup, DocumentViewMode, ProductSurface, StatusBarModel,
};
