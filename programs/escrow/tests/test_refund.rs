//! Integration tests for the escrow `refund` instruction.
//!
//! `refund` is the maker's recovery path once no taker has shown up: it returns
//! the vault's mint_a to the maker and closes the vault + escrow. It is gated
//! the *opposite* way from `take`: refund is only allowed once the escrow has
//! expired, so a maker can't strand a would-be taker mid-flight. These tests use
//! the time-warp feature (`advance_days`) to sit on both sides of expiry.
//!
//! Each test threads a [`Report`]; the Markdown lands in
//! `target/md-reports/<slug>.md`.

mod common;

use anchor_litesvm::{AnchorLiteSVM, MarkdownBlock, Pubkey, Report, TestHelpers};
use common::{balances, setup, EscrowBundle, DEPOSIT, RECEIVE, SEED};

const PROGRAM_SO: &[u8] = include_bytes!("../../../target/deploy/escrow.so");

/// Happy path: once expired, `refund` returns the deposit to the maker and
/// closes both the vault and the escrow.
#[test]
fn refund_returns_deposit_and_closes_escrow() {
    let mut md = Report::new(
        "Escrow: refund returns the deposit after expiry and closes the escrow",
        "When no taker shows up before the 90-day window closes, the maker \
         recovers: refund (allowed only once expired) returns the vault's full \
         mint_a to the maker and closes the vault + escrow (rent back to the \
         maker).",
    );

    let mut ctx = AnchorLiteSVM::build_with_program(escrow::ID, "escrow", PROGRAM_SO);
    let w = setup(&mut ctx, SEED);

    md.step("Setup: maker opens the escrow (deposit now in the vault)");
    ctx.tx(&[&w.maker])
        .build(
            w.bundle,
            escrow::instruction::Make { seed: SEED, receive: RECEIVE, deposit: DEPOSIT },
        )
        .send_ok()
        .print_markdown_pair();
    md.snapshot("after make", &balances(&ctx, &w));

    md.step("Advance 199 days (past the 90-day window, so refund is allowed)");
    ctx.svm.advance_days(199);

    md.step("Action: maker calls refund");
    md.note("refund declares no Signer; the maker signs only as the transaction fee payer.");
    ctx.tx(&[&w.maker])
        .build(w.bundle, escrow::instruction::Refund {})
        .send_ok()
        .print_markdown_pair();

    md.step("After: deposit back with the maker; vault + escrow closed");
    md.snapshot("after refund", &balances(&ctx, &w));
    md.check("maker recovered the deposit", Some(DEPOSIT), ctx.svm.token_balance(&w.bundle.maker_ata_a));
    md.check("vault account closed", false, ctx.account_exists(&w.bundle.vault));
    md.check("escrow account closed", false, ctx.account_exists(&w.bundle.escrow));
}

