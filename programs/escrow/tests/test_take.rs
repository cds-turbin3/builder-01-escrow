//! Integration tests for the escrow `take` instruction, built via `BuildableIx`.

mod common;

use anchor_litesvm::{AnchorLiteSVM, AssertionHelpers, Pubkey, TestHelpers};
use common::{DEPOSIT, RECEIVE, SEED};

const PROGRAM_SO: &[u8] = include_bytes!("../../../target/deploy/escrow.so");

/// Happy path: after `take`, the taker should hold the whole vault
/// (`DEPOSIT` of mint_a), the maker should hold the asking price (`RECEIVE` of
/// mint_b), and the vault should be closed. Runs at day 89 so we also prove
/// the boundary case: `take` is still allowed on the last day of the 90-day
/// expiry window (see `EXPIRY_PERIOD_IN_SECONDS`).
#[test]
fn take_and_close_succeeds_late_in_window() {
    // Arrange
    let mut ctx = AnchorLiteSVM::build_with_program(escrow::ID, "escrow", PROGRAM_SO);
    let (bundle, maker, taker) = common::setup(&mut ctx, SEED);
    bundle.alias_all(&mut ctx);

    // The escrow must exist and be funded before it can be taken.
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
        .print_logs_structured();

    // Day 89 of a 90-day window: still inside the allowed range. Picking a
    // value this close to the edge guards against an off-by-one in the
    // expiry check (`< expiry` vs `<= expiry`).
    ctx.svm.advance_days(89);

    // Act
    ctx.tx(&[&taker])
        .build(bundle, escrow::instruction::Take {})
        .send_ok()
        .print_logs_structured();

    // Assert
    // Taker received the full vault contents; maker received the asking price.
    ctx.svm.assert_token_balance(&bundle.taker_ata_a, DEPOSIT);
    ctx.svm.assert_token_balance(&bundle.maker_ata_b, RECEIVE);
    // Taker's mint_b ATA drained to zero (they spent the full RECEIVE amount,
    // which is exactly what `setup` minted into it).
    ctx.svm.assert_token_balance(&bundle.taker_ata_b, 0);
    // Vault account closed; lamports returned to the maker by Anchor's
    // `close = maker` constraint on the vault.
    ctx.svm.assert_account_closed(&bundle.vault);
}

/// Negative path (time-based): once the escrow has expired, `take` must be
/// rejected with `EscrowExpired`. The maker's recourse after expiry is
/// `refund`, not someone else's `take`.
#[test]
fn take_and_close_fails_after_expiry() {
    // Arrange
    let mut ctx = AnchorLiteSVM::build_with_program(escrow::ID, "escrow", PROGRAM_SO);
    let (bundle, maker, taker) = common::setup(&mut ctx, SEED);
    bundle.alias_all(&mut ctx);

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
        .print_logs_structured();

    // Jump well past the 90-day expiry window; 199 days is arbitrary, the
    // point is "definitely expired" (any value > 90 would do).
    ctx.svm.advance_days(199);

    // Act + Assert: specifically `EscrowExpired` (not a generic constraint
    // failure), so a future refactor that "still rejects" but for the wrong
    // reason gets caught.
    ctx.tx(&[&taker])
        .build(bundle, escrow::instruction::Take {})
        .send_err_named("EscrowExpired")
        .print_logs_structured();
}

/// Negative path: with a valid escrow in place, a wrong `vault` must be
/// rejected. We substitute a freshly-generated pubkey for `vault`; since
/// nothing was ever initialized at that address, Anchor's account check fires
/// with `AccountNotInitialized` before we get anywhere near the transfer.
/// (A different swap, e.g. an existing-but-wrong token account, would
/// surface a different error such as a constraint or owner mismatch; the
/// specific error matters because it identifies *which* guard caught us.)
#[test]
fn take_rejects_wrong_vault() {
    // Arrange
    let mut ctx = AnchorLiteSVM::build_with_program(escrow::ID, "escrow", PROGRAM_SO);
    let (bundle, maker, taker) = common::setup(&mut ctx, SEED);
    bundle.alias_all(&mut ctx);

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
        .print_logs_structured();

    let wrong_vault = Pubkey::new_unique();
    ctx.alias(wrong_vault, "WrongVault");

    // Act + Assert
    ctx.tx(&[&taker])
        .build_with(
            bundle,
            escrow::instruction::Take {},
            |a| a.vault = wrong_vault,
        )
        .send_err_named("AccountNotInitialized")
        .print_logs_structured();
}
