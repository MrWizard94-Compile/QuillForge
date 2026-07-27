//! M4: end-to-end primary user workflow (input → session → layout).

use qf_app::EditorController;
use qf_input::KeyEvent;
use qf_core::TextOffset;

#[test]
fn type_enter_undo_redo_and_layout() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("e2e.log");
    let mut ed = EditorController::create_default(&journal).unwrap();
    ed.ensure_editable_paragraph().unwrap();

    for ch in "abc".chars() {
        let r = ed.handle_key(KeyEvent::Char(ch)).unwrap();
        assert!(r.committed);
        assert!(r.layout_changed);
    }
    assert_eq!(ed.focused_text().as_deref(), Some("abc"));
    assert_eq!(ed.selection().offset, TextOffset(3));

    ed.handle_key(KeyEvent::Backspace).unwrap();
    assert_eq!(ed.focused_text().as_deref(), Some("ab"));

    ed.handle_key(KeyEvent::Enter).unwrap();
    assert_eq!(ed.focused_text().as_deref(), Some(""));
    assert_eq!(ed.session.state().document.blocks.len(), 2);

    ed.handle_text("second").unwrap();
    assert_eq!(ed.focused_text().as_deref(), Some("second"));

    ed.handle_key(KeyEvent::Undo).unwrap();
    // Undo removes last text insert on second paragraph
    assert_eq!(ed.focused_text().as_deref(), Some(""));

    ed.handle_key(KeyEvent::Redo).unwrap();
    assert_eq!(ed.focused_text().as_deref(), Some("second"));

    assert!(!ed.layout_tree.pages.is_empty());
    let h1 = ed.layout_tree.content_hash();
    let _ = ed.handle_pointer(qf_input::PointerEvent::Click {
        block_id: ed.selection().block_id.unwrap(),
        offset: TextOffset(0),
    });
    // Pointer does not mutate canonical state or layout.
    assert_eq!(ed.layout_tree.content_hash(), h1);
    assert_eq!(ed.selection().offset, TextOffset(0));

    let pkg = dir.path().join("e2e.qfdoc");
    ed.save(&pkg).unwrap();
    assert!(pkg.exists());
}
