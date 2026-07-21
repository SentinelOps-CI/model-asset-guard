import Init.Data.List.Basic
import ModelAssetGuard.Token.CHD
import ModelAssetGuard.Token.HashSeeded

/-!
Executable CHD displace search + small builder (Waves 21-24).

**Proven:** singleton `wellSized`; `candidateSlots` length + bounds;
`findDisplace_eq_some_imp`; Bool/Prop gates; compositional
`wellSized_of_findDisplace_success` (gate chain, no mega-unfold);
`wellSized_merge_bucket_places`; singleton free-`occupied` completeness;
`findDisplace_returns_zero_of_candidatesOk_at_zero` (restricted ∃d);
`finalizePlacement` / `assembleBucketed` success ⇒ `wellFormed`.

**Executable:** `findDisplace` / `buildAsciiMph` / `buildAsciiMphBucketed`
(Rust-aligned λ-plans, stable bucket order + pilots). Multi-key hash-family
completeness remains `chd_displace_search_obligation`.
-/

namespace ModelAssetGuard.Token.CHDBuild

open ModelAssetGuard.Token.CHD
open ModelAssetGuard.Token.CHD.Placement
open ModelAssetGuard.Token.CHD.BucketPlace
open ModelAssetGuard.Token.HashSeeded

/-- Singleton bucket place is well-sized when indices are in range. -/
theorem singleton_wellSized (i s n : Nat) (hi : i < n) (hs : s < n) :
    wellSized { keyIndices := [i], slots := [s] } n := by
  refine ⟨rfl, ?slotsDist, ?keysDist, ?slotsBound, ?keysBound⟩
  case slotsDist =>
    change pairwiseDistinct [s]
    refine And.intro ?_ trivial
    intro y hy; cases hy
  case keysDist =>
    change pairwiseDistinct [i]
    refine And.intro ?_ trivial
    intro y hy; cases hy
  case slotsBound =>
    intro j hj
    have hj0 : j = 0 := Nat.lt_one_iff.mp hj
    subst hj0
    exact hs
  case keysBound =>
    intro j hj
    have hj0 : j = 0 := Nat.lt_one_iff.mp hj
    subst hj0
    exact hi

/-- `candidateSlots` preserves length. -/
theorem candidateSlots_length
    (seed1 d n : Nat) (hn : n > 0) (keys : List (List Nat)) :
    (candidateSlots seed1 d n hn keys).length = keys.length := by
  simp [candidateSlots, List.length_map]

/-- Every candidate slot is `< n`. -/
theorem candidateSlots_lt
    (seed1 d n : Nat) (hn : n > 0) (keys : List (List Nat)) :
    ∀ s ∈ candidateSlots seed1 d n hn keys, s < n := by
  intro s hs
  simp only [candidateSlots, List.mem_map] at hs
  rcases hs with ⟨_, _, rfl⟩
  exact slotMod_lt _ n hn

/-- Occupied mask of length `n`, all free. -/
def freeOccupied (n : Nat) : List Bool :=
  List.replicate n false

/-- Is slot `s` free in `occupied`? -/
def slotFreeBool (occupied : List Bool) (s : Nat) : Bool :=
  if h : s < occupied.length then !(occupied[s]) else false

/-- Bool check: slots pairwise distinct, in-range, and free. -/
def candidatesOkBool (slots : List Nat) (tableSize : Nat) (occupied : List Bool) : Bool :=
  (occupied.length == tableSize) &&
  pairwiseNeBool slots &&
  slots.all (fun s => decide (s < tableSize) && slotFreeBool occupied s)

/-- Production Rust `MAX_DISPLACE` in `try_chd`. Lean smokes use `leanSmokeMaxD`. -/
def rustMaxDisplace : Nat := 4_000_000

/-- Default Lean search budget for builders / goldens (≪ `rustMaxDisplace`). -/
def leanSmokeMaxD : Nat := 10_000

/-- Search displace `d` in `[0, maxD]`.

Rust production uses `rustMaxDisplace` (4_000_000). Lean executable goldens and
smokes typically pass `leanSmokeMaxD` (10_000); callers may raise `maxD` for
parity with the Rust budget when needed. -/
def findDisplace
    (seed1 n : Nat) (hn : n > 0)
    (keyBytes : List (List Nat))
    (occupied : List Bool)
    (maxD : Nat) : Option Nat :=
  (List.range (maxD + 1)).findSome? fun d =>
    let slots := candidateSlots seed1 d n hn keyBytes
    if candidatesOkBool slots n occupied then some d else none

