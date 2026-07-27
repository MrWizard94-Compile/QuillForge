//! Search index rebuild equivalence (Logseq-query mental model: derived only).

use qf_core::{
    BlockBodyBytes, BlockId, OpSeq, Operation, ParentRef, SeqIdGen, TextOffset,
};
use qf_kernel::Kernel;
use qf_search::{SearchEngine, SearchIngestor, TokenQuery};

#[test]
fn event_rebuild_matches_full_rebuild() {
    let mut gen = SeqIdGen::default();
    let mut k = Kernel::fresh(
        qf_core::DocumentId::new(&mut gen),
        qf_core::SectionId::new(&mut gen),
        qf_core::ActorId::new(&mut gen),
        qf_core::Timestamp { unix_ms: 0 },
    );
    let bid = BlockId::new(&mut gen);
    k.apply_batch(&[
        Operation::InsertBlock {
            block_id: bid,
            parent: ParentRef::DocumentRoot,
            index: 0,
            body: BlockBodyBytes {
                type_name: "paragraph".into(),
                payload: Vec::new(),
            },
        },
        Operation::InsertText {
            block_id: bid,
            at: TextOffset(0),
            text: "alpha beta gamma".into(),
        },
    ])
    .unwrap();

    let mut eng_a = SearchEngine::new();
    eng_a.rebuild(k.state());

    let mut eng_b = SearchEngine::new();
    eng_b.on_events(&[], k.state()); // rebuild path

    let qa = eng_a.query_token(&TokenQuery {
        text: "beta".into(),
    });
    let qb = eng_b.query_token(&TokenQuery {
        text: "beta".into(),
    });
    assert_eq!(qa.len(), 1);
    assert_eq!(qb.len(), 1);
    assert_eq!(qa[0].block_id, bid);
    assert_eq!(eng_a.state_gen, OpSeq(2));
    assert_eq!(eng_b.state_gen, eng_a.state_gen);
}
