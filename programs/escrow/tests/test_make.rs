//! Integration tests for the escrow `make` instruction, built via `BuildableIx`.

mod common;

use anchor_litesvm::{AnchorLiteSVM, AssertionHelpers, Pubkey};
use common::{DEPOSIT, RECEIVE, SEED};

const PROGRAM_SO: &[u8] = include_bytes!("../../../target/deploy/escrow.so");

/// Happy path: `make` creates the escrow account and moves the deposit into the vault.
#[test]
fn make_creates_escrow_and_funds_vault() {
    // Arrange
    let mut ctx = AnchorLiteSVM::build_with_program(escrow::ID, "escrow", PROGRAM_SO);
    let (bundle, maker, _taker) = common::setup(&mut ctx, SEED);
    bundle.alias_all(&mut ctx);

    // Act
    ctx.tx(&[&maker])
        .build(
            bundle,
            escrow::instruction::Make {
                seed: SEED,
                receive: RECEIVE,
                deposit: DEPOSIT,
            },
        )
        .send_ok()
        .print_markdown_pair();

    // Assert
    // Escrow account was created and populated from the instruction args
    // (every field that round-trips through the program is checked here; if a
    // future change shuffles fields in `state::Escrow`, this fixes the layout
    // contract for `make`).
    let escrow_acct: escrow::Escrow = ctx
        .get_account(&bundle.escrow)
        .expect("escrow account should exist");
    assert_eq!(escrow_acct.seed, SEED);
    assert_eq!(escrow_acct.maker, bundle.maker);
    assert_eq!(escrow_acct.mint_a, bundle.mint_a);
    assert_eq!(escrow_acct.mint_b, bundle.mint_b);
    assert_eq!(escrow_acct.receive, RECEIVE);
    // The full `DEPOSIT` moved from the maker's source ATA into the vault;
    // checking both ends catches a `transfer` that fires with the wrong amount
    // or wrong direction.
    ctx.svm.assert_token_balance(&bundle.vault, DEPOSIT);
    ctx.svm.assert_token_balance(&bundle.maker_ata_a, 0);
}

/// Negative path: a wrong escrow PDA must be rejected by Anchor's seeds
/// constraint. We swap in a freshly-generated pubkey for `escrow` (so the
/// `seeds = [...]` check on the account fails) while leaving everything else
/// valid; the failure mode we expect is specifically `ConstraintSeeds`, not a
/// generic deserialization or ownership error.
#[test]
fn make_rejects_wrong_escrow_pda() {
    // Arrange
    let mut ctx = AnchorLiteSVM::build_with_program(escrow::ID, "escrow", PROGRAM_SO);
    let (bundle, maker, _taker) = common::setup(&mut ctx, SEED);
    bundle.alias_all(&mut ctx);
    let wrong_escrow = Pubkey::new_unique();
    ctx.alias(wrong_escrow, "WrongEscrow");

    // Act + Assert: `send_err_named` is the assertion. It panics if the
    // transaction succeeds or fails with anything other than a
    // `ConstraintSeeds` error.
    ctx.tx(&[&maker])
        .build_with(
            bundle,
            escrow::instruction::Make {
                seed: SEED,
                receive: RECEIVE,
                deposit: DEPOSIT,
            },
            |a| a.escrow = wrong_escrow,
        )
        .send_err_named("ConstraintSeeds")
        .print_markdown_pair();
}