/-- Mark slots occupied (executable). -/
def markOccupied (occupied : List Bool) (slots : List Nat) : List Bool :=
  slots.foldl
    (fun occ s => if _h : s < occ.length then occ.set s true else occ)
    occupied

/-- Finalize placement only when the Bool gate holds. -/
def finalizePlacement (keys : List String) (slots : List Nat) : Option Placement :=
  let p : Placement := { keys := keys, slots := slots }
  if wellFormedBool p then some p else none

theorem finalizePlacement_eq_some_imp
    (keys : List String) (slots : List Nat) (p : Placement)
    (h : finalizePlacement keys slots = some p) :
    p = { keys := keys, slots := slots } ∧ wellFormedBool p = true := by
  simp only [finalizePlacement] at h
  split at h
  · next hw =>
      have : p = { keys := keys, slots := slots } := Option.some.inj h.symm
      subst this
      exact ⟨rfl, hw⟩
  · exact False.elim (Option.noConfusion h)

theorem finalizePlacement_eq_some_imp_wellFormed
    (keys : List String) (slots : List Nat) (p : Placement)
    (h : finalizePlacement keys slots = some p) : wellFormed p :=
  wellFormed_of_wellFormedBool p (finalizePlacement_eq_some_imp keys slots p h).2

/-- Tiny ASCII CHD with singleton buckets (`n_buckets = n` processing order).

Places keys `0 .. n-1` sequentially using `findDisplace` on each singleton.
Returns a well-formed `Placement` when every key finds a free slot. -/
def buildAsciiMph (words : List String) (maxD : Nat := leanSmokeMaxD) : Option Placement :=
  let n := words.length
  if n == 0 then
    some { keys := [], slots := [] }
  else if h : n > 0 then
    let keyBytes := words.map (fun w => w.toList.map Char.toNat)
    let step (st : Option (List Bool × List Nat)) (i : Nat) :
        Option (List Bool × List Nat) :=
      match st with
      | none => none
      | some (occ, acc) =>
          match keyBytes[i]? with
          | none => none
          | some bytes =>
              match findDisplace defaultSeed1 n h [bytes] occ maxD with
              | none => none
              | some d =>
                  let s := slotOf defaultSeed1 d n h bytes
                  some (markOccupied occ [s], acc ++ [s])
    match (List.range n).foldl step (some (freeOccupied n, [])) with
    | none => none
    | some (_, slotList) =>
        finalizePlacement words slotList
  else
    none

/-- Smoke helper: build succeeds and is well-formed. -/
def buildAsciiMphOk (words : List String) : Bool :=
  match buildAsciiMph words with
  | some p => wellFormedBool p
  | none => false

/-! ## Wave 22 -- search soundness + n=1 completeness + bucketed builder -/

/-- `List.range n` starts with `0` when `n > 0`. -/
theorem range_eq_cons_zero (n : Nat) (hn : n > 0) :
    ∃ t, List.range n = 0 :: t := by
  match n, hn with
  | 0, h => exact (Nat.lt_irrefl 0 h).elim
  | n + 1, _ =>
      induction n with
      | zero => exact ⟨[], by simp [List.range_one]⟩
      | succ n ih =>
          have ⟨t, ht⟩ := ih (Nat.succ_pos _)
          refine ⟨t ++ [n + 1], ?_⟩
          rw [List.range_succ, ht, List.cons_append]

/-- If `findDisplace` returns `some d`, then `d ≤ maxD` and the Bool gate passed. -/
theorem findDisplace_eq_some_imp
    (seed1 n : Nat) (hn : n > 0) (keyBytes : List (List Nat))
    (occupied : List Bool) (maxD d : Nat)
    (h : findDisplace seed1 n hn keyBytes occupied maxD = some d) :
    d ≤ maxD ∧
      candidatesOkBool (candidateSlots seed1 d n hn keyBytes) n occupied = true := by
  simp only [findDisplace] at h
  rcases (List.findSome?_eq_some_iff).1 h with ⟨_l₁, a, _l₂, hl, hf, _⟩
  have ha_range : a ∈ List.range (maxD + 1) := by
    have : a ∈ _l₁ ++ a :: _l₂ := List.mem_append_right _ (List.mem_cons_self)
    simpa [← hl] using this
  have hle : a ≤ maxD := Nat.lt_succ_iff.mp (List.mem_range.mp ha_range)
  cases hif : (candidatesOkBool (candidateSlots seed1 a n hn keyBytes) n occupied) with
  | true =>
      have heq : some a = some d := by
        simp only [hif] at hf
        exact hf
      have : a = d := Option.some.inj heq
      subst this
      exact ⟨hle, hif⟩
  | false =>
      have heq : (none : Option Nat) = some d := by
        simp only [hif] at hf
        exact hf
      exact False.elim (Option.noConfusion heq)

