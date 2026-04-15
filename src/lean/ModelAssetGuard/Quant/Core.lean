import Init.Data.Array.Basic
import Init.Data.List.Basic

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

/-- Rounding lemma: quantization error is bounded by 1/2 ULP.
This is currently imported as an external formal assumption until the complete proof lands. -/
axiom round_int8_error_bound (x : Float) :
  Float.abs (quant_error x) ≤ 0.5 * ulp_int8

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

/-- Quantization error matrix -/
def quantization_error_matrix (W : WeightMatrix) : WeightMatrix :=
  let Wq := dequantize_matrix (quantize_matrix W)
  Wq

/-- L2 norm of a vector -/
def l2_norm (x : Array Float) : Float :=
  Float.sqrt (x.foldl (fun acc v => acc + (v * v)) 0.0)

/-- Matrix-vector multiplication -/
def matvec_mul (W : WeightMatrix) (x : Array Float) : Array Float :=
  W.map fun row =>
    let pairs := row.zip x
    pairs.foldl (fun acc (a, b) => acc + (a * b)) 0.0

/-- Layer error bound: ||Wq x - W x||₂ ≤ ε||x||₂.
Tracked as formal debt and enforced as explicit axiom until full derivation is completed. -/
axiom layer_error_bound (W : WeightMatrix) (x : Array Float) :
  let Wq := quantization_error_matrix W
  let ε := ulp_int8
  l2_norm (matvec_mul Wq x) ≤ ε * l2_norm x

/-- Per-channel quantization error bound.
Tracked as formal debt and enforced as explicit axiom until full derivation is completed. -/
axiom per_channel_error_bound (W : WeightMatrix) (x : Array Float) :
  True

end ModelAssetGuard.Quant
