//! Integration tests for the escrow `refund` instruction, built via `BuildableIx`.

mod common;

use anchor_litesvm::{AnchorLiteSVM, AssertionHelpers, Pubkey, TestHelpers};
use common::{pretty_log, DEPOSIT, RECEIVE, SEED};

const PROGRAM_SO: &[u8] = include_bytes!("../../../target/deploy/escrow.so");

/// Happy path: `refund` returns the deposit to the maker and closes the vault
/// and escrow accounts.
#[test]
fn refund_returns_deposit_and_closes_escrow() {
    // Arrange
    let mut ctx = AnchorLiteSVM::build_with_program(escrow::ID, PROGRAM_SO);
    let (bundle, maker, _taker) = common::setup(&mut ctx, SEED);

    // Arrange: make
    let make_ix = ctx.program().build_ix(
        bundle,
        escrow::instruction::Make {
            seed: SEED,
            receive: RECEIVE,
            deposit: DEPOSIT,
        },
    );
    ctx.execute_instruction(make_ix, &[&maker])
        .expect("make transaction should submit")
        .assert_success();

    ctx.svm.advance_days(199);

    // Act
    // `refund` declares no Signer; the maker signs only as the transaction fee payer.
    let refund_ix = ctx
        .program()
        .build_ix(bundle, escrow::instruction::Refund {});
    let result = ctx
        .execute_instruction(refund_ix, &[&maker])
        .expect("refund transaction should submit");

    // Assert
    result.assert_success();
    pretty_log(&result, "refund_returns_deposit_and_closes_escrow");
    // result.print_logs();
    // result.print_logs_structured();
    ctx.svm.assert_token_balance(&bundle.maker_ata_a, DEPOSIT);
    ctx.svm.assert_account_closed(&bundle.vault);
    ctx.svm.assert_account_closed(&bundle.escrow);
}

#[test]
fn refund_fails_before_expiry() {
    // Arrange
    let mut ctx = AnchorLiteSVM::build_with_program(escrow::ID, PROGRAM_SO);
    let (bundle, maker, _taker) = common::setup(&mut ctx, SEED);

    // Arrange: make
    let make_ix = ctx.program().build_ix(
        bundle,
        escrow::instruction::Make {
            seed: SEED,
            receive: RECEIVE,
            deposit: DEPOSIT,
        },
    );
    ctx.execute_instruction(make_ix, &[&maker])
        .expect("make transaction should submit")
        .assert_success();

    ctx.svm.advance_days(19);

    // Act
    // `refund` declares no Signer; the maker signs only as the transaction fee payer.
    let refund_ix = ctx
        .program()
        .build_ix(bundle, escrow::instruction::Refund {});
    let result = ctx
        .execute_instruction(refund_ix, &[&maker])
        .expect("refund transaction should submit");

    // Assert
    result.assert_anchor_error("EscrowNotExpired");
    pretty_log(&result, "refund_fails_before_expiry");
    // result.print_logs();
    // result.print_logs_structured();
}

/// Negative path: with a valid escrow in place, a wrong `maker` must be rejected.
#[test]
fn refund_rejects_wrong_maker() {
    // Arrange
    let mut ctx = AnchorLiteSVM::build_with_program(escrow::ID, PROGRAM_SO);
    let (bundle, maker, _taker) = common::setup(&mut ctx, SEED);

    // Arrange: make
    let make_ix = ctx.program().build_ix(
        bundle,
        escrow::instruction::Make {
            seed: SEED,
            receive: RECEIVE,
            deposit: DEPOSIT,
        },
    );
    ctx.execute_instruction(make_ix, &[&maker])
        .expect("make transaction should submit")
        .assert_success();

    ctx.svm.advance_days(199);

    let wrong_maker = Pubkey::new_unique();

    // Act
    let refund_ix = ctx
        .program()
        .build_ix_with(bundle, escrow::instruction::Refund {}, |a| {
            a.maker = wrong_maker
        });
    let result = ctx
        .execute_instruction(refund_ix, &[&maker])
        .expect("refund transaction should submit");
    pretty_log(&result, "refund_rejects_wrong_maker");
    // result.print_logs();
    // result.print_logs_structured();

    // Assert
    result.assert_anchor_error("ConstraintTokenOwner");
}