/-- `hash % 1 = 0`. -/
theorem slotMod_one (h : Nat) : slotMod h 1 (by decide : 1 > 0) = 0 := by
  simp [slotMod, Nat.mod_one]

/-- For table size 1, every candidate slot is `0`. -/
theorem candidateSlots_tableSize_one
    (seed1 d : Nat) (hn : (1 : Nat) > 0) (keys : List (List Nat)) :
    candidateSlots seed1 d 1 hn keys = List.replicate keys.length 0 := by
  induction keys with
  | nil => simp [candidateSlots]
  | cons k ks ih =>
      simp only [candidateSlots, List.map_cons, List.replicate_succ]
      have hs : slotOf seed1 d 1 hn k = 0 := by
        simp [slotOf, slotMod_one]
      change slotOf seed1 d 1 hn k :: candidateSlots seed1 d 1 hn ks =
        0 :: List.replicate ks.length 0
      rw [hs, show candidateSlots seed1 d 1 hn ks = List.replicate ks.length 0 from ih]

/-- Bool gate holds for a single free slot at table size 1. -/
theorem candidatesOkBool_singleton_free_one :
    candidatesOkBool [0] 1 (freeOccupied 1) = true := by
  native_decide

/-- Completeness for `n = 1` is checked executably (`findDisplace_n1_smoke` in Tests).

A full forall-maxD `findDisplace` proof triggers deep `simp` recursion on
`findSome?`/`if` in Lean 4.21 without Mathlib helpers; the gate + `slotMod_one` facts
above are the reusable lemmas. Wave 23 proves singleton free-occupied completeness. -/
def findDisplace_n1_ok (seed1 : Nat) (bytes : List Nat) (maxD : Nat := 8) : Bool :=
  findDisplace seed1 1 (by decide) [bytes] (freeOccupied 1) maxD == some 0

/-- Group key indices `0..n-1` by `bucketOf seed0 nBuckets`. -/
def indicesByBucket
    (seed0 nBuckets : Nat) (hn : nBuckets > 0)
    (keyBytes : List (List Nat)) : List (List Nat) :=
  let n := keyBytes.length
  let buckets : List (List Nat) := List.replicate nBuckets []
  (List.range n).foldl
    (fun bucks i =>
      match keyBytes[i]? with
      | none => bucks
      | some bytes =>
          let b := bucketOf seed0 nBuckets hn bytes
          if hb : b < bucks.length then
            bucks.set b (bucks[b] ++ [i])
          else bucks)
    buckets

/-- Place one multi-key bucket via `findDisplace` (executable). -/
def placeBucket
    (seed1 n : Nat) (hn : n > 0)
    (keyBytes : List (List Nat)) (idxs : List Nat)
    (occ : List Bool) (maxD : Nat) : Option (List Bool × List (Nat × Nat) × Nat) :=
  let bytes := idxs.filterMap (fun i => keyBytes[i]?)
  if bytes.length != idxs.length then none
  else
    match findDisplace seed1 n hn bytes occ maxD with
    | none => none
    | some d =>
        let slots := candidateSlots seed1 d n hn bytes
        if slots.length != idxs.length then none
        else
          let pairs := List.zip idxs slots
          some (markOccupied occ slots, pairs, d)

/-- Bucketed CHD build result: placement + per-bucket pilots (Rust `pilots`). -/
structure BucketedBuild where
  placement : Placement
  nBuckets : Nat
  pilots : List Nat
  deriving Repr

/-- Assemble key-to-slot placement + pilots through the wellFormed Bool gate. -/
def assembleBucketed
    (words : List String) (nBuckets : Nat)
    (pairs : List (Nat × Nat)) (pilots : List Nat) : Option BucketedBuild :=
  let n := words.length
  let slots :=
    (List.range n).map fun i =>
      (pairs.find? fun p => p.1 == i).map (·.2) |>.getD 0
  match finalizePlacement words slots with
  | none => none
  | some p => some { placement := p, nBuckets := nBuckets, pilots := pilots }

