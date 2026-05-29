//! Bundle + alias registration for escrow integration tests.
//! Host-only; never reaches the BPF binary.
//!
//! `AliasMirror` emits `Self::alias_all(&self, ctx)` that registers every
//! `Pubkey` field with a PascalCase label derived from the field name. So
//! `bundle.alias_all(&mut ctx)` once per test seeds the alias table that
//! the structured-log printer reads.

use anchor_lang::prelude::Pubkey;
use anchor_litesvm::AliasMirror;

#[derive(Copy, Clone, Debug, AliasMirror)]
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
