import Init.Data.List.Basic
import Init.Data.Nat.Basic

/-!
Lean model of Rust `PerfectHashVocab::hash_seeded` (Wave 20).

Uses `Nat` arithmetic mod `2^64` to mirror wrapping `u64`. This is an
executable refinement of the CHD hash — not a cryptographic claim.
-/

namespace ModelAssetGuard.Token.HashSeeded

/-- `2^64` as the UInt64 modulus. -/
def u64Mod : Nat := 18446744073709551616  -- 2^64

@[inline] def wrap (x : Nat) : Nat := x % u64Mod

/-- Default CHD seeds (must match Rust `DEFAULT_SEED0` / `DEFAULT_SEED1`). -/
def defaultSeed0 : Nat := 0x9e3779b97f4a7c15
def defaultSeed1 : Nat := 0xbf58476d1ce4e5b9

/-- Mix step: `h = h * 33 + b` (wrapping). -/
def mix33 (h b : Nat) : Nat :=
  wrap (wrap h * 33 + (b % 256))

/-- Fold bytes into the accumulator (pre-finalizer). -/
def foldBytes (seed : Nat) (bytes : List Nat) : Nat :=
  bytes.foldl (fun h b => mix33 h b) (wrap (seed + 5381))

/-- SplitMix64 finalizer (wrapping Nat). -/
def splitMix64 (h0 : Nat) : Nat :=
  let h1 := wrap ((wrap h0 ^^^ (wrap h0 / 2 ^ 30)) * 0xbf58476d1ce4e5b9)
  let h2 := wrap ((wrap h1 ^^^ (wrap h1 / 2 ^ 27)) * 0x94d049bb133111eb)
  wrap (wrap h2 ^^^ (wrap h2 / 2 ^ 31))

/-- Full seeded hash over a byte list (values truncated to 8 bits). -/
def hashSeededBytes (seed : Nat) (bytes : List Nat) : Nat :=
  splitMix64 (foldBytes seed bytes)

/-- Hash ASCII / UTF-8 code units given as Nats. -/
def hashSeededAscii (seed : Nat) (s : String) : Nat :=
  hashSeededBytes seed (s.toList.map Char.toNat)

/-- Slot index used by CHD: `hash % n` when `n > 0`. -/
def slotMod (h n : Nat) (_hn : n > 0) : Nat :=
  h % n

theorem slotMod_lt (h n : Nat) (hn : n > 0) : slotMod h n hn < n :=
  Nat.mod_lt h hn

/-- Determinism: same inputs ⇒ same hash. -/
theorem hashSeededBytes_deterministic (seed : Nat) (bytes : List Nat) :
    hashSeededBytes seed bytes = hashSeededBytes seed bytes := rfl

/-- CHD bucket index from seed0. -/
def bucketOf (seed0 nBuckets : Nat) (hn : nBuckets > 0) (bytes : List Nat) : Nat :=
  slotMod (hashSeededBytes seed0 bytes) nBuckets hn

/-- CHD slot from seed1 + displace `d`. -/
def slotOf (seed1 d n : Nat) (hn : n > 0) (bytes : List Nat) : Nat :=
  slotMod (hashSeededBytes (wrap (seed1 + d)) bytes) n hn

/-- Candidate slots for a bucket's keys under displace `d`. -/
def candidateSlots (seed1 d n : Nat) (hn : n > 0) (keys : List (List Nat)) : List Nat :=
  keys.map (fun k => slotOf seed1 d n hn k)

/-- Goldens checked against Rust/Python mirrors (Wave 20). -/
def goldenEmptySeed0 : Nat := 9862407849072414892
def goldenASeed0 : Nat := 13358874647675253293
def goldenAbSeed0 : Nat := 10513868738868360859
def goldenHelloSeed0 : Nat := 5468378884496047854

end ModelAssetGuard.Token.HashSeeded
