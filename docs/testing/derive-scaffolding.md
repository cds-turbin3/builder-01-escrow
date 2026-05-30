# Derive scaffolding: how the escrow drives itself in tests

This is a walkthrough of the host-only derive machinery that lets the escrow
integration tests build instructions from a single `EscrowBundle` of pubkeys,
rather than hand-assembling each `accounts::*` / `instruction::*` pair. The
pieces live in four places (the program crate, the test harness, and the
`anchor-litesvm` dependency), so this doc stitches them into one story.

We will examine the three derives stacked on `EscrowBundle` (`BundledPubkeys`,
`AliasMirror`, `Bundle`), what each generates, how a test call flows through
them, and the one caveat worth knowing (token-program injection). 

A note on terminology, to avoid confusion before we start: 
>
>Anchor generates *two* structs per instruction, in two different modules, and we'll lean on both.
>
>1. The `#[derive(Accounts)]` struct (e.g. `Make` in `instructions/make.rs`) is the on-chain *validation* struct: it carries lifetimes, `Signer`, `InterfaceAccount`, and the `#[account(...)]` constraints. 
>
>1. Anchor also emits a client-side `accounts::Make` whose fields are bare `Pubkey`s (the account-metas struct), plus an `instruction::Make` carrying the args. 

When this doc says "the accounts struct" unqualified, it means the
bare-`Pubkey` `accounts::Make`; the validation struct is where the derives are
*written*, but the bare struct is what they target.

## The pieces, and where they live

| Piece | Location | Role |
|---|---|---|
| `EscrowBundle` (10 `Pubkey` fields) | `programs/escrow/src/test_helpers.rs` | The single source of every account address a test needs |
| `#[derive(BundledPubkeys)]` + `#[bundled_with(EscrowBundle)]` | on each `Make`/`Take`/`Refund` in `programs/escrow/src/instructions/*.rs` | Wires the bundle to that instruction |
| `setup()` | `programs/escrow/tests/common/mod.rs` | Populates a real, funded `EscrowBundle` for a scenario |
| The derive macros | `anchor-litesvm-derive` (a dependency) | Emit the glue code described below |


`EscrowBundle` deliberately lives *in the program crate* (`src/test_helpers.rs`),
not in the integration-test directory. The binding reason is *where the derive
runs*.

`#[derive(BundledPubkeys)]` is attached to the `#[derive(Accounts)]` struct, so
its `From` / `BuildableIx` impls expand right there, inside the `escrow` crate.
They name three paths (the bundle, `crate::accounts::Make`, and
`crate::instruction::Make`), and all three have to resolve from that site.

The integration tests are a *separate* crate that depends on `escrow`, so a
bundle defined over in `tests/` would be invisible to the expansion; it has to
live somewhere the program crate can see.

> **N.B.** Coherence isn't the constraint, despite what you might expect: the
> `for` type `accounts::Make` is local, so the orphan rule is satisfied
> trivially. Reachability is what pins the bundle into the crate.

The module is host-only (`#[cfg(not(target_os = "solana"))]` in `lib.rs`, and
the `anchor-litesvm` dep is target-gated in `Cargo.toml`), so none of this
reaches the BPF binary.

### Configuring the three paths

None of those three paths is hard-coded; `#[bundled_with(...)]` parameterises
them (`anchor-litesvm-derive`, `src/parse.rs:83`):

- **The bundle** is the required positional argument. We pass
  `crate::test_helpers::EscrowBundle`, but it can be any path the program crate
  can name: a different module, or a type re-exported from a shared crate that
  both the program and its tests depend on (just not the test crate, per above).
- **The accounts and instruction structs** default to `crate::accounts::<Name>`
  and `crate::instruction::<Name>`, where `<Name>` is the derived struct's own
  identifier (`emit.rs` builds both from `accounts_ident`). Override either with
  an order-independent keyword argument when Anchor puts them somewhere else (a
  renamed instruction, re-exported modules):

  ```rust
  #[bundled_with(
      crate::test_helpers::EscrowBundle,
      instruction = crate::ix::MakeOffer,  // the args type lives here instead
      accounts = crate::accts::Make,       // the metas type lives here instead
  )]
  ```

  An unknown key, or a duplicate `instruction =` / `accounts =`, is a compile
  error with a pointed message. The escrow needs none of this: its structs sit at
  the default Anchor paths, so all three instructions use the bare
  `#[bundled_with(crate::test_helpers::EscrowBundle)]` form.

## Step 1: the cfg gate

Every instruction wears the same hat:

```rust
#[cfg_attr(
    not(target_os = "solana"),
    derive(anchor_litesvm::BundledPubkeys),
    bundled_with(crate::test_helpers::EscrowBundle)
)]
#[derive(Accounts)]
pub struct Make<'info> { /* ... */ }
```

