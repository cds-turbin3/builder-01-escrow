//! Shared test scaffolding for the escrow integration tests.
//!
//! Holds the `EscrowBundle` pubkey bundle, the `BuildableIx` glue that pairs
//! each escrow instruction with its accounts struct, the scenario constants,
//! and the `setup` fixture that builds a ready-to-use escrow scenario.

// This module is compiled into all three test binaries, and not every binary
// reads every bundle field (e.g. test_refund never touches the taker ATAs).
// Silence the resulting per-binary dead-code noise for this scaffolding module.
#![allow(dead_code)]

use anchor_lang::prelude::Pubkey;
use anchor_litesvm::BuildableIx;

/// One escrow scenario's worth of pubkeys: the "address book" threaded through
/// `make`, `take`, and `refund`.
#[derive(Copy, Clone)]
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
    pub token_program: Pubkey,
    pub associated_token_program: Pubkey,
    pub system_program: Pubkey,
}

impl From<EscrowBundle> for escrow::accounts::Make {
    fn from(b: EscrowBundle) -> Self {
        Self {
            maker: b.maker,
            mint_a: b.mint_a,
            mint_b: b.mint_b,
            maker_ata_a: b.maker_ata_a,
            escrow: b.escrow,
            vault: b.vault,
            token_program: b.token_program,
            associated_token_program: b.associated_token_program,
            system_program: b.system_program,
        }
    }
}

impl From<EscrowBundle> for escrow::accounts::Take {
    fn from(b: EscrowBundle) -> Self {
        Self {
            taker: b.taker,
            maker: b.maker,
            mint_a: b.mint_a,
            mint_b: b.mint_b,
            taker_ata_a: b.taker_ata_a,
            taker_ata_b: b.taker_ata_b,
            maker_ata_b: b.maker_ata_b,
            escrow: b.escrow,
            vault: b.vault,
            token_program: b.token_program,
            associated_token_program: b.associated_token_program,
            system_program: b.system_program,
        }
    }
}

impl From<EscrowBundle> for escrow::accounts::Refund {
    fn from(b: EscrowBundle) -> Self {
        Self {
            maker: b.maker,
            mint_a: b.mint_a,
            maker_ata_a: b.maker_ata_a,
            escrow: b.escrow,
            vault: b.vault,
            token_program: b.token_program,
            system_program: b.system_program,
        }
    }
}

impl BuildableIx<EscrowBundle> for escrow::instruction::Make {
    type Accounts = escrow::accounts::Make;
}

impl BuildableIx<EscrowBundle> for escrow::instruction::Take {
    type Accounts = escrow::accounts::Take;
}

impl BuildableIx<EscrowBundle> for escrow::instruction::Refund {
    type Accounts = escrow::accounts::Refund;
}