/// Negative (time-based): a maker cannot pull funds back early. While the escrow
/// is still live, `refund` must be rejected with `EscrowNotExpired`, so a maker
/// can't strand a would-be taker.
#[test]
fn refund_fails_before_expiry() {
    let mut md = Report::new(
        "Escrow: refund is rejected before expiry",
        "refund is the mirror of take's gate: allowed only after the window \
         closes. Inside the window it must fail with EscrowNotExpired, so a maker \
         cannot yank the deposit out from under a taker who is mid-flight.",
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

    md.step("Advance only 19 days (comfortably inside the 90-day window)");
    ctx.svm.advance_days(19);

    md.step("Action: maker calls refund while still live → must fail");
    let rejection = ctx
        .tx(&[&w.maker])
        .build(w.bundle, escrow::instruction::Refund {})
        .send_err_named("EscrowNotExpired");
    md.block(
        "rejection logs",
        MarkdownBlock::Fenced { lang: "console".into(), body: rejection.logs_structured_string() },
    );

    md.step("After: the deposit is still escrowed");
    md.snapshot("balances", &balances(&ctx, &w));
    md.check("vault still holds the deposit", Some(DEPOSIT), ctx.svm.token_balance(&w.bundle.vault));
    md.check("escrow account still open", true, ctx.account_exists(&w.bundle.escrow));
}

/// Negative: with a valid, expired escrow, a wrong `maker` must be rejected.
/// We swap a fresh pubkey in for `maker` while still signing with the real maker
/// (so the fee-payer signature passes); the failure comes from
/// `ConstraintTokenOwner`, fired when Anchor checks that `maker_ata_a`'s owner
/// matches the `maker` account passed in.
#[test]
fn refund_rejects_wrong_maker() {
    let mut md = Report::new(
        "Escrow: refund rejects a wrong maker account",
        "Even past expiry, refund must pay the *right* maker. Swapping an \
         unrelated pubkey into the `maker` slot (while the real maker still signs \
         as fee payer) fails ConstraintTokenOwner: maker_ata_a's owner no longer \
         matches the passed maker.",
    );

    let mut ctx = AnchorLiteSVM::build_with_program(escrow::ID, "escrow", PROGRAM_SO);
    let w = setup(&mut ctx, SEED);

    md.step("Setup: maker opens the escrow, then the window expires");
    ctx.tx(&[&w.maker])
        .build(
            w.bundle,
            escrow::instruction::Make { seed: SEED, receive: RECEIVE, deposit: DEPOSIT },
        )
        .send_ok()
        .print_markdown_pair();
    ctx.svm.advance_days(199);

    let wrong_maker = Pubkey::new_unique();
    ctx.alias(wrong_maker, "WrongMaker");

    md.step("Action: refund with the maker slot pointed at an unrelated pubkey → must fail");
    let rejection = ctx
        .tx(&[&w.maker])
        .build_with(w.bundle, escrow::instruction::Refund {}, |a| a.maker = wrong_maker)
        .send_err_named("ConstraintTokenOwner");
    md.block(
        "rejection logs",
        MarkdownBlock::Fenced { lang: "console".into(), body: rejection.logs_structured_string() },
    );

    md.step("After: the real escrow and vault are intact");
    md.check("vault still holds the deposit", Some(DEPOSIT), ctx.svm.token_balance(&w.bundle.vault));
    md.check("escrow account still open", true, ctx.account_exists(&w.bundle.escrow));
}

/// `EscrowBundle::default()` (from `#[derive(Bundle)]`) seeds every field with a
/// fresh `Pubkey::new_unique()`. Because the shared ten-field bundle is projected
/// per instruction (each generated `From<EscrowBundle>` reads only the fields
/// *its* accounts struct names), a test can pin just that subset and let the rest
/// fall to placeholders. `refund` is the narrowest case: it projects only
/// `maker`/`mint_a`/`maker_ata_a`/`escrow`/`vault`, so the five taker-side fields
/// can stay throwaway and the refund still settles. See
/// `docs/testing/derive-scaffolding.md` for the full picture.
#[test]
fn refund_projects_only_its_bundle_subset() {
    let mut md = Report::new(
        "Escrow: refund reads only its slice of the shared bundle",
        "The ten-field EscrowBundle is shared across make/take/refund, but each \
         instruction's generated `From<EscrowBundle>` projects only the fields it \
         names. refund touches just maker/mint_a/maker_ata_a/escrow/vault, so a \
         bundle that pins those and leaves the five taker-side fields as \
         `..EscrowBundle::default()` placeholders still settles the refund: the \
         ergonomic that `#[derive(Bundle)]`'s Default buys.",
    );

    let mut ctx = AnchorLiteSVM::build_with_program(escrow::ID, "escrow", PROGRAM_SO);
    let w = setup(&mut ctx, SEED);

    md.step("Setup: maker opens the escrow with the full bundle, then it expires");
    ctx.tx(&[&w.maker])
        .build(
            w.bundle,
            escrow::instruction::Make { seed: SEED, receive: RECEIVE, deposit: DEPOSIT },
        )
        .send_ok()
        .print_markdown_pair();
    ctx.svm.advance_days(199);

    // Pin only the fields refund projects; the taker-side five (taker, mint_b,
    // maker_ata_b, taker_ata_a, taker_ata_b) fall to `Pubkey::new_unique()` and
    // are never read by `From<EscrowBundle> for accounts::Refund`.
    let refund_bundle = EscrowBundle {
        maker: w.bundle.maker,
        mint_a: w.bundle.mint_a,
        maker_ata_a: w.bundle.maker_ata_a,
        escrow: w.bundle.escrow,
        vault: w.bundle.vault,
        ..EscrowBundle::default()
    };

    md.step("Action: refund with a bundle whose taker-side fields are placeholders");
    md.note(
        "Only maker/mint_a/maker_ata_a/escrow/vault are pinned; the other five are \
         fresh `Pubkey::new_unique()` values from Bundle's Default, and refund never \
         looks at them.",
    );
    ctx.tx(&[&w.maker])
        .build(refund_bundle, escrow::instruction::Refund {})
        .send_ok()
        .print_markdown_pair();

    md.step("After: refund settled exactly as it would with the full bundle");
    md.snapshot("after refund", &balances(&ctx, &w));
    md.check("maker recovered the deposit", Some(DEPOSIT), ctx.svm.token_balance(&w.bundle.maker_ata_a));
    md.check("vault account closed", false, ctx.account_exists(&w.bundle.vault));
    md.check("escrow account closed", false, ctx.account_exists(&w.bundle.escrow));
}