`cfg_attr(not(target_os = "solana"), ...)` applies the inner attributes only in
host builds. So the on-chain build sees a plain `#[derive(Accounts)]` struct, and
the test build additionally gets the `BundledPubkeys` glue. Same source, two
faces.

## Step 2: what `BundledPubkeys` generates

For each struct, the derive emits two impls (see `anchor-litesvm-derive`,
`src/emit.rs:45` and `src/emit.rs:77`):

1. A projection, `From<EscrowBundle> for accounts::Make`. Each field of the bare
   accounts struct is filled either by reading the same-named bundle field
   (`maker: b.maker`) or, for a recognised program type, by a *constant* canonical
   ID (more on that next).
2. A type-level pairing, `BuildableIx<EscrowBundle> for instruction::Make`, with
   `type Accounts = accounts::Make`. This is what lets
   `ctx.tx(..).build(bundle, instruction::Make { .. })` find the right accounts
   struct from the args type alone. Hand a `Make`'s args to a `Refund`'s builder
   and it's a compile error, not a runtime surprise.

The code blocks below are verbatim `cargo expand` output, not paraphrases.
Regenerate any of them with `cargo expand -p escrow <module>` (needs
`cargo-expand` and a nightly toolchain; the host target is implied, so the
`cfg(not(target_os = "solana"))` glue is active):

<details>
<summary><code>cargo expand -p escrow instructions::make</code> &mdash; the two impls <code>BundledPubkeys</code> adds</summary>

```rust
impl ::core::convert::From<crate::test_helpers::EscrowBundle>
for crate::accounts::Make {
    fn from(b: crate::test_helpers::EscrowBundle) -> Self {
        Self {
            maker: b.maker,
            mint_a: b.mint_a,
            mint_b: b.mint_b,
            maker_ata_a: b.maker_ata_a,
            escrow: b.escrow,
            vault: b.vault,
            token_program: anchor_spl::token::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
    }
}
impl ::anchor_litesvm::BuildableIx<crate::test_helpers::EscrowBundle>
for crate::instruction::Make {
    type Accounts = crate::accounts::Make;
}
```

The first six fields read from `b`; the last three are the injected program IDs.
(The `accounts::Make` / `instruction::Make` these target are themselves
Anchor-generated, sitting in the same expansion above this block.)

</details>

### Which fields get injected vs. projected

The derive recognises three Anchor program types by their textual shape and fills
them with a constant ID instead of reading the bundle (`src/parse.rs:239`):

| Field type | Injected ID |
|---|---|
| `Program<'_, System>` | `anchor_lang::solana_program::system_program::ID` |
| `Program<'_, AssociatedToken>` | `anchor_spl::associated_token::ID` |
| `Interface<'_, TokenInterface>` | `anchor_spl::token::ID` |

Everything else projects from the bundle. So `EscrowBundle` carries the accounts
that vary per scenario, and omits the three programs that are always the same.

> **N.B. (the one caveat).** `Interface<TokenInterface>` always injects the
> *classic* SPL Token program ID (`anchor_spl::token::ID`), never Token-2022.
> These tests mint with classic SPL Token (`create_token_mint_at` in
> `common/mod.rs`), so the injected ID is correct. If a future test used a
> Token-2022 mint, the injected `token_program` would be wrong, and you'd
> override it at the call site with `build_with` (the same mechanism
> `refund_rejects_wrong_maker` uses to swap a field):
> `ctx.tx(..).build_with(bundle, ix, |a| a.token_program = spl_token_2022::ID)`.

## Step 3: the other two derives

`EscrowBundle` stacks two more derives, each with one job:

```rust
#[derive(Copy, Clone, Debug, AliasMirror, Bundle)]
pub struct EscrowBundle { /* ten Pubkey fields */ }
```

- **`AliasMirror`** emits `alias_all(&self, ctx)`, which registers every field
  under a PascalCase label derived from its name (`maker_ata_a` -> `MakerAtaA`),
  so the structured-log printer can show human names instead of base58.

  Caveat: the escrow suite no longer calls it. `setup()` (in `common/mod.rs`)
  moved to *canonical* aliasing: it names the leaves (`Maker`, `Taker`, `A`, `B`,
  `Escrow`) and composes the token-account labels from them with `alias_ata`, so
  the trace reads `Maker/A`, `Escrow/A` (the vault), and so on, rather than the
  flat `MakerAtaA`. `alias_all` is still generated (that's the derive's whole
  job), just currently unused; the expansion below is exactly what
  `w.bundle.alias_all(&mut ctx)` would register if you called it.

- **`Bundle`** emits `Default`, filling every field with a fresh
  `Pubkey::new_unique()` (`src/emit.rs`, the `emit_bundle_default` path). That's
  the ergonomic the next section turns on: `..EscrowBundle::default()` lets a test
  pin only the fields it cares about and leave the rest as throwaway placeholders.

