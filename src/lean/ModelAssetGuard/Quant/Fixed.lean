/-
Fixed-point / integer quantization bounds proveable in Lean 4 without Mathlib.

Runtime Float paths in `Core.lean` use obligation markers (not axioms); these
theorems are the honest half-ULP core on `Nat`/`Int`. Wave 28 aborted the
Mathlib / IEEE Float bridge (see `docs/float-obligation.md`); markers stay.
-/

namespace ModelAssetGuard.Quant.Fixed

/-- Nearest-integer rounding of the nonnegative rational `n / d` (ties round up). -/
def roundDivNat (n d : Nat) (_hd : d > 0) : Nat :=
  (2 * n + d) / (2 * d)

/-- `|r - d|` as Nat. -/
def absDiffNat (r d : Nat) : Nat :=
  if r ≥ d then r - d else d - r

theorem absDiffNat_eq_natAbs (r d : Nat) :
    absDiffNat r d = Int.natAbs ((r : Int) - (d : Int)) := by
  unfold absDiffNat
  by_cases h : r ≥ d
  · rw [if_pos h]
    have : (r : Int) - (d : Int) = ((r - d : Nat) : Int) := (Int.ofNat_sub h).symm
    rw [this, Int.natAbs_natCast]
  · rw [if_neg h]
    have hlt : r < d := Nat.lt_of_not_ge h
    have hle : r ≤ d := Nat.le_of_lt hlt
    have hsub : (d : Int) - (r : Int) = ((d - r : Nat) : Int) := (Int.ofNat_sub hle).symm
    have hneg : (r : Int) - (d : Int) = - ((d : Int) - (r : Int)) := by
      rw [Int.sub_eq_add_neg, Int.sub_eq_add_neg, Int.neg_add, Int.neg_neg, Int.add_comm]
    rw [hneg, Int.natAbs_neg, hsub, Int.natAbs_natCast]

theorem absDiffNat_le_of_lt_two_mul (r d : Nat) (hr : r < 2 * d) :
    absDiffNat r d ≤ d := by
  unfold absDiffNat
  by_cases h : r ≥ d
  · rw [if_pos h]
    have hle : r ≤ 2 * d := Nat.le_of_lt hr
    have hle' : r ≤ d + d := by
      rw [Nat.two_mul] at hle
      exact hle
    exact Nat.sub_le_iff_le_add.mpr hle'
  · rw [if_neg h]
    exact Nat.sub_le d r

private theorem two_mul_sub_eq
    (n q d r : Nat) (h : 2 * n + d = 2 * d * q + r) :
    (2 : Int) * ((n : Int) - (q : Int) * (d : Int)) = (r : Int) - (d : Int) := by
  have hI :
      (2 : Int) * (n : Int) + (d : Int) =
        (2 : Int) * (d : Int) * (q : Int) + (r : Int) := by
    have := congrArg (fun x : Nat => (x : Int)) h
    simpa [Int.natCast_add, Int.natCast_mul] using this
  -- Goal equivalent to 2n - 2qd = r - d
  apply Eq.symm
  -- r - d = 2n + d - d - 2dq = 2n - 2dq  from hI
  calc
    (r : Int) - (d : Int)
        = (2 : Int) * (d : Int) * (q : Int) + (r : Int) - (d : Int) -
            (2 : Int) * (d : Int) * (q : Int) := by
          simp [Int.add_sub_cancel, Int.sub_eq_add_neg, Int.add_assoc, Int.add_left_comm,
            Int.add_comm, Int.add_left_neg, Int.add_right_neg, Int.zero_add, Int.add_zero]
    _ = (2 : Int) * (n : Int) + (d : Int) - (d : Int) -
            (2 : Int) * (d : Int) * (q : Int) := by
          rw [← hI]
    _ = (2 : Int) * (n : Int) - (2 : Int) * (d : Int) * (q : Int) := by
          simp [Int.add_sub_cancel]
    _ = (2 : Int) * (n : Int) - (2 : Int) * ((q : Int) * (d : Int)) := by
          rw [show (2 : Int) * (d : Int) * (q : Int) = (2 : Int) * ((q : Int) * (d : Int)) by
            rw [Int.mul_assoc, Int.mul_comm (d : Int) (q : Int), ← Int.mul_assoc]]
    _ = (2 : Int) * ((n : Int) - (q : Int) * (d : Int)) := by
          rw [← Int.mul_sub]

