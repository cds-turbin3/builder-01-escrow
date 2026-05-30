//! Bundle + alias registration for escrow integration tests.
//! Host-only; never reaches the BPF binary.
//!
//! Three derives stack here, each carrying one job; how they stitch together
//! with the per-instruction `BundledPubkeys` projections is written up in
//! `docs/testing/derive-scaffolding.md`. In short:
//!
//! - `AliasMirror` emits `Self::alias_all(&self, ctx)`, registering every
//!   `Pubkey` field under a PascalCase label (`MakerAtaA`). The suite aliases
//!   canonically in `setup()` (`Maker/A`, `Escrow/A`, ... via `alias_ata`)
//!   instead, so `alias_all` is generated but unused here.
//! - `Bundle` emits a `Default` that seeds every field with a fresh
//!   `Pubkey::new_unique()`, so `..EscrowBundle::default()` lets a test pin only
//!   the fields a given instruction actually projects and leave the rest as
//!   throwaway placeholders.

use anchor_lang::prelude::Pubkey;
use anchor_litesvm::{AliasMirror, Bundle};

#[derive(Copy, Clone, Debug, AliasMirror, Bundle)]
pub struct EscrowBundle {
    pub maker: Pubkey,
    pub taker: Pubkey,
    pub mint_a: Pubkey,
    pub mint_b: Pubkey,
    pub maker_ata_a: Pubkey,
    pub maker_ata_b: Pubkey,
    pub taker_ata_a: Pubkey,
    pub taker_ata_b: Pubkey,
    pub escrow: Pubkey,
    pub vault: Pubkey,
}
