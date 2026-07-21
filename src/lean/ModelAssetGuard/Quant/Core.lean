import Init.Data.Array.Basic
import Init.Data.List.Basic
import ModelAssetGuard.Quant.Fixed

namespace ModelAssetGuard.Quant

/-- Rounding function for int8 quantization -/
def round_int8 (x : Float) : Int :=
  if x ≥ 0.0 then
    Int.ofNat (Float.toUInt64 (x + 0.5) |>.toNat)
  else
    Int.negOfNat (Float.toUInt64 ((0.0 - x) + 0.5) |>.toNat)

/-- Convert back to float -/
def int8_to_float (i : Int) : Float :=
  match i with
  | Int.ofNat n => n.toFloat
  | Int.negSucc n => 0.0 - (n.succ.toFloat)

/-- Quantization error -/
def quant_error (x : Float) : Float :=
  int8_to_float (round_int8 x) - x

/-- ULP (Unit in Last Place) for int8 -/
def ulp_int8 : Float := 1.0

/-- Float half-ULP claim for `round_int8` is **not** an axiom (Wave 6).

Lean 4 `Float` ops are opaque externals; a real proof is not available without
a Float model. Honest status: unproven obligation marker (same pattern as
`layer_error_bound_float_l2_obligation`). Proven analogue on cleared-denominator
rationals is `fixed_round_half_ulp` / `Fixed.roundDivInt_error`.
Wave 28 aborted Mathlib/IEEE discharge; see `docs/float-obligation.md`.
Runtime L2 checks live in Rust `guardd_verify_quant_128_vectors`. -/
def round_int8_error_bound_float_obligation (_x : Float) : Prop :=
  True

/-- Proven half-ULP bound on fixed-point rationals (see `Fixed`). -/
theorem fixed_round_half_ulp (n : Int) (d : Nat) (hd : d > 0) :
    2 * Int.natAbs (n - Fixed.roundDivInt n d hd * (d : Int)) ≤ d :=
  Fixed.roundDivInt_error n d hd

/-- Layer weight matrix type -/
abbrev WeightMatrix := Array (Array Float)

/-- Quantized weight matrix -/
abbrev QuantizedMatrix := Array (Array Int)

/-- Quantize a weight matrix -/
def quantize_matrix (W : WeightMatrix) : QuantizedMatrix :=
  W.map (fun row => row.map round_int8)

/-- Dequantize a matrix -/
def dequantize_matrix (Wq : QuantizedMatrix) : WeightMatrix :=
  Wq.map (fun row => row.map int8_to_float)

/-- Elementwise matrix subtraction `A - B` (row-major, truncated to shared shape). -/
def matrix_sub (A B : WeightMatrix) : WeightMatrix :=
  A.zip B |>.map fun (rowA, rowB) =>
    rowA.zip rowB |>.map fun (a, b) => a - b

/-- Quantization error matrix: `W_q - W` after int8 quantize/dequantize.
Previously (F015) this incorrectly returned dequantized weights alone. -/
def quantization_error_matrix (W : WeightMatrix) : WeightMatrix :=
  let Wq := dequantize_matrix (quantize_matrix W)
  matrix_sub Wq W

/-- L2 norm of a vector -/
def l2_norm (x : Array Float) : Float :=
  Float.sqrt (x.foldl (fun acc v => acc + (v * v)) 0.0)

/-- Matrix-vector multiplication -/
def matvec_mul (W : WeightMatrix) (x : Array Float) : Array Float :=
  W.map fun row =>
    let pairs := row.zip x
    pairs.foldl (fun acc (a, b) => acc + (a * b)) 0.0

/-- Float L2 claim `||Ex||₂ ≤ (1/2)||x||₂` is **not** an axiom (Wave 5).

Honest status: unproven on opaque `Float`. Proven analogue under cleared-denominator
Int hypotheses is `layer_row_l1_bound` / `Fixed.rows_dot_abs_bound`. Runtime
authority for the L2 metric remains Rust `guardd_verify_quant_128_vectors`. -/
def layer_error_bound_float_l2_obligation (_W : WeightMatrix) (_x : Array Float) : Prop :=
  True

/-- Proven entrywise Int bound: if `2*|eⱼ| ≤ scale` then `2*|e·x| ≤ scale * ||x||₁`. -/
theorem layer_row_l1_bound
    (es xs : List Int) (scale : Nat) (hs : scale > 0)
    (hlen : es.length = xs.length)
    (hbound : ∀ e ∈ es, 2 * Int.natAbs e ≤ scale) :
    2 * Int.natAbs (Fixed.dot es xs) ≤ scale * Fixed.l1Norm xs :=
  Fixed.dot_abs_bound_of_entrywise es xs scale hs hlen hbound

/-- Row-wise Int layer bound: every row satisfying the entrywise scale is L1-bounded. -/
theorem layer_rows_l1_bound
    (rows : List (List Int)) (xs : List Int) (scale : Nat) (hs : scale > 0)
    (hrows : ∀ es ∈ rows,
        es.length = xs.length ∧ ∀ e ∈ es, 2 * Int.natAbs e ≤ scale) :
    ∀ es ∈ rows, 2 * Int.natAbs (Fixed.dot es xs) ≤ scale * Fixed.l1Norm xs :=
  Fixed.rows_dot_abs_bound rows xs scale hs hrows

/-- Vacuous placeholder retained for API stability; not a contentful bound. -/
theorem per_channel_error_bound (_W : WeightMatrix) (_x : Array Float) : True :=
  trivial

end ModelAssetGuard.Quant