/-- Half-ULP bound: `2 * |n - round(n/d) * d| ≤ d`. -/
theorem roundDivNat_error (n d : Nat) (hd : d > 0) :
    2 * Int.natAbs ((n : Int) - (roundDivNat n d hd : Int) * (d : Int)) ≤ d := by
  let q := roundDivNat n d hd
  let m := 2 * n + d
  let denom := 2 * d
  have hdivmod : denom * (m / denom) + m % denom = m := Nat.div_add_mod m denom
  have hdenpos : 0 < denom := Nat.mul_pos (by decide : 0 < 2) hd
  have hrlt : m % denom < denom := Nat.mod_lt m hdenpos
  have hq : q = m / denom := rfl
  have hNat : 2 * n + d = 2 * d * q + m % denom := by
    have h1 : denom * q + m % denom = m := by
      simpa [hq] using hdivmod
    -- unfold lets: m = 2*n+d, denom = 2*d
    have h2 : (2 * d) * q + m % (2 * d) = 2 * n + d := by
      simpa [denom, m] using h1
    -- rewrite RHS→LHS form expected by two_mul_sub_eq
    have h3 := h2.symm
    -- normalize  (2*d)*q  to  2*d*q
    simpa [Nat.mul_assoc] using h3
  have hDiff := two_mul_sub_eq n q d (m % denom) hNat
  have hmul :
      Int.natAbs ((2 : Int) * ((n : Int) - (q : Int) * (d : Int))) =
        2 * Int.natAbs ((n : Int) - (q : Int) * (d : Int)) := by
    rw [Int.natAbs_mul]
    rfl
  have hAbs :
      2 * Int.natAbs ((n : Int) - (q : Int) * (d : Int)) =
        absDiffNat (m % denom) d := by
    have h1 := congrArg Int.natAbs hDiff
    -- h1: natAbs(2*(n-qd)) = natAbs(r-d)
    have h2 :
        2 * Int.natAbs ((n : Int) - (q : Int) * (d : Int)) =
          Int.natAbs (((m % denom : Nat) : Int) - (d : Int)) := by
      rw [← hmul, h1]
    rw [h2, absDiffNat_eq_natAbs]
  have hLe : absDiffNat (m % denom) d ≤ d :=
    absDiffNat_le_of_lt_two_mul (m % denom) d (by
      change m % denom < 2 * d
      simpa [denom] using hrlt)
  exact hAbs ▸ hLe

/-- Signed rounding of `n / d` (unsigned round on `|n|`, restore sign). -/
def roundDivInt (n : Int) (d : Nat) (hd : d > 0) : Int :=
  let q := roundDivNat (Int.natAbs n) d hd
  if n ≥ 0 then (q : Int) else - (q : Int)

theorem roundDivInt_error (n : Int) (d : Nat) (hd : d > 0) :
    2 * Int.natAbs (n - roundDivInt n d hd * (d : Int)) ≤ d := by
  unfold roundDivInt
  by_cases hpos : n ≥ 0
  · rw [if_pos hpos]
    have hn : n = (Int.natAbs n : Int) := Int.eq_natAbs_of_nonneg hpos
    rw [hn]
    simpa using roundDivNat_error (Int.natAbs n) d hd
  · rw [if_neg hpos]
    let a := Int.natAbs n
    have hn : n = - (a : Int) :=
      match Int.natAbs_eq n with
      | Or.inl hEq => False.elim (hpos (hEq ▸ Int.natCast_nonneg _))
      | Or.inr hEq => by simpa [a] using hEq
    have h := roundDivNat_error a d hd
    let q := roundDivNat a d hd
    have hparse :
        n - (-(q : Int)) * (d : Int) = n + (q : Int) * (d : Int) := by
      have : (-(q : Int)) * (d : Int) = - ((q : Int) * (d : Int)) := by
        rw [Int.neg_mul]
      rw [this, Int.sub_neg]
    have hgoal :
        n + (q : Int) * (d : Int) =
          - ((a : Int) - (q : Int) * (d : Int)) := by
      rw [hn]
      rw [Int.sub_eq_add_neg, Int.neg_add, Int.neg_neg, Int.add_comm]
    rw [hparse, hgoal, Int.natAbs_neg]
    exact h

/-- Dot product of equal-length integer lists. -/
def dot (es xs : List Int) : Int :=
  match es, xs with
  | [], [] => 0
  | e :: es, x :: xs => e * x + dot es xs
  | _, _ => 0

/-- L1 norm of an integer list. -/
def l1Norm : List Int → Nat
  | [] => 0
  | x :: xs => Int.natAbs x + l1Norm xs