Both expansions, verbatim from `cargo expand -p escrow test_helpers`:

<details>
<summary><code>AliasMirror</code> &mdash; the generated <code>alias_all</code></summary>

```rust
impl EscrowBundle {
    /// Register every `Pubkey` field with a friendly label in
    /// the context's alias table. Returns the context for
    /// chaining. Generated by `#[derive(AliasMirror)]`.
    pub fn alias_all<'__a>(
        &self,
        ctx: &'__a mut ::anchor_litesvm::AnchorContext,
    ) -> &'__a mut ::anchor_litesvm::AnchorContext {
        ctx.alias(self.maker, "Maker")
            .alias(self.taker, "Taker")
            .alias(self.mint_a, "MintA")
            .alias(self.mint_b, "MintB")
            .alias(self.maker_ata_a, "MakerAtaA")
            .alias(self.maker_ata_b, "MakerAtaB")
            .alias(self.taker_ata_a, "TakerAtaA")
            .alias(self.taker_ata_b, "TakerAtaB")
            .alias(self.escrow, "Escrow")
            .alias(self.vault, "Vault")
    }
}
```

</details>

<details>
<summary><code>Bundle</code> &mdash; the generated <code>Default</code></summary>

```rust
impl ::core::default::Default for EscrowBundle {
    fn default() -> Self {
        Self {
            maker: ::anchor_lang::prelude::Pubkey::new_unique(),
            taker: ::anchor_lang::prelude::Pubkey::new_unique(),
            mint_a: ::anchor_lang::prelude::Pubkey::new_unique(),
            mint_b: ::anchor_lang::prelude::Pubkey::new_unique(),
            maker_ata_a: ::anchor_lang::prelude::Pubkey::new_unique(),
            maker_ata_b: ::anchor_lang::prelude::Pubkey::new_unique(),
            taker_ata_a: ::anchor_lang::prelude::Pubkey::new_unique(),
            taker_ata_b: ::anchor_lang::prelude::Pubkey::new_unique(),
            escrow: ::anchor_lang::prelude::Pubkey::new_unique(),
            vault: ::anchor_lang::prelude::Pubkey::new_unique(),
        }
    }
}
```

</details>

## How a test call flows through all of it

A single line in a test:

```rust
ctx.tx(&[&w.maker])
    .build(w.bundle, escrow::instruction::Refund {})
    .send_ok();
