//! Integration tests for the escrow `make` instruction, built via `BuildableIx`.

mod common;

use anchor_litesvm::{Program, Pubkey};
use common::EscrowBundle;

/// Smoke test: the `From` / `BuildableIx` glue compiles and `build_ix` resolves
/// each args struct to the right accounts struct. Pure construction, no runtime.
#[test]
fn buildable_ix_glue_typechecks() {
    // Arrange
    let bundle = EscrowBundle {
        maker: Pubkey::new_unique(),
        taker: Pubkey::new_unique(),
        mint_a: Pubkey::new_unique(),
        mint_b: Pubkey::new_unique(),
        maker_ata_a: Pubkey::new_unique(),
        maker_ata_b: Pubkey::new_unique(),
        taker_ata_a: Pubkey::new_unique(),
        taker_ata_b: Pubkey::new_unique(),
        escrow: Pubkey::new_unique(),
        vault: Pubkey::new_unique(),
        token_program: Pubkey::new_unique(),
        associated_token_program: Pubkey::new_unique(),
        system_program: Pubkey::new_unique(),
    };
    let program = Program::new(escrow::ID);

    // Act
    let make_ix = program.build_ix(
        bundle,
        escrow::instruction::Make { seed: 1, receive: 2, deposit: 3 },
    );
    let take_ix = program.build_ix(bundle, escrow::instruction::Take {});
    let refund_ix = program.build_ix(bundle, escrow::instruction::Refund {});

    // Assert
    assert_eq!(make_ix.program_id, escrow::ID);
    assert_eq!(take_ix.program_id, escrow::ID);
    assert_eq!(refund_ix.program_id, escrow::ID);
    assert_eq!(make_ix.accounts.len(), 9);
    assert_eq!(take_ix.accounts.len(), 12);
    assert_eq!(refund_ix.accounts.len(), 7);
    assert!(!make_ix.data.is_empty());
}