/-- Cleared-denominator triangle bound for a row·vector product.
If every entry satisfies `2*|e| ≤ scale`, then
`2 * |∑ eᵢ xᵢ| ≤ scale * ∑ |xᵢ|`. -/
theorem dot_abs_bound_of_entrywise
    (es xs : List Int) (scale : Nat) (_hs : scale > 0)
    (hlen : es.length = xs.length)
    (hbound : ∀ e ∈ es, 2 * Int.natAbs e ≤ scale) :
    2 * Int.natAbs (dot es xs) ≤ scale * l1Norm xs := by
  induction es generalizing xs with
  | nil =>
      cases xs with
      | nil => simp [dot, l1Norm]
      | cons _ _ => cases hlen
  | cons e es ih =>
      cases xs with
      | nil => cases hlen
      | cons x xs =>
          simp only [List.length_cons] at hlen
          have hlen' : es.length = xs.length := Nat.succ.inj hlen
          have he : 2 * Int.natAbs e ≤ scale :=
            hbound e (List.mem_cons.mpr (Or.inl rfl))
          have hes : ∀ e' ∈ es, 2 * Int.natAbs e' ≤ scale := fun e' he' =>
            hbound e' (List.mem_cons.mpr (Or.inr he'))
          have ih := ih xs hlen' hes
          simp only [dot, l1Norm]
          have htri :
              Int.natAbs (e * x + dot es xs) ≤
                Int.natAbs (e * x) + Int.natAbs (dot es xs) :=
            Int.natAbs_add_le _ _
          have hprod : 2 * Int.natAbs (e * x) ≤ scale * Int.natAbs x := by
            have hm : Int.natAbs (e * x) = Int.natAbs e * Int.natAbs x :=
              Int.natAbs_mul e x
            calc
              2 * Int.natAbs (e * x)
                  = 2 * (Int.natAbs e * Int.natAbs x) := by rw [hm]
              _ = (2 * Int.natAbs e) * Int.natAbs x := by rw [Nat.mul_assoc]
              _ ≤ scale * Int.natAbs x := Nat.mul_le_mul_right _ he
          have hsum :
              2 * Int.natAbs (e * x) + 2 * Int.natAbs (dot es xs) ≤
                scale * Int.natAbs x + scale * l1Norm xs :=
            Nat.add_le_add hprod ih
          have hleft :
              2 * Int.natAbs (e * x + dot es xs) ≤
                2 * Int.natAbs (e * x) + 2 * Int.natAbs (dot es xs) := by
            calc
              2 * Int.natAbs (e * x + dot es xs)
                  ≤ 2 * (Int.natAbs (e * x) + Int.natAbs (dot es xs)) :=
                    Nat.mul_le_mul_left 2 htri
              _ = 2 * Int.natAbs (e * x) + 2 * Int.natAbs (dot es xs) := by
                    rw [Nat.mul_add]
          have hright :
              scale * Int.natAbs x + scale * l1Norm xs =
                scale * (Int.natAbs x + l1Norm xs) := by
            rw [Nat.mul_add]
          exact Nat.le_trans hleft (by
            rw [← hright]
            exact hsum)

/-- Half-ULP restated for the Float bridge narrative: the integer error is at
most half a denominator unit (`2 * |err| ≤ d` ⇔ `|err| ≤ d/2` when thinking in
rationals `n/d`). Replaces inventing a Mathlib Float theorem. -/
theorem roundDivNat_error_half_unit (n d : Nat) (hd : d > 0) :
    2 * Int.natAbs ((n : Int) - (roundDivNat n d hd : Int) * (d : Int)) ≤ d :=
  roundDivNat_error n d hd

/-- Same statement for signed `roundDivInt`. -/
theorem roundDivInt_error_half_unit (n : Int) (d : Nat) (hd : d > 0) :
    2 * Int.natAbs (n - roundDivInt n d hd * (d : Int)) ≤ d :=
  roundDivInt_error n d hd

/-- Every row that meets the entrywise scale hypothesis is L1-bounded against `xs`. -/
theorem rows_dot_abs_bound
    (rows : List (List Int)) (xs : List Int) (scale : Nat) (hs : scale > 0)
    (hrows : ∀ es ∈ rows,
        es.length = xs.length ∧ ∀ e ∈ es, 2 * Int.natAbs e ≤ scale) :
    ∀ es ∈ rows, 2 * Int.natAbs (dot es xs) ≤ scale * l1Norm xs := by
  intro es hes
  have h := hrows es hes
  exact dot_abs_bound_of_entrywise es xs scale hs h.1 h.2

end ModelAssetGuard.Quant.Fixed