```

stitches the pieces together like this:

1. `build(bundle, instruction::Refund {})` reads `BuildableIx<EscrowBundle> for
   instruction::Refund` to learn the paired accounts type is `accounts::Refund`.
2. It runs the `From<EscrowBundle> for accounts::Refund` projection: five fields
   read from the bundle, two program IDs injected.
3. The resulting account-metas plus the `Refund {}` args become an `Instruction`,
   which `send_ok()` submits and asserts succeeds.

The aliases registered in `setup()` (the canonical `Maker/A`, `Escrow/A`, ...
labels) then stand in for base58 when the report is rendered.

## Worked example: projecting only a subset

Here's where the shared bundle earns its keep. The ten-field `EscrowBundle` is
shared across all three instructions, but each `From<EscrowBundle>` reads only the
fields *its* accounts struct names:

| Bundle field | Make | Take | Refund |
|---|:---:|:---:|:---:|
| `maker` | proj | proj | proj |
| `taker` | | proj | |
| `mint_a` | proj | proj | proj |
| `mint_b` | proj | proj | |
| `maker_ata_a` | proj | | proj |
| `maker_ata_b` | | proj | |
| `taker_ata_a` | | proj | |
| `taker_ata_b` | | proj | |
| `escrow` | proj | proj | proj |
| `vault` | proj | proj | proj |
| *(injected)* `system_program` | const | const | const |
| *(injected)* `associated_token_program` | const | const | — |
| *(injected)* `token_program` | const | const | const |

`refund` is the narrowest column: it projects only
`maker`/`mint_a`/`maker_ata_a`/`escrow`/`vault`. The expansion makes that
concrete: its `from(b)` body never mentions `b.taker`, `b.mint_b`, or any of the
`*_ata_*` fields, and it injects only two programs (no
`associated_token_program`, which `Refund` doesn't declare).

<details>
<summary><code>cargo expand -p escrow instructions::refund</code> &mdash; the narrowest projection</summary>

```rust
impl ::core::convert::From<crate::test_helpers::EscrowBundle>
for crate::accounts::Refund {
    fn from(b: crate::test_helpers::EscrowBundle) -> Self {
        Self {
            maker: b.maker,
            mint_a: b.mint_a,
            maker_ata_a: b.maker_ata_a,
            escrow: b.escrow,
            vault: b.vault,
            token_program: anchor_spl::token::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
    }
}
impl ::anchor_litesvm::BuildableIx<crate::test_helpers::EscrowBundle>
for crate::instruction::Refund {
    type Accounts = crate::accounts::Refund;
}
```

</details>

So a refund call can pin exactly those five and let `Bundle`'s `Default` fill the
other five with placeholders that are never read. That's exactly what
`refund_projects_only_its_bundle_subset` (in `tests/test_refund.rs`) asserts:

```rust
let refund_bundle = EscrowBundle {
    maker: w.bundle.maker,
    mint_a: w.bundle.mint_a,
    maker_ata_a: w.bundle.maker_ata_a,
    escrow: w.bundle.escrow,
    vault: w.bundle.vault,
    ..EscrowBundle::default()  // taker, mint_b, maker_ata_b, taker_ata_a, taker_ata_b
};
ctx.tx(&[&w.maker]).build(refund_bundle, escrow::instruction::Refund {}).send_ok();
```

The refund settles identically to the full-bundle path: the placeholder fields
genuinely never reach `accounts::Refund`, and the test proves it by checking the
maker recovers the deposit and the vault + escrow close.

## Escape hatches

The escrow needs none of these (its ten fields are all bare `Pubkey` and project
one-to-one), but they're the tools for when the simple projection isn't enough, so
here's what each does and what it expands to.

### `#[bundle(unwrap)]` and `#[bundle(wrap_some)]`: when source and target differ in type

The plain projection is `field: b.field`, which only type-checks when the bundle
field (the *source*) and the accounts field it feeds (the *target*) are the same
type: `Pubkey -> Pubkey`. These two attributes are the hatch for when they differ.

The difference is always an `Option` wrapper (the accounts metas are bare `Pubkey`
or `Option<Pubkey>`, nothing more exotic), so there are exactly two directions to
bridge:

| On the field | Source (bundle) | Target (accounts) | Generated projection |
|---|---|---|---|
| *(nothing)* | `Pubkey` | `Pubkey` | `field: b.field` |
| `#[bundle(unwrap)]` | `Option<Pubkey>` | `Pubkey` | `field: Option::expect(b.field, "...")` |
| `#[bundle(wrap_some)]` | `Pubkey` | `Option<Pubkey>` | `field: Option::Some(b.field)` |

They earn their keep when *one* bundle has to feed accounts structs that disagree
on a field's optionality. Say two instructions share a bundle, but one needs a
`payment_mint` and the other treats it as optional. You carry it in the bundle as
`Option<Pubkey>` and let the attribute reconcile the shape per instruction.

`#[bundle(unwrap)]`, on an accounts field that is a bare `T` while the bundle
field is `Option<T>`:

```rust
#[bundled_with(crate::test_helpers::PayBundle)]   // PayBundle { payment_mint: Option<Pubkey>, .. }
#[derive(Accounts)]
pub struct Buy<'info> {
    #[bundle(unwrap)]
    pub payment_mint: InterfaceAccount<'info, Mint>,  // required here
    // ...
}
```

projects with `Option::expect`, so a `None` fails loudly at build time rather than
sending a garbage account (the message names the field and the struct):

```rust
payment_mint: ::core::option::Option::expect(
    b.payment_mint,
    "bundle field `payment_mint` is None, but accounts::Buy.payment_mint requires \
     Some(_); set the bundle field before building this instruction",
),
```

`#[bundle(wrap_some)]` is the mirror image: the bundle field is a bare `T`, the
accounts field is `Option<T>`, and the projection wraps it:

```rust
pub payment_mint: Option<InterfaceAccount<'info, Mint>>,  // with #[bundle(wrap_some)]
// expands to:  payment_mint: ::core::option::Option::Some(b.payment_mint),
```

> **N.B.** These win over the well-known-program classification: put
> `#[bundle(unwrap)]` on a `Program<System>` field and it projects from the bundle
> instead of injecting the system ID. That's almost never what you want, but the
> derive honours the explicit attribute rather than silently ignoring it
> (`src/parse.rs:550` pins this behaviour).

### `build_with`: a per-call override

When you want the normal projection but with one field tweaked for a single
instruction, override at the call site instead of in the bundle.
`build_with(bundle, ix, |a| ...)` runs the `From` projection, then hands you the
resulting accounts struct to mutate before the `Instruction` is assembled. The
escrow's own negative test uses it to point `maker` at the wrong account:

```rust
ctx.tx(&[&w.maker])
    .build_with(w.bundle, escrow::instruction::Refund {}, |a| a.maker = wrong_maker)
    .send_err_named("ConstraintTokenOwner");
```

This is also the hatch for the Token-2022 caveat above: keep the bundle as-is and
correct the injected program in the closure,
`|a| a.token_program = spl_token_2022::ID`.
