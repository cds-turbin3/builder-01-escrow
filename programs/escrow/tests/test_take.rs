//! Integration tests for the escrow `take` instruction.
//!
//! `take` is the settlement: the taker sends `escrow.receive` of mint_b to the
//! maker, and in return the vault's full mint_a balance is released to the
//! taker; the vault then closes (rent back to the maker). `take` is gated by a
//! 90-day expiry window, so these tests exercise the time-warp feature
//! (`advance_days`) to land on both sides of the boundary.
//!
//! Each test threads a [`Report`]; the Markdown lands in
//! `target/md-reports/<slug>.md`.

mod common;

use anchor_litesvm::{AnchorLiteSVM, MarkdownBlock, Pubkey, Report, TestHelpers};
use common::{balances, setup, DEPOSIT, RECEIVE, SEED};

const PROGRAM_SO: &[u8] = include_bytes!("../../../target/deploy/escrow.so");

/// Happy path, run at day 89: the taker receives the whole vault (`DEPOSIT` of
/// mint_a), the maker receives the asking price (`RECEIVE` of mint_b), and the
/// vault closes. Day 89 of a 90-day window is deliberate: it pins the boundary
/// (`< expiry` vs `<= expiry`) so an off-by-one in the expiry check is caught.
#[test]
fn take_and_close_succeeds_late_in_window() {
    let mut md = Report::new(
        "Escrow: take settles the swap on the last day of the window",
        "With an open escrow, the taker calls take: they pay `receive` of mint_b \
         to the maker and receive the vault's full mint_a deposit; the vault then \
         closes. Run at day 89 of the 90-day window to pin the expiry boundary: \
         take is still allowed on the last day.",
    );

    let mut ctx = AnchorLiteSVM::build_with_program(escrow::ID, "escrow", PROGRAM_SO);
    let w = setup(&mut ctx, SEED);

    md.step("Setup: maker opens the escrow (make funds the vault)");
    ctx.tx(&[&w.maker])
        .build(
            w.bundle,
            escrow::instruction::Make { seed: SEED, receive: RECEIVE, deposit: DEPOSIT },
        )
        .send_ok()
        .print_markdown_pair();
    md.snapshot("after make", &balances(&ctx, &w));

    md.step("Advance to day 89 (still inside the 90-day window)");
    md.note("Picking a value one day short of expiry guards against an off-by-one in the `< expiry` check.");
    ctx.svm.advance_days(89);

    md.step("Action: taker calls take");
    ctx.tx(&[&w.taker])
        .build(w.bundle, escrow::instruction::Take {})
        .send_ok()
        .print_markdown_pair();

    md.step("After: the two-sided swap settled; vault closed");
    md.snapshot("after take", &balances(&ctx, &w));
    md.check("taker received the deposit (mint_a)", Some(DEPOSIT), ctx.svm.token_balance(&w.bundle.taker_ata_a));
    md.check("maker received the price (mint_b)", Some(RECEIVE), ctx.svm.token_balance(&w.bundle.maker_ata_b));
    md.check("taker's mint_b fully spent", Some(0), ctx.svm.token_balance(&w.bundle.taker_ata_b));
    md.check("vault account closed", false, ctx.account_exists(&w.bundle.vault));

    // The authority story across both sends: makers and takers sign their own
    // inbound transfers, the Escrow PDA signs the vault release and close via
    // invoke_signed. And the account index: the standing structure of every
    // account the swap touched, classified by owner and authority.
    md.authority(&ctx.authority_story());
    md.block("account index", ctx.account_index());
}

/// Negative (time-based): once the escrow has expired, `take` must be rejected
/// with `EscrowExpired`. The maker's recourse after expiry is `refund`, not
/// someone else's `take`.
#[test]
fn take_and_close_fails_after_expiry() {
    let mut md = Report::new(
        "Escrow: take is rejected after the escrow expires",
        "Past the 90-day window the offer is dead: take must fail with \
         EscrowExpired (not a generic constraint error), so a refactor that \
         still rejects but for the wrong reason is caught. After expiry the \
         maker's path is refund, not a taker's take.",
    );

    let mut ctx = AnchorLiteSVM::build_with_program(escrow::ID, "escrow", PROGRAM_SO);
    let w = setup(&mut ctx, SEED);

    md.step("Setup: maker opens the escrow");
    ctx.tx(&[&w.maker])
        .build(
            w.bundle,
            escrow::instruction::Make { seed: SEED, receive: RECEIVE, deposit: DEPOSIT },
        )
        .send_ok()
        .print_markdown_pair();

    md.step("Advance 199 days (definitely past the 90-day window)");
    ctx.svm.advance_days(199);

    md.step("Action: taker calls take on the expired escrow → must fail");
    let rejection = ctx
        .tx(&[&w.taker])
        .build(w.bundle, escrow::instruction::Take {})
        .send_err_named("EscrowExpired");
    md.block(
        "rejection logs",
        MarkdownBlock::Fenced { lang: "console".into(), body: rejection.logs_structured_string() },
    );

    md.step("After: nothing settled; the deposit is still in the vault");
    md.snapshot("balances", &balances(&ctx, &w));
    md.check("vault still holds the deposit", Some(DEPOSIT), ctx.svm.token_balance(&w.bundle.vault));
    md.check("escrow account still open", true, ctx.account_exists(&w.bundle.escrow));
}

/// Negative: a wrong `vault` must be rejected. We substitute a fresh pubkey;
/// since nothing was initialized at that address, Anchor's account check fires
/// `AccountNotInitialized` before reaching the transfer. The specific error
/// identifies *which* guard caught us.
#[test]
fn take_rejects_wrong_vault() {
    let mut md = Report::new(
        "Escrow: take rejects a wrong vault account",
        "Substituting an uninitialized pubkey for the vault must fail with \
         AccountNotInitialized (Anchor's account check), before any transfer. \
         The specific error name matters: it pins which guard fired.",
    );

    let mut ctx = AnchorLiteSVM::build_with_program(escrow::ID, "escrow", PROGRAM_SO);
    let w = setup(&mut ctx, SEED);

    md.step("Setup: maker opens the escrow");
    ctx.tx(&[&w.maker])
        .build(
            w.bundle,
            escrow::instruction::Make { seed: SEED, receive: RECEIVE, deposit: DEPOSIT },
        )
        .send_ok()
        .print_markdown_pair();

    let wrong_vault = Pubkey::new_unique();
    ctx.alias(wrong_vault, "WrongVault");

    md.step("Action: taker calls take with an uninitialized vault → must fail");
    let rejection = ctx
        .tx(&[&w.taker])
        .build_with(w.bundle, escrow::instruction::Take {}, |a| a.vault = wrong_vault)
        .send_err_named("AccountNotInitialized");
    md.block(
        "rejection logs",
        MarkdownBlock::Fenced { lang: "console".into(), body: rejection.logs_structured_string() },
    );

    md.step("After: the real escrow and vault are intact");
    md.check("vault still holds the deposit", Some(DEPOSIT), ctx.svm.token_balance(&w.bundle.vault));
}
