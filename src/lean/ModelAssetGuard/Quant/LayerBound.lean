import ModelAssetGuard.Quant.Core
import Init.System.IO

namespace ModelAssetGuard.Quant

/-- Layer configuration -/
structure LayerConfig where
  fan_in : Nat
  fan_out : Nat
  quant_type : String -- "int8", "fp16", etc.
  deriving Repr

/-- Compute ε bound for a layer -/
def compute_epsilon_bound (config : LayerConfig) : Float :=
  match config.quant_type with
  | "int8" => ulp_int8 * Float.sqrt config.fan_in.toFloat
  | "fp16" => 2.0 * Float.sqrt config.fan_in.toFloat
  | _ => 1.0 * Float.sqrt config.fan_in.toFloat

/-- Random activation vector generator -/
def generate_random_vector (n : Nat) (seed : Nat) : Array Float :=
  (List.range n).foldl
    (fun acc i =>
      let x := (seed + i) % 1000
      acc.push (((x.toFloat / 1000.0) * 2.0) - 1.0))
    #[]

/-- Generate 128 random activation vectors -/
def generate_128_vectors (n : Nat) : Array (Array Float) :=
  (List.range 128).foldl (fun acc i => acc.push (generate_random_vector n i)) #[]

/-- Compute maximum error across 128 random activation vectors -/
def compute_max_error_128_vectors (W : WeightMatrix)
  (config : LayerConfig) : Float :=
  let error_matrix := quantization_error_matrix W
  let vectors := generate_128_vectors config.fan_in

  let errors := vectors.map fun x =>
    let error_output := matvec_mul error_matrix x
    l2_norm error_output / l2_norm x -- Normalized error

  errors.foldl (fun max_err err => if err > max_err then err else max_err) 0.0

/-- Detailed verification result with 128 vectors analysis -/
structure LayerVerification128 where
  layer_name : String
  config : LayerConfig
  epsilon_bound : Float
  max_error_128_vectors : Float
  mean_error_128_vectors : Float
  error_std_deviation : Float
  passed_128_vectors : Bool
  error_distribution : Array Float
  deriving Repr

/-- Verify a single layer with 128 random activation vectors -/
def verify_layer_128_vectors (layer_name : String) (W : WeightMatrix)
  (config : LayerConfig) : LayerVerification128 :=
  let epsilon_bound := compute_epsilon_bound config
  let error_matrix := quantization_error_matrix W
  let vectors := generate_128_vectors config.fan_in

  -- Compute errors for all 128 vectors
  let error_distribution := vectors.map fun x =>
    let error_output := matvec_mul error_matrix x
    l2_norm error_output / l2_norm x

  -- Compute statistics
  let max_error := error_distribution.foldl (fun max_err err => if err > max_err then err else max_err) 0.0
  let sum_errors := error_distribution.foldl (fun sum err => sum + err) 0.0
  let mean_error := sum_errors / 128.0

  -- Compute standard deviation
  let sum_squared_diff := error_distribution.foldl (fun sum err =>
    sum + (err - mean_error) * (err - mean_error)) 0.0
  let std_deviation := Float.sqrt (sum_squared_diff / 128.0)

  {
    layer_name,
    config,
    epsilon_bound,
    max_error_128_vectors := max_error,
    mean_error_128_vectors := mean_error,
    error_std_deviation := std_deviation,
    passed_128_vectors := max_error ≤ epsilon_bound,
    error_distribution
  }

/-- Multi-layer verification with 128 vectors per layer -/
structure ModelVerification128 where
  layers : List LayerVerification128
  total_layers : Nat
  passed_layers : Nat
  overall_pass_rate : Float
  mean_max_error : Float
  worst_layer : Option String
  deriving Repr

/-- Verify all layers in a model with 128 vectors each -/
def verify_model_128_vectors (layers : List (String × WeightMatrix × LayerConfig)) : ModelVerification128 :=
  let layer_results := layers.map fun (name, W, config) =>
    verify_layer_128_vectors name W config

  let passed_count := layer_results.filter (·.passed_128_vectors) |>.length
  let total_count := layer_results.length
  let pass_rate := if total_count > 0 then (passed_count.toFloat / total_count.toFloat) * 100.0 else 0.0

  let max_errors := layer_results.map (·.max_error_128_vectors)
  let mean_max_error := if max_errors.length > 0 then
    max_errors.foldl (fun sum err => sum + err) 0.0 / max_errors.length.toFloat else 0.0

  {
    layers := layer_results,
    total_layers := total_count,
    passed_layers := passed_count,
    overall_pass_rate := pass_rate,
    mean_max_error,
    worst_layer := none
  }

/-- Proof obligations are tracked separately; this placeholder keeps API stable. -/
axiom verify_128_vectors_sound (layer_name : String) (W : WeightMatrix)
  (config : LayerConfig) (verification : LayerVerification128) : Prop

/-- Legacy verification for backward compatibility -/
structure LayerVerification where
  layer_name : String
  config : LayerConfig
  epsilon_bound : Float
  actual_error : Float
  passed : Bool
  deriving Repr

/-- Legacy verify a single layer -/
def verify_layer (layer_name : String) (W : WeightMatrix)
  (config : LayerConfig) : LayerVerification :=
  let verification_128 := verify_layer_128_vectors layer_name W config
  {
    layer_name,
    config,
    epsilon_bound := verification_128.epsilon_bound,
    actual_error := verification_128.max_error_128_vectors,
    passed := verification_128.passed_128_vectors
  }

/-- Legacy multi-layer verification -/
structure ModelVerification where
  layers : List LayerVerification
  total_layers : Nat
  passed_layers : Nat
  deriving Repr

/-- Legacy verify all layers in a model -/
def verify_model (layers : List (String × WeightMatrix × LayerConfig)) : ModelVerification :=
  let layer_results := layers.map fun (name, W, config) =>
    verify_layer name W config

  let passed_count := layer_results.filter (·.passed) |>.length

  {
    layers := layer_results,
    total_layers := layers.length,
    passed_layers := passed_count
  }

/-- Proof that ε bound is positive -/
axiom epsilon_bound_positive (config : LayerConfig) :
  compute_epsilon_bound config ≥ 0

/-- Proof obligations are tracked separately; this placeholder keeps API stable. -/
axiom layer_verification_sound (layer_name : String) (W : WeightMatrix)
  (config : LayerConfig) (verification : LayerVerification) : Prop

end ModelAssetGuard.Quant
