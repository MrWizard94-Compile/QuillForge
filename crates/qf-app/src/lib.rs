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
    decode_image_rgba, decode_image_size, demo_png_bytes, product_mark_png,
    product_mark_png_for_document, PageChrome, PLACEHOLDER_PNG, PRODUCT_MARK_PNG,
};
pub use product_surface::{
    CommandAction, CommandBarTab, CommandGroup, DocumentViewMode, ProductSurface, StatusBarModel,
};
pub use review::{ReviewPanel, SuggestionRow};
pub use shell_session::ShellSession;
pub use qf_search::{
    EmbeddingPack, HitPath, SearchHit, SearchMode, SearchQuery, DEFAULT_DENSE_DIM,
    EMBED_ALGO_HASH_NGRAM_V1, EMBED_ALGO_NEURAL_HASH_V1, EMBED_ALGO_ONNX_V1, NEURAL_HASH_DIM,
};
