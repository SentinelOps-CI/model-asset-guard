# Float obligation (executable refinement)

Model Asset Guard keeps **0 Lean `axiom`s**. Float-facing quantization claims
are honest obligation markers (`Prop := True`), not proofs.

## Wave 28 status: aborted (honest gate)

Wave 28 evaluated replacing
`round_int8_error_bound_float_obligation` /
`layer_error_bound_float_l2_obligation` with a real refinement from
`Quant.Fixed` into an IEEE / Float model. **Aborted.** Markers stay markers.

### Why discharge was refused

1. **Lean 4 `Float` is opaque.** Arithmetic (`+`, `*`, `sqrt`, comparisons,
   `toUInt64`) is external. There is no kernel-level IEEE model to rewrite
   against without either Mathlib-scale infrastructure or new `axiom`s about
   those ops. Fake lemmas over `Float` would be axiom theater.

2. **Mathlib cost vs payoff.** Pulling Mathlib solely for a Float bridge would
   dominate Lean CI (toolchain fetch + compile) while still leaving a gap:
   Mathlib’s IEEE theories do not automatically refine Lean’s opaque `Float`
   without a morphism that is itself axiomatic or incomplete. That fails the
   “non-vacuous, 0 axioms” exit criterion.

3. **Local Rational / Fixed already exists.** Half-ULP and entrywise L1 bounds
   are proven on cleared-denominator `Nat`/`Int` in `Quant.Fixed`. Replacing
   Float markers with those theorems (or with `True`-shaped wrappers) would
   rename theater, not discharge the Float claim.

4. **L2 layer claim needs more than rounding.** `||Ex||₂ ≤ (1/2)||x||₂` on
   opaque `Float` additionally needs a sound sqrt / norm model. No incomplete
   stub was landed.

5. **Sampling soundness stays out of scope.** `verify_128_vectors_sound` and
   `layer_verification_sound` remain markers. Proving Monte Carlo / 128-vector
   coverage via `True` is forbidden.

### What would reopen the gate

A future change may discharge Float markers only if all hold:

- A CI-buildable IEEE (or equivalent) model with **0 new axioms**, or an
  inventory-documented temporary axiom count that `check_axioms.py` accepts
- A **stated** refinement morphism from `Fixed` rounding into that model that
  transports `roundDivInt_error` / `rows_dot_abs_bound` non-vacuously
- Theorems that replace the obligation defs (not `Prop := True` aliases)
- Explicit non-claims for 128-vector / layer verification soundness unless a
  real statistical theorem exists

Until then: markers stay; runtime L2 authority remains Rust
`guardd_verify_quant_128_vectors`.

## Why Float is not proven here

Lean 4 `Float` is an opaque external type. Without a IEEE-754 model (typically
via Mathlib or a custom refinement), statements such as

- `|round_int8(x) - x| ≤ 1/2`
- `||E x||₂ ≤ (1/2) ||x||₂`

cannot be discharged as theorems over `Float`. Inventing fake proofs would be
worse than markers.

## What is proven instead

Cleared-denominator / fixed-point forms in `ModelAssetGuard.Quant.Fixed`:

- `roundDivNat_error` / `roundDivInt_error` — half-ULP on `Nat`/`Int`
- `roundDivNat_error_half_unit` / `roundDivInt_error_half_unit` — aliases for
  the bridge narrative (same theorems; no Mathlib)
- `dot_abs_bound_of_entrywise` / `rows_dot_abs_bound` — Int L1-style bounds

`Core.fixed_round_half_ulp` and `layer_row_l1_bound` alias those results.
Runtime L2 sampling remains Rust (`guardd_verify_quant_128_vectors`).

## Inventory pointers

| Marker | Role |
| --- | --- |
| `round_int8_error_bound_float_obligation` | Former Float half-ULP axiom; Wave 28 left as marker |
| `layer_error_bound_float_l2_obligation` | Former Float L2 layer axiom; Wave 28 left as marker |
| `verify_128_vectors_sound` / `layer_verification_sound` | Runtime soundness not claimed in Lean |

CI `scripts/check_axioms.py` enforces axiom count **0**.
