//! Integration tests for the escrow `take` instruction, built via `BuildableIx`.

mod common;

use anchor_litesvm::{AnchorLiteSVM, AssertionHelpers, Pubkey};
use common::{DEPOSIT, RECEIVE, SEED};

const PROGRAM_SO: &[u8] = include_bytes!("../../../target/deploy/escrow.so");

/// Happy path: after `take`, the taker should hold the whole vault
/// (`DEPOSIT` of mint_a), the maker should hold the asking price (`RECEIVE` of
/// mint_b), and the vault should be closed.
#[test]
fn take_swaps_tokens_and_closes_vault() {
    // Arrange
    let mut ctx = AnchorLiteSVM::build_with_program(escrow::ID, PROGRAM_SO);
    let (bundle, maker, taker) = common::setup(&mut ctx, SEED);

    // Arrange: make (the escrow must exist and be funded before it can be taken)
    let make_ix = ctx.program().build_ix(
        bundle,
        escrow::instruction::Make { seed: SEED, receive: RECEIVE, deposit: DEPOSIT },
    );
    ctx.execute_instruction(make_ix, &[&maker])
        .expect("make transaction should submit")
        .assert_success();

    // Act
    let take_ix = ctx.program().build_ix(bundle, escrow::instruction::Take {});
    let result = ctx
        .execute_instruction(take_ix, &[&taker])
        .expect("take transaction should submit");

    // Assert
    result.assert_success();
    ctx.svm.assert_token_balance(&bundle.taker_ata_a, DEPOSIT);
    ctx.svm.assert_token_balance(&bundle.maker_ata_b, RECEIVE);
    ctx.svm.assert_token_balance(&bundle.taker_ata_b, 0);
    ctx.svm.assert_account_closed(&bundle.vault);
}
