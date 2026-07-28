//! Quillforge application library — editor controller + Word-class product surface.
//!
//! Canonical path remains:
//! `Key/Pointer → Command → DocumentSession → Journal → Kernel → Layout (derived)`.
//!
//! Product priority: Word-class document editor UX without Microsoft branding
//! (`docs/28-product-surface.md`, [`product_surface`]).

mod browser_demo;
mod editor;
mod gui;
mod page_chrome;
mod product_surface;
mod review;
mod shell_session;

pub use browser_demo::{run_browser_sync_demo, run_browser_wss_demo};
pub use editor::{EditResult, EditorController, EditorError};
pub use gui::run_gui;
pub use page_chrome::{
    decode_image_rgba, decode_image_size, demo_png_bytes, PageChrome, PLACEHOLDER_PNG,
};
pub use product_surface::{
    CommandAction, CommandBarTab, CommandGroup, DocumentViewMode, ProductSurface, StatusBarModel,
};
pub use review::{ReviewPanel, SuggestionRow};
pub use shell_session::ShellSession;
pub use qf_search::{HitPath, SearchHit, SearchMode, SearchQuery};