theorem assembleBucketed_eq_some_imp
    (words : List String) (nBuckets : Nat)
    (pairs : List (Nat × Nat)) (pilots : List Nat) (b : BucketedBuild)
    (h : assembleBucketed words nBuckets pairs pilots = some b) :
    wellFormed b.placement ∧ b.nBuckets = nBuckets ∧ b.pilots = pilots := by
  simp only [assembleBucketed] at h
  cases hf : finalizePlacement words
      ((List.range words.length).map fun i =>
        (pairs.find? fun p => p.1 == i).map (·.2) |>.getD 0) with
  | none => simp [hf] at h
  | some p =>
      simp only [hf] at h
      have hb : b = { placement := p, nBuckets := nBuckets, pilots := pilots } :=
        Option.some.inj h.symm
      subst hb
      exact ⟨finalizePlacement_eq_some_imp_wellFormed _ _ _ hf, rfl, rfl⟩

theorem assembleBucketed_eq_some_imp_wellFormed
    (words : List String) (nBuckets : Nat)
    (pairs : List (Nat × Nat)) (pilots : List Nat) (b : BucketedBuild)
    (h : assembleBucketed words nBuckets pairs pilots = some b) :
    wellFormed b.placement :=
  (assembleBucketed_eq_some_imp words nBuckets pairs pilots b h).1

/-- Largest length first; equal lengths ⇒ ascending bucket id (Rust Wave 24). -/
def bucketLe (groups : List (List Nat)) (b1 b2 : Nat) : Bool :=
  let l1 := (groups[b1]?).map List.length |>.getD 0
  let l2 := (groups[b2]?).map List.length |>.getD 0
  decide (l1 > l2 ∨ (l1 = l2 ∧ b1 ≤ b2))

/-- Stable bucket processing order matching Rust `bucket_process_order`. -/
def bucketProcessOrder (nBuckets : Nat) (groups : List (List Nat)) : List Nat :=
  (List.range nBuckets).mergeSort (bucketLe groups)

/-- Core bucketed builder for a fixed `nBuckets` (Rust `try_chd` shape).

