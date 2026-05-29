//! Shared test scaffolding for the escrow integration tests.
//!
//! The `EscrowBundle` and its `BundledPubkeys` / `AliasMirror` derives
//! live in `escrow::test_helpers` (the program crate, so the per-ix
//! `BundledPubkeys` impls on the Accounts structs satisfy the orphan
//! rule). This module re-exports the bundle so test files keep
//! importing from `common::EscrowBundle`. What's actually here:
//! scenario constants and the `setup` fixture that builds a ready-to-
//! use escrow scenario (funded actors, two mints with distinct
//! decimals, source ATAs, the escrow PDA, and every derived account).

// NOTE (compilation dimension)
// This module is compiled into all three test binaries, and not every binary
// reads every bundle field (e.g. test_refund never touches the taker ATAs).
// Silence the resulting per-binary dead-code noise for this scaffolding module.
#![allow(dead_code)]

use anchor_litesvm::{AnchorContext, Keypair, Signer, TestHelpers};
use spl_associated_token_account::get_associated_token_address;

// Re-exported so tests keep importing from `common::EscrowBundle`.
pub use escrow::test_helpers::EscrowBundle;

/// Scenario constants. `DEPOSIT != RECEIVE` and the two mints differ in
/// decimals so the `take` bug (which uses `escrow.receive` and `mint_b.decimals`
/// where it should use `vault.amount` and `mint_a.decimals`) cannot stay hidden.
pub const SEED: u64 = 42;
pub const MINT_A_DECIMALS: u8 = 6;
pub const MINT_B_DECIMALS: u8 = 9;
pub const DEPOSIT: u64 = 1_000_000;
pub const RECEIVE: u64 = 2_000_000_000;

/// Build a ready-to-use escrow scenario: funded maker/taker, two mints with
/// distinct decimals, the maker's and taker's funded source ATAs, and every
/// derived address (escrow PDA, vault, and the two `init_if_needed` ATAs).
///
/// Panics on any infrastructure failure; a panic here is a broken fixture, not
/// a failure of the code under test.
pub fn setup(ctx: &mut AnchorContext, seed: u64) -> (EscrowBundle, Keypair, Keypair) {
    let maker = ctx
        .svm
        .create_funded_account(10_000_000_000)
        .expect("fund maker");
    let taker = ctx
        .svm
        .create_funded_account(10_000_000_000)
        .expect("fund taker");

    let mint_a = ctx
        .svm
        .create_token_mint(&maker, MINT_A_DECIMALS)
        .expect("create mint_a");
    let mint_b = ctx
        .svm
        .create_token_mint(&taker, MINT_B_DECIMALS)
        .expect("create mint_b");

    let maker_ata_a = ctx
        .svm
        .create_associated_token_account(&mint_a.pubkey(), &maker)
        .expect("create maker_ata_a");
    ctx.svm
        .mint_to(&mint_a.pubkey(), &maker_ata_a, &maker, DEPOSIT)
        .expect("fund maker_ata_a");

    let taker_ata_b = ctx
        .svm
        .create_associated_token_account(&mint_b.pubkey(), &taker)
        .expect("create taker_ata_b");
    ctx.svm
        .mint_to(&mint_b.pubkey(), &taker_ata_b, &taker, RECEIVE)
        .expect("fund taker_ata_b");

    let maker_key = maker.pubkey();
    let taker_key = taker.pubkey();

    let escrow = ctx.svm.get_pda(
        &[escrow::ESCROW_SEED, maker_key.as_ref(), &seed.to_le_bytes()],
        &escrow::ID,
    );
    let vault = get_associated_token_address(&escrow, &mint_a.pubkey());
    let taker_ata_a = get_associated_token_address(&taker_key, &mint_a.pubkey());
    let maker_ata_b = get_associated_token_address(&maker_key, &mint_b.pubkey());

    let bundle = EscrowBundle {
        maker: maker_key,
        taker: taker_key,
        mint_a: mint_a.pubkey(),
        mint_b: mint_b.pubkey(),
        maker_ata_a,
        maker_ata_b,
        taker_ata_a,
        taker_ata_b,
        escrow,
        vault,
    };

    (bundle, maker, taker)
}

