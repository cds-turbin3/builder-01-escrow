//! Integration tests for the escrow `make` instruction.
//!
//! Each test threads a [`Report`]: it narrates intent and snapshots the
//! before/after token balances, with `check`s doubling as the assertions. The
//! Markdown lands in `target/md-reports/<slug>.md`.

mod common;

use anchor_litesvm::{AnchorLiteSVM, MarkdownBlock, Pubkey, Report, TestHelpers};
use common::{balances, setup, DEPOSIT, RECEIVE, SEED};

const PROGRAM_SO: &[u8] = include_bytes!("../../../target/deploy/escrow.so");

/// Happy path: `make` creates the escrow account and moves the deposit into the
/// vault.
#[test]
fn make_creates_escrow_and_funds_vault() {
    let mut md = Report::new(
        "Escrow: make creates the escrow and funds the vault",
        "The maker opens an escrow offering `deposit` of mint_a in exchange for \
         `receive` of mint_b. `make` records the terms in the escrow account and \
         moves the full deposit from the maker's source ATA into the vault \
         (an ATA owned by the escrow PDA).",
    );

    let mut ctx = AnchorLiteSVM::build_with_program(escrow::ID, "escrow", PROGRAM_SO);
    let w = setup(&mut ctx, SEED);

    md.step("Before: maker holds the deposit, vault does not exist yet");
    md.snapshot("balances", &balances(&ctx, &w));

    md.step("Action: maker calls make(seed, receive, deposit)");
    ctx.tx(&[&w.maker])
        .build(
            w.bundle,
            escrow::instruction::Make { seed: SEED, receive: RECEIVE, deposit: DEPOSIT },
        )
        .send_ok()
        .print_markdown_pair();

    md.step("After: escrow records the terms; the deposit sits in the vault");
    md.snapshot("balances", &balances(&ctx, &w));

    // The escrow account round-trips the instruction args. If a future change
    // shuffles `state::Escrow`, these checks pin the layout contract for `make`.
    let escrow_acct: escrow::Escrow = ctx.get_account(&w.bundle.escrow).expect("escrow exists");
    md.check("escrow.seed", SEED, escrow_acct.seed);
    md.check("escrow.maker", w.bundle.maker, escrow_acct.maker);
    md.check("escrow.mint_a", w.bundle.mint_a, escrow_acct.mint_a);
    md.check("escrow.mint_b", w.bundle.mint_b, escrow_acct.mint_b);
    md.check("escrow.receive", RECEIVE, escrow_acct.receive);

    // The full deposit moved maker -> vault; checking both ends catches a
    // transfer with the wrong amount or direction.
    md.check("vault holds the deposit", Some(DEPOSIT), ctx.svm.token_balance(&w.bundle.vault));
    md.check("maker source drained", Some(0), ctx.svm.token_balance(&w.bundle.maker_ata_a));
}

/// Negative: a wrong escrow PDA must be rejected by Anchor's `seeds` constraint.
/// We swap in a fresh pubkey for `escrow` (so the `seeds = [...]` check fails)
/// while leaving everything else valid; the expected failure is specifically
/// `ConstraintSeeds`, not a generic deserialization or ownership error.
#[test]
fn make_rejects_wrong_escrow_pda() {
    let mut md = Report::new(
        "Escrow: make rejects a wrong escrow PDA",
        "The escrow account is constrained by `seeds = [b\"escrow\", maker, seed]`. \
         Substituting an unrelated pubkey for the escrow account must fail the \
         seeds check (ConstraintSeeds) with nothing else touched.",
    );

    let mut ctx = AnchorLiteSVM::build_with_program(escrow::ID, "escrow", PROGRAM_SO);
    let w = setup(&mut ctx, SEED);
    let wrong_escrow = Pubkey::new_unique();
    ctx.alias(wrong_escrow, "WrongEscrow");

    md.step("Action: maker calls make but the escrow account is the wrong PDA");
    let rejection = ctx
        .tx(&[&w.maker])
        .build_with(
            w.bundle,
            escrow::instruction::Make { seed: SEED, receive: RECEIVE, deposit: DEPOSIT },
            |a| a.escrow = wrong_escrow,
        )
        .send_err_named("ConstraintSeeds");
    md.block(
        "rejection logs",
        MarkdownBlock::Fenced { lang: "console".into(), body: rejection.logs_structured_string() },
    );

    md.step("After: nothing moved; the maker still holds the deposit");
    md.snapshot("balances", &balances(&ctx, &w));
    md.check("maker still holds the deposit", Some(DEPOSIT), ctx.svm.token_balance(&w.bundle.maker_ata_a));
}