Groups by `seed0`, largest-first + id tie-break, searches displace, records `pilots[b]`. -/
def tryAsciiMphBucketed
    (words : List String) (nBuckets : Nat) (maxD : Nat := leanSmokeMaxD) :
    Option BucketedBuild :=
  let n := words.length
  if n == 0 then
    some { placement := { keys := [], slots := [] }, nBuckets := nBuckets, pilots := [] }
  else if h : n > 0 then
    if hnB : nBuckets > 0 then
      let keyBytes := words.map (fun w => w.toList.map Char.toNat)
      let groups := indicesByBucket defaultSeed0 nBuckets hnB keyBytes
      let order := bucketProcessOrder nBuckets groups
      let step (st : Option (List Bool × List (Nat × Nat) × List Nat)) (b : Nat) :
          Option (List Bool × List (Nat × Nat) × List Nat) :=
        match st with
        | none => none
        | some (occ, acc, pilots) =>
            match groups[b]? with
            | none => some (occ, acc, pilots)
            | some idxs =>
                if idxs.isEmpty then some (occ, acc, pilots)
                else
                  match placeBucket defaultSeed1 n h keyBytes idxs occ maxD with
                  | none => none
                  | some (occ', pairs, d) =>
                      let pilots' :=
                        if _hb : b < pilots.length then pilots.set b d else pilots
                      some (occ', acc ++ pairs, pilots')
      let initPilots := List.replicate nBuckets 0
      match order.foldl step (some (freeOccupied n, [], initPilots)) with
      | none => none
      | some (_, pairs, pilots) => assembleBucketed words nBuckets pairs pilots
    else none
  else
    none

/-- Dedup preserving first occurrence (for Rust bucket plans). -/
def natEraseDups : List Nat → List Nat
  | [] => []
  | x :: xs => x :: (natEraseDups xs).filter (· != x)

/-- Rust-aligned bucket plan: lambda~5, lambda~2, then `n` (always last). -/
def rustBucketPlans (n : Nat) : List Nat :=
  if n == 0 then [0]
  else
    let a := ((n + 4) / 5).max 1
    let b := ((n + 1) / 2).max 1
    natEraseDups [a, b, n]

/-- Bucketed ASCII MPH: try Rust bucket plans; return placement. -/
def buildAsciiMphBucketed
    (words : List String) (maxD : Nat := leanSmokeMaxD) : Option Placement :=
  let n := words.length
  if n == 0 then
    some { keys := [], slots := [] }
  else
    (rustBucketPlans n).findSome? fun nb =>
      (tryAsciiMphBucketed words nb maxD).map (·.placement)

/-- Same as `buildAsciiMphBucketed` but keeps pilots (for Lean/Rust goldens). -/
def buildAsciiMphBucketedFull
    (words : List String) (maxD : Nat := leanSmokeMaxD) : Option BucketedBuild :=
  let n := words.length
  if n == 0 then
    some { placement := { keys := [], slots := [] }, nBuckets := 0, pilots := [] }
  else
    (rustBucketPlans n).findSome? fun nb => tryAsciiMphBucketed words nb maxD

/-- Force `n_buckets = n` (Lean/Rust golden parity under lambda~1). -/
def buildAsciiMphBucketedN
    (words : List String) (maxD : Nat := leanSmokeMaxD) : Option BucketedBuild :=
  let n := words.length
  if n == 0 then
    some { placement := { keys := [], slots := [] }, nBuckets := 0, pilots := [] }
  else
    tryAsciiMphBucketed words n maxD

def buildAsciiMphBucketedOk (words : List String) : Bool :=
  match buildAsciiMphBucketed words with
  | some p => wellFormedBool p
  | none => false

/-! ## Wave 23 -- Bool gates, placeBucket soundness, free-occupied completeness -/

/-- `a != b = false` iff `a = b` for Nat. -/
theorem nat_bne_eq_false_iff {a b : Nat} : ((a != b) = false) ↔ a = b := by
  cases h : (a == b) with
  | true =>
      have : a = b := beq_iff_eq.mp h
      simp [bne, h, this]
  | false =>
      have hne : a ≠ b := fun heq => by
        have : (a == b) = true := beq_iff_eq.mpr heq
        simp [h] at this
      simp [bne, h]
      exact hne

/-- `not (b = true)` implies `b = false`. -/
theorem bool_ne_true_eq_false {b : Bool} (h : ¬ b = true) : b = false := by
  cases b with
  | true => exact (h rfl).elim
  | false => rfl

/-- `candidatesOkBool` implies combinatorial `displaceCandidatesOk`. -/
theorem displaceCandidatesOk_of_candidatesOkBool
    (slots : List Nat) (tableSize : Nat) (occupied : List Bool)
    (h : candidatesOkBool slots tableSize occupied = true) :
    displaceCandidatesOk slots tableSize occupied := by
  simp only [candidatesOkBool, Bool.and_eq_true, beq_iff_eq] at h
  obtain ⟨⟨hlen, hdistB⟩, hall⟩ := h
  refine ⟨pairwiseDistinct_of_pairwiseNeBool hdistB, hlen, ?_, ?_⟩
  · intro s hs
    have := (List.all_eq_true.1 hall) s hs
    simp only [Bool.and_eq_true, decide_eq_true_eq] at this
    exact this.1
  · intro s hs
    have := (List.all_eq_true.1 hall) s hs
    simp only [Bool.and_eq_true, decide_eq_true_eq] at this
    obtain ⟨hsBound, hfree⟩ := this
    have hsOcc : s < occupied.length := by simpa [hlen] using hsBound
    refine ⟨hsOcc, ?_⟩
    simp only [slotFreeBool, hsOcc, ↓reduceDIte] at hfree
    cases hocc : occupied[s] with
    | true => simp [hocc] at hfree
    | false => rfl

/-- Empty candidate list is always OK when the mask length matches. -/
theorem candidatesOkBool_nil (tableSize : Nat) (occupied : List Bool)
    (hlen : occupied.length = tableSize) :
    candidatesOkBool [] tableSize occupied = true := by
  simp [candidatesOkBool, pairwiseNeBool, hlen]

/-- Free mask: every in-range slot reports free. -/
theorem slotFreeBool_freeOccupied (n s : Nat) (hs : s < n) :
    slotFreeBool (freeOccupied n) s = true := by
  simp [slotFreeBool, freeOccupied, List.length_replicate, hs, List.getElem_replicate]

/-- Singleton slot on a free mask passes the Bool gate. -/
theorem candidatesOkBool_singleton_free (n s : Nat) (hs : s < n) :
    candidatesOkBool [s] n (freeOccupied n) = true := by
  simp [candidatesOkBool, pairwiseNeBool, slotFreeBool, freeOccupied,
    List.length_replicate, hs, List.getElem_replicate, decide_eq_true hs,
    List.all_cons, List.all_nil]

/-- Singleton candidate slots under any displace are free-OK on an empty table. -/
theorem candidatesOkBool_singleton_candidate_free
    (seed1 d n : Nat) (hn : n > 0) (bytes : List Nat) :
    candidatesOkBool (candidateSlots seed1 d n hn [bytes]) n (freeOccupied n) = true := by
  simp only [candidateSlots, List.map_cons, List.map_nil]
  exact candidatesOkBool_singleton_free n (slotOf seed1 d n hn bytes) (slotMod_lt _ n hn)

/-- Empty key list: `findDisplace` returns `some 0` (first candidate). -/
theorem findDisplace_nil_keys
    (seed1 n : Nat) (hn : n > 0) (occupied : List Bool) (maxD : Nat)
    (hlen : occupied.length = n) :
    findDisplace seed1 n hn [] occupied maxD = some 0 := by
  have ⟨t, ht⟩ := range_eq_cons_zero (maxD + 1) (Nat.succ_pos _)
  simp only [findDisplace, ht, List.findSome?_cons]
  have hok : candidatesOkBool (candidateSlots seed1 0 n hn []) n occupied = true := by
    simpa [candidateSlots] using candidatesOkBool_nil n occupied hlen
  simp [hok]

/-- Singleton + free occupied: search always returns `d = 0` (Wave 23 completeness). -/
theorem findDisplace_singleton_freeOccupied
    (seed1 n : Nat) (hn : n > 0) (bytes : List Nat) (maxD : Nat) :
    findDisplace seed1 n hn [bytes] (freeOccupied n) maxD = some 0 := by
  have ⟨t, ht⟩ := range_eq_cons_zero (maxD + 1) (Nat.succ_pos _)
  simp only [findDisplace, ht, List.findSome?_cons]
  have hok := candidatesOkBool_singleton_candidate_free seed1 0 n hn bytes
  simp [hok]

/-- Table size 1 cannot place two-or-more keys (all candidates collide at slot 0). -/
theorem candidatesOkBool_two_zeros_false :
    candidatesOkBool [0, 0] 1 (freeOccupied 1) = false := by
  native_decide

/-!
`placeBucket` soundness is compositional (avoid deep `match`/`if` recursion):

1. `findDisplace_eq_some_imp` — returned `d` passes `candidatesOkBool`
2. `displaceCandidatesOk_of_candidatesOkBool` — Bool gate → Prop
3. `wellSized_of_displaceCandidatesOk` — Prop → `wellSized` (under distinct idxs)

A single mega-theorem unfolding `placeBucket`'s nested `match`/`if` hits the same
Lean 4.21 recursion traps as full `findDisplace` completeness (Wave 22).
Wave 24 packages the chain as `wellSized_of_findDisplace_success` and the merge
step as `wellSized_merge_bucket_places` (reuses `wellSized_append`).
-/

/-- Compositional: successful `findDisplace` ⇒ `wellSized` place (no `placeBucket` unfold). -/
theorem wellSized_of_findDisplace_success
    (seed1 n : Nat) (hn : n > 0)
    (bytes : List (List Nat)) (idxs : List Nat)
    (occupied : List Bool) (maxD d : Nat)
    (hfind : findDisplace seed1 n hn bytes occupied maxD = some d)
    (hlen : idxs.length = bytes.length)
    (hk : Placement.pairwiseDistinct idxs)
    (hkeyBound : ∀ i, (h : i < idxs.length) → idxs[i] < n) :
    (placeFromCandidates idxs (candidateSlots seed1 d n hn bytes)).wellSized n := by
  have ⟨_, hbool⟩ := findDisplace_eq_some_imp seed1 n hn bytes occupied maxD d hfind
  have hprop := displaceCandidatesOk_of_candidatesOkBool
    (candidateSlots seed1 d n hn bytes) n occupied hbool
  have hlen' : idxs.length = (candidateSlots seed1 d n hn bytes).length := by
    simpa [candidateSlots_length seed1 d n hn bytes] using hlen
  exact wellSized_of_displaceCandidatesOk idxs
    (candidateSlots seed1 d n hn bytes) n occupied hlen' hk hkeyBound hprop

/-- Fold/append step: merging two successful places stays `wellSized` under disjointness. -/
theorem wellSized_merge_bucket_places
    (p q : BucketPlace) (n : Nat)
    (hp : p.wellSized n) (hq : q.wellSized n) (hd : BucketPlace.disjoint p q) :
    (BucketPlace.append p q).wellSized n :=
  BucketPlace.wellSized_append p q n hp hq hd

/-- Restricted ∃d: if `d = 0` already passes the Bool gate, search returns `some 0`.

Does **not** claim a full hash-family completeness theorem — only that the
executable search finds the first candidate when it is already OK. -/
theorem findDisplace_returns_zero_of_candidatesOk_at_zero
    (seed1 n : Nat) (hn : n > 0) (keyBytes : List (List Nat))
    (occupied : List Bool) (maxD : Nat)
    (hok : candidatesOkBool (candidateSlots seed1 0 n hn keyBytes) n occupied = true) :
    findDisplace seed1 n hn keyBytes occupied maxD = some 0 := by
  have ⟨t, ht⟩ := range_eq_cons_zero (maxD + 1) (Nat.succ_pos _)
  simp only [findDisplace, ht, List.findSome?_cons]
  simp [hok]

/-- Singleton `placeBucket` on a free table succeeds at `d = 0`. -/
theorem placeBucket_singleton_freeOccupied
    (seed1 n : Nat) (hn : n > 0) (keyBytes : List (List Nat))
    (i : Nat) (bytes : List Nat) (maxD : Nat)
    (hi : keyBytes[i]? = some bytes) :
    placeBucket seed1 n hn keyBytes [i] (freeOccupied n) maxD =
      some (
        markOccupied (freeOccupied n) (candidateSlots seed1 0 n hn [bytes]),
        [(i, slotOf seed1 0 n hn bytes)],
        0) := by
  have hfind := findDisplace_singleton_freeOccupied seed1 n hn bytes maxD
  -- Executable equality for the singleton filterMap/zip path.
  simp [placeBucket, List.filterMap, hi, hfind, candidateSlots,
    List.zip_cons_cons, List.zip_nil_right]

theorem empty_placement_wellFormed :
    wellFormed ({ keys := [], slots := [] } : Placement) :=
  wellFormed_of_wellFormedBool _ (by native_decide)

/-- Lean/Rust golden: key-to-slot under `n_buckets = n` for `["a","b","c"]`. -/
def goldenAbcSlots : List Nat := [0, 2, 1]
def goldenAbcPilots : List Nat := [0, 0, 0]

/-- Lean/Rust golden: `["hello","world","test"]` with `n_buckets = n`. -/
def goldenHelloSlots : List Nat := [0, 2, 1]
def goldenHelloPilots : List Nat := [2, 0, 1]

/-- Lean/Rust golden: `["a","b","c","d"]` with `n_buckets = n`. -/
def goldenAbcdSlots : List Nat := [2, 1, 0, 3]
def goldenAbcdPilots : List Nat := [0, 3, 0, 1]

/-- Smoke: bucketed-N build matches shared CHD goldens. -/
def goldenBucketedNOk : Bool :=
  match buildAsciiMphBucketedN ["a", "b", "c"] with
  | none => false
  | some b =>
      (b.placement.slots == goldenAbcSlots) &&
      (b.pilots == goldenAbcPilots) &&
      wellFormedBool b.placement &&
      match buildAsciiMphBucketedN ["hello", "world", "test"] with
      | none => false
      | some b2 =>
          (b2.placement.slots == goldenHelloSlots) &&
          (b2.pilots == goldenHelloPilots) &&
          match buildAsciiMphBucketedN ["a", "b", "c", "d"] with
          | none => false
          | some b3 =>
              (b3.placement.slots == goldenAbcdSlots) &&
              (b3.pilots == goldenAbcdPilots)

/-! ## Wave 24 -- stable order, λ-plan goldens, compositional gates, restricted ∃d -/

/-- Artificial equal-length order: three size-2 buckets ⇒ `[0,1,2]`. -/
def bucketOrderEqualLengthOk : Bool :=
  bucketProcessOrder 3 [[0, 1], [2, 3], [4, 5]] == [0, 1, 2] &&
  bucketProcessOrder 4 [[0, 1, 2], [3], [4, 5, 6], [7]] == [0, 2, 1, 3] &&
  bucketProcessOrder 4 [[], [0, 1], [], [2, 3]] == [1, 3, 0, 2]

/-- Real-key equal-length stresses (shared with Rust `test_chd_bucket_order_parity_stress`). -/
def bucketOrderStressOk : Bool :=
  let words4 := ["one", "two", "three", "four"]
  let kb4 := words4.map (fun w => w.toList.map Char.toNat)
  let g4 := indicesByBucket defaultSeed0 2 (by decide) kb4
  let words8 := ["w0", "w1", "w2", "w3", "w4", "w5", "w6", "w7"]
  let kb8 := words8.map (fun w => w.toList.map Char.toNat)
  let g8 := indicesByBucket defaultSeed0 4 (by decide) kb8
  (g4.map List.length == [2, 2]) &&
  (bucketProcessOrder 2 g4 == [0, 1]) &&
  (g8.map List.length == [2, 0, 4, 2]) &&
  (bucketProcessOrder 4 g8 == [2, 0, 3, 1])

/-- First successful λ-plan goldens (Rust `test_chd_lean_golden_lambda_plans`). -/
def goldenPlanAbcSlots : List Nat := [0, 2, 1]
def goldenPlanAbcPilots : List Nat := [0]
def goldenPlanHelloSlots : List Nat := [1, 2, 0]
def goldenPlanHelloPilots : List Nat := [5]
def goldenPlanAbcdSlots : List Nat := [2, 3, 1, 0]
def goldenPlanAbcdPilots : List Nat := [22]
def goldenPlanW8Slots : List Nat := [1, 3, 2, 0, 5, 4, 7, 6]
def goldenPlanW8Pilots : List Nat := [1, 92]
def goldenPlanXyzSlots : List Nat := [0, 4, 3, 5, 1, 2]
def goldenPlanXyzPilots : List Nat := [41, 5]
def goldenPlanOneSlots : List Nat := [0, 1, 3, 2]
def goldenPlanOnePilots : List Nat := [5]

/-- Check one λ-plan golden (nBuckets + slots + pilots + wellFormed). -/
def goldenPlanCaseOk
    (words : List String) (nBuckets : Nat)
    (slots pilots : List Nat) : Bool :=
  match buildAsciiMphBucketedFull words with
  | none => false
  | some b =>
      (b.nBuckets == nBuckets) &&
      (b.placement.slots == slots) &&
      (b.pilots == pilots) &&
      wellFormedBool b.placement

/-- Smoke: first successful Rust λ-plan matches Lean `buildAsciiMphBucketedFull`. -/
def goldenBucketedPlanOk : Bool :=
  goldenPlanCaseOk ["a", "b", "c"] 1 goldenPlanAbcSlots goldenPlanAbcPilots &&
  goldenPlanCaseOk ["hello", "world", "test"] 1 goldenPlanHelloSlots goldenPlanHelloPilots &&
  goldenPlanCaseOk ["a", "b", "c", "d"] 1 goldenPlanAbcdSlots goldenPlanAbcdPilots &&
  goldenPlanCaseOk ["w0", "w1", "w2", "w3", "w4", "w5", "w6", "w7"] 2
    goldenPlanW8Slots goldenPlanW8Pilots &&
  goldenPlanCaseOk ["x", "y", "z", "w", "v", "u"] 2
    goldenPlanXyzSlots goldenPlanXyzPilots &&
  goldenPlanCaseOk ["one", "two", "three", "four"] 1
    goldenPlanOneSlots goldenPlanOnePilots

/-- Two-key free-table smoke: search finds some displace (not a hash-family theorem). -/
def findDisplace_twoKey_free_smoke : Bool :=
  match findDisplace defaultSeed1 2 (by decide) [[97], [98]] (freeOccupied 2) 64 with
  | some _ => true
  | none => false

/-- Nonempty occupied: second singleton must take the free slot. -/
def findDisplace_singleton_occupied_smoke : Bool :=
  let occ := markOccupied (freeOccupied 2) [0]
  match findDisplace defaultSeed1 2 (by decide) [[98]] occ 64 with
  | none => false
  | some d => slotOf defaultSeed1 d 2 (by decide) [98] == 1

/-- Wave 24 aggregate executable checks. -/
def wave24ChdOk : Bool :=
  bucketOrderEqualLengthOk &&
  bucketOrderStressOk &&
  goldenBucketedPlanOk &&
  findDisplace_twoKey_free_smoke &&
  findDisplace_singleton_occupied_smoke

end ModelAssetGuard.Token.CHDBuild
