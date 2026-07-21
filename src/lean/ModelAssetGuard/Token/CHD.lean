import Init.Data.List.Basic

/-!
Abstract CHD / MPH placement + construction fragment (Waves 18–19).

Runtime builds a Compress-Hash-Displace table (`kind: "chd_mph"`) in Rust.
This module does **not** formalize `hash_seeded` or the displace search loop.
It captures:

1. Post-condition of a successful MPH: injective key→slot placement
2. Combinatorial merge of successful bucket places (Rust post-conditions)
-/

namespace ModelAssetGuard.Token.CHD

/-- Placement of vocab keys into MPH slots (parallel lists). -/
structure Placement where
  keys : List String
  slots : List Nat
  deriving Repr

namespace Placement

/-- Number of keys (intended table size). -/
def n (p : Placement) : Nat := p.keys.length

/-- Pairwise distinctness on a list (first-occurrence order). -/
def pairwiseDistinct {α : Type} : List α → Prop
  | [] => True
  | x :: xs => (∀ y ∈ xs, x ≠ y) ∧ pairwiseDistinct xs

theorem pairwiseDistinct_of_mem_ne {α : Type}
    {xs : List α} (h : pairwiseDistinct xs) {i j : Nat}
    (hi : i < xs.length) (hj : j < xs.length) (hne : i ≠ j) :
    xs[i] ≠ xs[j] := by
  induction xs generalizing i j with
  | nil => cases hi
  | cons x xs ih =>
      obtain ⟨hxdist, hrest⟩ := h
      match i, j with
      | 0, 0 => exact (hne rfl).elim
      | 0, j' + 1 =>
          have hj' : j' < xs.length := Nat.lt_of_succ_lt_succ hj
          have mem : xs[j'] ∈ xs := List.getElem_mem hj'
          exact fun heq => (hxdist xs[j'] mem) heq
      | i' + 1, 0 =>
          have hi' : i' < xs.length := Nat.lt_of_succ_lt_succ hi
          have mem : xs[i'] ∈ xs := List.getElem_mem hi'
          exact fun heq => (hxdist xs[i'] mem) heq.symm
      | i' + 1, j' + 1 =>
          have hi' : i' < xs.length := Nat.lt_of_succ_lt_succ hi
          have hj' : j' < xs.length := Nat.lt_of_succ_lt_succ hj
          have hne' : i' ≠ j' := mt (congrArg Nat.succ) hne
          exact ih hrest hi' hj' hne'

/-- Append preserves pairwise distinctness when sides are distinct and cross-disjoint. -/
theorem pairwiseDistinct_append {α : Type} {xs ys : List α}
    (hx : pairwiseDistinct xs) (hy : pairwiseDistinct ys)
    (hdisj : ∀ x ∈ xs, ∀ y ∈ ys, x ≠ y) :
    pairwiseDistinct (xs ++ ys) := by
  induction xs with
  | nil => simpa using hy
  | cons x xs ih =>
      obtain ⟨hxhead, hxrest⟩ := hx
      refine ⟨?_, ih hxrest (fun z hz y hy => hdisj z (List.mem_cons_of_mem x hz) y hy)⟩
      intro z hz
      have hz' : z ∈ xs ++ ys := by simpa using hz
      match List.mem_append.1 hz' with
      | Or.inl hzx => exact hxhead z hzx
      | Or.inr hzy => exact hdisj x (List.Mem.head xs) z hzy

/-- Successful CHD/MPH build post-condition (abstract). -/
def wellFormed (p : Placement) : Prop :=
  p.slots.length = p.keys.length ∧
  (∀ i, (h : i < p.slots.length) → p.slots[i] < p.keys.length) ∧
  pairwiseDistinct p.slots ∧
  pairwiseDistinct p.keys

/-- Constructor interface used by construction lemmas. -/
theorem wellFormed_mk
    (keys : List String) (slots : List Nat)
    (hlen : slots.length = keys.length)
    (hbound : ∀ i, (h : i < slots.length) → slots[i] < keys.length)
    (hslots : pairwiseDistinct slots)
    (hkeys : pairwiseDistinct keys) :
    wellFormed { keys := keys, slots := slots } :=
  ⟨hlen, hbound, hslots, hkeys⟩

/-- Distinct key indices receive distinct slots. -/
theorem slots_injective_of_wellFormed
    (p : Placement) (hw : p.wellFormed)
    {i j : Nat} (hi : i < p.slots.length) (hj : j < p.slots.length)
    (heq : p.slots[i] = p.slots[j]) : i = j := by
  have hdist := hw.2.2.1
  match decEq i j with
  | isTrue h => exact h
  | isFalse hne => exact absurd heq (pairwiseDistinct_of_mem_ne hdist hi hj hne)

/-- Distinct key strings at distinct indices. -/
theorem keys_injective_of_wellFormed
    (p : Placement) (hw : p.wellFormed)
    {i j : Nat} (hi : i < p.keys.length) (hj : j < p.keys.length)
    (heq : p.keys[i] = p.keys[j]) : i = j := by
  have hdist := hw.2.2.2
  match decEq i j with
  | isTrue h => exact h
  | isFalse hne => exact absurd heq (pairwiseDistinct_of_mem_ne hdist hi hj hne)

/-- Same slot ↔ same key index under well-formedness. -/
theorem slot_eq_implies_index_eq
    (p : Placement) (hw : p.wellFormed)
    {i j : Nat} (hi : i < p.slots.length) (hj : j < p.slots.length) :
    p.slots[i] = p.slots[j] ↔ i = j :=
  ⟨fun h => slots_injective_of_wellFormed p hw hi hj h, fun h => h ▸ rfl⟩

/-- Pairwise `≠` as a Bool for smoke tests. -/
def pairwiseNeBool {α : Type} [BEq α] : List α → Bool
  | [] => true
  | x :: xs => xs.all (fun y => y != x) && pairwiseNeBool xs

/-- Bool gate matches Prop for lawful `BEq` types (Wave 23). -/
theorem pairwiseNeBool_eq_true_iff_pairwiseDistinct {α : Type}
    [BEq α] [LawfulBEq α] (xs : List α) :
    pairwiseNeBool xs = true ↔ pairwiseDistinct xs := by
  induction xs with
  | nil => simp [pairwiseNeBool, pairwiseDistinct]
  | cons x xs ih =>
      simp only [pairwiseNeBool, pairwiseDistinct, Bool.and_eq_true]
      constructor
      · intro ⟨hall, hrest⟩
        refine ⟨?_, ih.mp hrest⟩
        intro y hy
        exact (bne_iff_ne.mp ((List.all_eq_true.1 hall) y hy)).symm
      · intro ⟨hne, hrest⟩
        refine ⟨?_, ih.mpr hrest⟩
        exact List.all_eq_true.2 fun y hy => bne_iff_ne.mpr (hne y hy).symm

theorem pairwiseDistinct_of_pairwiseNeBool {α : Type}
    [BEq α] [LawfulBEq α] {xs : List α} (h : pairwiseNeBool xs = true) :
    pairwiseDistinct xs :=
  (pairwiseNeBool_eq_true_iff_pairwiseDistinct xs).1 h

/-- Executable well-formedness check for smoke tests. -/
def wellFormedBool (p : Placement) : Bool :=
  let n := p.keys.length
  (p.slots.length == n) &&
  p.slots.all (fun s => decide (s < n)) &&
  pairwiseNeBool p.slots &&
  pairwiseNeBool p.keys

/-- `wellFormedBool = true` implies abstract `wellFormed` (Wave 23). -/
theorem wellFormed_of_wellFormedBool (p : Placement)
    (h : wellFormedBool p = true) : wellFormed p := by
  simp only [wellFormedBool, Bool.and_eq_true, beq_iff_eq] at h
  obtain ⟨⟨⟨hlen, hboundB⟩, hslotsB⟩, hkeysB⟩ := h
  refine ⟨hlen, ?_, pairwiseDistinct_of_pairwiseNeBool hslotsB,
    pairwiseDistinct_of_pairwiseNeBool hkeysB⟩
  intro i hi
  have hmem : p.slots[i] ∈ p.slots := List.getElem_mem hi
  have := (List.all_eq_true.1 hboundB) p.slots[i] hmem
  exact of_decide_eq_true this

/-- Tiny example: two keys → slots 0,1 (abstract MPH). -/
def exampleTwo : Placement :=
  { keys := ["a", "b"], slots := [0, 1] }

/-- Collision example (not well-formed): two keys share slot 0. -/
def exampleCollision : Placement :=
  { keys := ["a", "b"], slots := [0, 0] }

/-- Three-key MPH example (slots are a permutation of `0..2`). -/
def exampleThree : Placement :=
  { keys := ["x", "y", "z"], slots := [2, 0, 1] }

end Placement

/-! ## Construction fragment (Wave 19)

Models successful CHD bucket places as parallel `(keyIndex, slot)` lists.
Does not claim the Rust search finds such places — only that merging
successful places preserves injectivity. -/

/-- One successful bucket displace: parallel key indices → slots. -/
structure BucketPlace where
  keyIndices : List Nat
  slots : List Nat
  deriving Repr

namespace BucketPlace

/-- Combinatorial success condition for one CHD bucket (no hash model). -/
def wellSized (bp : BucketPlace) (tableSize : Nat) : Prop :=
  bp.keyIndices.length = bp.slots.length ∧
  Placement.pairwiseDistinct bp.slots ∧
  Placement.pairwiseDistinct bp.keyIndices ∧
  (∀ i, (h : i < bp.slots.length) → bp.slots[i] < tableSize) ∧
  (∀ i, (h : i < bp.keyIndices.length) → bp.keyIndices[i] < tableSize)

/-- Concatenate two bucket places (sequential CHD bucket processing). -/
def append (p q : BucketPlace) : BucketPlace :=
  { keyIndices := p.keyIndices ++ q.keyIndices
    slots := p.slots ++ q.slots }

/-- Cross-disjoint slots and key indices. -/
def disjoint (p q : BucketPlace) : Prop :=
  (∀ s ∈ p.slots, ∀ t ∈ q.slots, s ≠ t) ∧
  (∀ a ∈ p.keyIndices, ∀ b ∈ q.keyIndices, a ≠ b)

/-- Merging two well-sized, cross-disjoint places yields a well-sized place. -/
theorem wellSized_append
    (p q : BucketPlace) (n : Nat)
    (hp : p.wellSized n) (hq : q.wellSized n) (hd : disjoint p q) :
    (append p q).wellSized n := by
  obtain ⟨hlenp, hsp, hkp, hsbp, hkbp⟩ := hp
  obtain ⟨hlenq, hsq, hkq, hsbq, hkbq⟩ := hq
  refine ⟨?_, ?_, ?_, ?_, ?_⟩
  · simp [append, hlenp, hlenq]
  · exact Placement.pairwiseDistinct_append hsp hsq hd.1
  · exact Placement.pairwiseDistinct_append hkp hkq hd.2
  · intro i hi
    simp only [append] at hi ⊢
    have hi' : i < p.slots.length + q.slots.length := by
      simpa [List.length_append] using hi
    by_cases hlt : i < p.slots.length
    · have : (p.slots ++ q.slots)[i] = p.slots[i] := List.getElem_append_left hlt
      rw [this]
      exact hsbp i hlt
    · have hge : p.slots.length ≤ i := Nat.le_of_not_gt hlt
      have hj : i - p.slots.length < q.slots.length := by
        have := Nat.sub_lt_left_of_lt_add hge hi'
        simpa using this
      have : (p.slots ++ q.slots)[i] = q.slots[i - p.slots.length] :=
        List.getElem_append_right hge
      rw [this]
      exact hsbq (i - p.slots.length) hj
  · intro i hi
    simp only [append] at hi ⊢
    have hi' : i < p.keyIndices.length + q.keyIndices.length := by
      simpa [List.length_append] using hi
    by_cases hlt : i < p.keyIndices.length
    · have : (p.keyIndices ++ q.keyIndices)[i] = p.keyIndices[i] :=
        List.getElem_append_left hlt
      rw [this]
      exact hkbp i hlt
    · have hge : p.keyIndices.length ≤ i := Nat.le_of_not_gt hlt
      have hj : i - p.keyIndices.length < q.keyIndices.length := by
        have := Nat.sub_lt_left_of_lt_add hge hi'
        simpa using this
      have : (p.keyIndices ++ q.keyIndices)[i] =
          q.keyIndices[i - p.keyIndices.length] := List.getElem_append_right hge
      rw [this]
      exact hkbq (i - p.keyIndices.length) hj

/-- Empty bucket place is well-sized. -/
theorem wellSized_nil (n : Nat) :
    wellSized { keyIndices := [], slots := [] } n := by
  refine ⟨rfl, trivial, trivial, ?_, ?_⟩
  · intro i hi; cases hi
  · intro i hi; cases hi

/-- Executable wellSized check for smoke tests. -/
def wellSizedBool (bp : BucketPlace) (tableSize : Nat) : Bool :=
  (bp.keyIndices.length == bp.slots.length) &&
  Placement.pairwiseNeBool bp.slots &&
  Placement.pairwiseNeBool bp.keyIndices &&
  bp.slots.all (fun s => decide (s < tableSize)) &&
  bp.keyIndices.all (fun k => decide (k < tableSize))

/-- `wellSizedBool = true` implies `wellSized` (Wave 23). -/
theorem wellSized_of_wellSizedBool (bp : BucketPlace) (tableSize : Nat)
    (h : wellSizedBool bp tableSize = true) : wellSized bp tableSize := by
  simp only [wellSizedBool, Bool.and_eq_true, beq_iff_eq] at h
  obtain ⟨⟨⟨⟨hlen, hslotsB⟩, hkeysB⟩, hsb⟩, hkb⟩ := h
  refine ⟨hlen, Placement.pairwiseDistinct_of_pairwiseNeBool hslotsB,
    Placement.pairwiseDistinct_of_pairwiseNeBool hkeysB, ?_, ?_⟩
  · intro i hi
    have hmem : bp.slots[i] ∈ bp.slots := List.getElem_mem hi
    exact of_decide_eq_true ((List.all_eq_true.1 hsb) bp.slots[i] hmem)
  · intro i hi
    have hmem : bp.keyIndices[i] ∈ bp.keyIndices := List.getElem_mem hi
    exact of_decide_eq_true ((List.all_eq_true.1 hkb) bp.keyIndices[i] hmem)

/-- Cross-disjoint Bool check (smoke). -/
def disjointBool (p q : BucketPlace) : Bool :=
  p.slots.all (fun s => q.slots.all (fun t => s != t)) &&
  p.keyIndices.all (fun a => q.keyIndices.all (fun b => a != b))

end BucketPlace

/-- Displace-search correctness for a *found* displace (Wave 20).

If `candidateSlots` under `(seed1,d,n)` are pairwise distinct and free in
`occupied`, the resulting `BucketPlace` is `wellSized`. This discharges the
combinatorial half of CHD placement once Rust (or an oracle) supplies `d`.
Hash correctness is `HashSeeded`; finding `d` remains `chd_displace_search_obligation`. -/
def displaceCandidatesOk
    (slots : List Nat) (tableSize : Nat) (occupied : List Bool) : Prop :=
  Placement.pairwiseDistinct slots ∧
  occupied.length = tableSize ∧
  (∀ s ∈ slots, s < tableSize) ∧
  (∀ s ∈ slots, ∃ h : s < occupied.length, occupied[s] = false)

/-- Build a bucket place from key indices + candidate slots of equal length. -/
def placeFromCandidates (keyIndices slots : List Nat) : BucketPlace :=
  { keyIndices := keyIndices, slots := slots }

theorem wellSized_of_displaceCandidatesOk
    (keyIndices slots : List Nat) (tableSize : Nat) (occupied : List Bool)
    (hlen : keyIndices.length = slots.length)
    (hk : Placement.pairwiseDistinct keyIndices)
    (hkeyBound : ∀ i, (h : i < keyIndices.length) → keyIndices[i] < tableSize)
    (hc : displaceCandidatesOk slots tableSize occupied) :
    (placeFromCandidates keyIndices slots).wellSized tableSize := by
  obtain ⟨hsdist, _, hsbound, _⟩ := hc
  refine ⟨hlen, hsdist, hk, ?_, hkeyBound⟩
  intro i hi
  have hsmem : slots[i] ∈ slots := List.getElem_mem hi
  exact hsbound slots[i] hsmem

/-- Runtime CHD verifies the stored string after slot lookup; Lean models
injective placement + merge of bucket places + hash-seeded slot candidates. -/
def runtime_string_verify_obligation : Prop := True

/-- Finding a displace `d` (search loop) remains an obligation (Rust + tests).

Wave 24 shrinks—not discharges—this marker: Lean proves
`findDisplace_returns_zero_of_candidatesOk_at_zero` (gate already true at `d = 0`)
and executable 2-key / nonempty-occupied smokes. A full hash-family completeness
theorem is still not claimed. Once a displace is found,
`wellSized_of_displaceCandidatesOk` / `wellSized_of_findDisplace_success` apply. -/
def chd_displace_search_obligation : Prop := True

end ModelAssetGuard.Token.CHD
