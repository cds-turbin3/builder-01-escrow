//! Integration tests for the escrow `make` instruction, built via `BuildableIx`.

mod common;

use anchor_litesvm::{AnchorLiteSVM, AssertionHelpers, Program, Pubkey};
use common::{EscrowBundle, DEPOSIT, RECEIVE, SEED};

const PROGRAM_SO: &[u8] = include_bytes!("../../../target/deploy/escrow.so");

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

/// Happy path: `make` creates the escrow account and moves the deposit into the vault.
#[test]
fn make_creates_escrow_and_funds_vault() {
    // Arrange
    let mut ctx = AnchorLiteSVM::build_with_program(escrow::ID, PROGRAM_SO);
    let (bundle, maker, _taker) = common::setup(&mut ctx, SEED);

    // Act
    let ix = ctx.program().build_ix(
        bundle,
        escrow::instruction::Make { seed: SEED, receive: RECEIVE, deposit: DEPOSIT },
    );
    let result = ctx
        .execute_instruction(ix, &[&maker])
        .expect("make transaction should submit");

    // Assert
    result.assert_success();
    let escrow_acct: escrow::Escrow =
        ctx.get_account(&bundle.escrow).expect("escrow account should exist");
    assert_eq!(escrow_acct.seed, SEED);
    assert_eq!(escrow_acct.maker, bundle.maker);
    assert_eq!(escrow_acct.mint_a, bundle.mint_a);
    assert_eq!(escrow_acct.mint_b, bundle.mint_b);
    assert_eq!(escrow_acct.receive, RECEIVE);
    ctx.svm.assert_token_balance(&bundle.vault, DEPOSIT);
    ctx.svm.assert_token_balance(&bundle.maker_ata_a, 0);
}

/// Negative path: a wrong escrow PDA must be rejected by Anchor's seeds constraint.
#[test]
fn make_rejects_wrong_escrow_pda() {
    // Arrange
    let mut ctx = AnchorLiteSVM::build_with_program(escrow::ID, PROGRAM_SO);
    let (bundle, maker, _taker) = common::setup(&mut ctx, SEED);
    let wrong_escrow = Pubkey::new_unique();

    // Act
    let ix = ctx.program().build_ix_with(
        bundle,
        escrow::instruction::Make { seed: SEED, receive: RECEIVE, deposit: DEPOSIT },
        |a| a.escrow = wrong_escrow,
    );
    let result = ctx
        .execute_instruction(ix, &[&maker])
        .expect("make transaction should submit");

    // Assert
    result.assert_anchor_error("ConstraintSeeds");
}
