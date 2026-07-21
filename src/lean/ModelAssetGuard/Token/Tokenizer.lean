import Init.Data.String.Basic
import Init.Data.Char.Lemmas
import Init.Data.List.Basic
import Init.Data.List.Lemmas
import Init.Data.List.Find
import Init.Data.List.Pairwise

namespace ModelAssetGuard.Token

/-- Token type -/
abbrev Token := Nat

/-- Tokenizer interface -/
class Tokenizer (α : Type) where
  encode : α → String → List Token
  decode : α → List Token → String
  vocab_size : α → Nat

/-- BPE (Byte Pair Encoding) tokenizer -/
structure BPETokenizer where
  vocab : List (String × Token)
  merges : List (String × String × String)
  deriving Repr

/-- SentencePiece tokenizer -/
structure SentencePieceTokenizer where
  vocab : List (String × Token)
  model_proto : List UInt8
  deriving Repr

/-- Hash-vocab tokenizer (runtime: CHD MPH by default; Lean model uses assoc list).
Legacy open-address JSON still loads in Rust; see `Token.CHD` for MPH placement lemmas. -/
structure PerfectHashTokenizer where
  vocab : List (String × Token)
  hash_table : List Nat
  deriving Repr

/-- Determinism property: encode ∘ decode = id on token lists -/
def is_deterministic {α : Type} [Tokenizer α] (t : α) : Prop :=
  ∀ tokens : List Token,
    Tokenizer.encode t (Tokenizer.decode t tokens) = tokens

/-- Surjectivity property for UTF-8 strings -/
def is_surjective_utf8 {α : Type} [Tokenizer α] (t : α) : Prop :=
  ∀ s : String,
    ∃ tokens : List Token, Tokenizer.decode t tokens = s

/-- String round-trip: decode ∘ encode = id -/
def is_string_roundtrip {α : Type} [Tokenizer α] (t : α) : Prop :=
  ∀ s : String, Tokenizer.decode t (Tokenizer.encode t s) = s

/-- Decode one token to a character (shared by BPE/SP stubs). -/
def decode_token_char (vocab : List (String × Token)) (token : Token) : Char :=
  match vocab.find? (fun (_str, tok) => tok == token) with
  | some (str, _) => str.toList.headD (Char.ofNat token)
  | none => Char.ofNat token

/-- BPE encode: character codes (merges not applied in this stub). -/
def bpe_encode (_t : BPETokenizer) (text : String) : List Token :=
  text.toList.map (fun c => c.toNat)

/-- BPE decode: vocab lookup head char, else `Char.ofNat`. -/
def bpe_decode (t : BPETokenizer) (tokens : List Token) : String :=
  String.mk (tokens.map (decode_token_char t.vocab))

/-- SentencePiece encode (char-level stub). -/
def sp_encode (_t : SentencePieceTokenizer) (text : String) : List Token :=
  text.toList.map (fun c => c.toNat)

/-- SentencePiece decode (char-level stub). -/
def sp_decode (t : SentencePieceTokenizer) (tokens : List Token) : String :=
  String.mk (tokens.map (decode_token_char t.vocab))

/-- Hash-vocab encode: whole-string lookup. -/
def perfect_hash_encode (t : PerfectHashTokenizer) (text : String) : List Token :=
  match t.vocab.find? (fun (str, _) => str == text) with
  | some (_, tok) => [tok]
  | none => [0]

/-- Hash-vocab decode. -/
def perfect_hash_decode (t : PerfectHashTokenizer) (tokens : List Token) : String :=
  let words := tokens.map (fun token =>
    match t.vocab.find? (fun (_, tok) => tok == token) with
    | some (str, _) => str
    | none => "<unk>"
  )
  String.intercalate " " words

instance : Tokenizer BPETokenizer where
  encode := fun t text => bpe_encode t text
  decode := fun t tokens => bpe_decode t tokens
  vocab_size := fun t => t.vocab.length

instance : Tokenizer SentencePieceTokenizer where
  encode := fun t text => sp_encode t text
  decode := fun t tokens => sp_decode t tokens
  vocab_size := fun t => t.vocab.length

instance : Tokenizer PerfectHashTokenizer where
  encode := fun t text => perfect_hash_encode t text
  decode := fun t tokens => perfect_hash_decode t tokens
  vocab_size := fun t => t.vocab.length

/-- Vocab decode is faithful when every token decodes as `Char.ofNat`.
Empty vocab is faithful; nonempty single-char identity vocabs also qualify. -/
def vocab_decode_faithful (vocab : List (String × Token)) : Prop :=
  ∀ tok : Token, decode_token_char vocab tok = Char.ofNat tok

private theorem decode_chars_of_faithful
    (vocab : List (String × Token)) (hfaith : vocab_decode_faithful vocab)
    (tokens : List Token) :
    tokens.map (decode_token_char vocab) = tokens.map Char.ofNat := by
  induction tokens with
  | nil => rfl
  | cons t ts ih =>
      simp only [List.map_cons, hfaith t, ih]

private theorem chars_ofNat_toNat (cs : List Char) :
    (cs.map Char.toNat).map Char.ofNat = cs := by
  induction cs with
  | nil => rfl
  | cons c cs ih => simp [Char.ofNat_toNat, ih]

/-- Char-level BPE: decode∘encode = id when vocab decode is faithful (merges unused). -/
theorem bpe_string_roundtrip_of_faithful_vocab
    (t : BPETokenizer) (hfaith : vocab_decode_faithful t.vocab) :
    is_string_roundtrip t := by
  intro s
  simp only [Tokenizer.decode, Tokenizer.encode, bpe_decode, bpe_encode]
  have hmap := decode_chars_of_faithful t.vocab hfaith (s.toList.map Char.toNat)
  rw [hmap, chars_ofNat_toNat]
  exact String.ext rfl

/-- Empty vocab is decode-faithful. -/
theorem vocab_decode_faithful_empty :
    vocab_decode_faithful ([] : List (String × Token)) := by
  intro tok
  rfl

/-- Nonempty vocabs whose every entry is the single char `Char.ofNat tok` are decode-faithful. -/
theorem vocab_decode_faithful_of_forall_single_char
    (vocab : List (String × Token))
    (h : ∀ p ∈ vocab, p.1.toList = [Char.ofNat p.2]) :
    vocab_decode_faithful vocab := by
  intro tok
  match vocab with
  | [] => rfl
  | p :: ps =>
      have hp : p.1.toList = [Char.ofNat p.2] :=
        h p (List.mem_cons.mpr (Or.inl rfl))
      have hps : ∀ q ∈ ps, q.1.toList = [Char.ofNat q.2] := fun q hq =>
        h q (List.mem_cons.mpr (Or.inr hq))
      have ihtail : vocab_decode_faithful ps :=
        vocab_decode_faithful_of_forall_single_char ps hps
      show decode_token_char (p :: ps) tok = Char.ofNat tok
      unfold decode_token_char
      -- `find?` on cons: match on head predicate, else recurse.
      cases hbeq : (p.2 == tok) with
      | true =>
          have htok : p.2 = tok := by simpa [beq_iff_eq] using hbeq
          simp only [List.find?, hbeq]
          have : p.1.toList.headD (Char.ofNat tok) = Char.ofNat tok := by
            rw [hp, htok]
            rfl
          exact this
      | false =>
          simp only [List.find?, hbeq]
          simpa [decode_token_char] using ihtail tok

/-- Singleton single-char identity vocab is decode-faithful. -/
theorem vocab_decode_faithful_singleton (c : Char) :
    vocab_decode_faithful [(String.mk [c], c.toNat)] := by
  apply vocab_decode_faithful_of_forall_single_char
  intro p hp
  simp only [List.mem_singleton] at hp
  cases hp
  simp [Char.ofNat_toNat]

/-- Char-level BPE empty-vocab round-trip (corollary). -/
theorem bpe_char_level_string_roundtrip
    (t : BPETokenizer) (hempty : t.vocab = []) :
    is_string_roundtrip t := by
  have hfaith : vocab_decode_faithful t.vocab := by
    rw [hempty]
    exact vocab_decode_faithful_empty
  exact bpe_string_roundtrip_of_faithful_vocab t hfaith

/-- Nonempty single-char identity vocab ⇒ BPE string round-trip. -/
theorem bpe_string_roundtrip_of_single_char_vocab
    (t : BPETokenizer)
    (h : ∀ p ∈ t.vocab, p.1.toList = [Char.ofNat p.2]) :
    is_string_roundtrip t :=
  bpe_string_roundtrip_of_faithful_vocab t
    (vocab_decode_faithful_of_forall_single_char t.vocab h)

/-- Char-level SentencePiece surjectivity when vocab is empty. -/
theorem sp_surjective_of_empty_vocab
    (t : SentencePieceTokenizer) (hempty : t.vocab = []) :
    is_surjective_utf8 t := by
  intro s
  refine ⟨s.toList.map Char.toNat, ?_⟩
  simp only [Tokenizer.decode, sp_decode]
  have hfaith : vocab_decode_faithful t.vocab := by
    rw [hempty]
    exact vocab_decode_faithful_empty
  have hmap := decode_chars_of_faithful t.vocab hfaith (s.toList.map Char.toNat)
  rw [hmap, chars_ofNat_toNat]
  exact String.ext rfl

/-- Char-level SP: decode∘encode = id under decode-faithful vocab. -/
theorem sp_string_roundtrip_of_faithful_vocab
    (t : SentencePieceTokenizer) (hfaith : vocab_decode_faithful t.vocab) :
    is_string_roundtrip t := by
  intro s
  simp only [Tokenizer.decode, Tokenizer.encode, sp_decode, sp_encode]
  have hmap := decode_chars_of_faithful t.vocab hfaith (s.toList.map Char.toNat)
  rw [hmap, chars_ofNat_toNat]
  exact String.ext rfl

/-- Char-level SP string round-trip when vocab is empty (corollary). -/
theorem sp_char_level_string_roundtrip
    (t : SentencePieceTokenizer) (hempty : t.vocab = []) :
    is_string_roundtrip t := by
  have hfaith : vocab_decode_faithful t.vocab := by
    rw [hempty]
    exact vocab_decode_faithful_empty
  exact sp_string_roundtrip_of_faithful_vocab t hfaith

/-- One BPE merge step: replace the first adjacent pair `(a,b)` with `m`. -/
def apply_merge_once : List Token → Token → Token → Token → List Token
  | [], _, _, _ => []
  | [x], _, _, _ => [x]
  | x :: y :: rest, a, b, m =>
      if x == a && y == b then m :: rest
      else x :: apply_merge_once (y :: rest) a b m

/-- A merge step never lengthens the token list. -/
theorem apply_merge_once_length_le (tokens : List Token) (a b m : Token) :
    (apply_merge_once tokens a b m).length ≤ tokens.length := by
  induction tokens with
  | nil => simp [apply_merge_once]
  | cons x xs ih =>
      cases xs with
      | nil => simp [apply_merge_once]
      | cons y rest =>
          simp only [apply_merge_once]
          by_cases h : (x == a && y == b) = true
          · rw [if_pos h]
            -- m::rest vs x::y::rest
            simp only [List.length_cons]
            exact Nat.le_succ _
          · rw [if_neg h]
            simp only [List.length_cons]
            exact Nat.succ_le_succ ih

/-- Pair `(a,b)` occurs as adjacent tokens (propositional equality). -/
def adjacent_pair_occurs : List Token → Token → Token → Prop
  | [], _, _ => False
  | [_], _, _ => False
  | x :: y :: rest, a, b => (x = a ∧ y = b) ∨ adjacent_pair_occurs (y :: rest) a b

/-- When the merge pair occurs, the token list strictly shortens. -/
theorem apply_merge_once_length_lt_of_occurs
    (tokens : List Token) (a b m : Token)
    (hocc : adjacent_pair_occurs tokens a b) :
    (apply_merge_once tokens a b m).length < tokens.length := by
  induction tokens with
  | nil => exact False.elim hocc
  | cons x xs ih =>
      cases xs with
      | nil => exact False.elim hocc
      | cons y rest =>
          simp only [adjacent_pair_occurs, apply_merge_once] at hocc ⊢
          cases hocc with
          | inl heq =>
              have hx : x == a := by simp [heq.1, beq_iff_eq]
              have hy : y == b := by simp [heq.2, beq_iff_eq]
              have h : (x == a && y == b) = true := by simp [hx, hy]
              rw [if_pos h]
              exact Nat.lt_succ_self _
          | inr hrest =>
              by_cases h : (x == a && y == b) = true
              · rw [if_pos h]
                exact Nat.lt_succ_self _
              · rw [if_neg h]
                have hlt := ih hrest
                simp only [List.length_cons] at hlt ⊢
                exact Nat.succ_lt_succ hlt

/-- When the merge pair occurs, length decreases by exactly one (additive form). -/
theorem apply_merge_once_length_eq_pred_of_occurs'
    (tokens : List Token) (a b m : Token)
    (hocc : adjacent_pair_occurs tokens a b) :
    (apply_merge_once tokens a b m).length + 1 = tokens.length := by
  induction tokens with
  | nil => exact False.elim hocc
  | cons x xs ih =>
      cases xs with
      | nil => exact False.elim hocc
      | cons y rest =>
          simp only [adjacent_pair_occurs, apply_merge_once] at hocc ⊢
          cases hocc with
          | inl heq =>
              have hx : x == a := by simp [heq.1, beq_iff_eq]
              have hy : y == b := by simp [heq.2, beq_iff_eq]
              have h : (x == a && y == b) = true := by simp [hx, hy]
              rw [if_pos h]
              simp [List.length_cons]
          | inr hrest =>
              by_cases h : (x == a && y == b) = true
              · rw [if_pos h]
                simp [List.length_cons]
              · rw [if_neg h]
                have ih' := ih hrest
                simp only [List.length_cons] at ih' ⊢
                omega

/-- When the merge pair occurs, length decreases by exactly one. -/
theorem apply_merge_once_length_eq_pred_of_occurs
    (tokens : List Token) (a b m : Token)
    (hocc : adjacent_pair_occurs tokens a b) :
    (apply_merge_once tokens a b m).length = tokens.length - 1 := by
  have h := apply_merge_once_length_eq_pred_of_occurs' tokens a b m hocc
  have hpos : 0 < tokens.length := by
    cases tokens with
    | nil => exact False.elim hocc
    | cons x xs =>
        cases xs with
        | nil => exact False.elim hocc
        | cons _ _ => simp [List.length_cons]
  omega

/-- Apply a token-level merge schedule left-to-right (structural multi-merge). -/
def apply_merge_table (tokens : List Token) (merges : List (Token × Token × Token)) : List Token :=
  match merges with
  | [] => tokens
  | (a, b, m) :: rest => apply_merge_table (apply_merge_once tokens a b m) rest

/-- Empty merge schedule is identity. -/
theorem apply_merge_table_nil (tokens : List Token) :
    apply_merge_table tokens [] = tokens :=
  rfl

/-- Multi-merge never lengthens the token list. -/
theorem apply_merge_table_length_le
    (tokens : List Token) (merges : List (Token × Token × Token)) :
    (apply_merge_table tokens merges).length ≤ tokens.length := by
  induction merges generalizing tokens with
  | nil => simp [apply_merge_table]
  | cons head rest ih =>
      simp only [apply_merge_table]
      have honce :=
        apply_merge_once_length_le tokens head.1 head.2.1 head.2.2
      have hrest := ih (apply_merge_once tokens head.1 head.2.1 head.2.2)
      exact Nat.le_trans hrest honce

/-- Char-level encode then apply a token merge schedule. -/
def bpe_encode_with_merge_table (text : String) (merges : List (Token × Token × Token)) : List Token :=
  apply_merge_table (text.toList.map Char.toNat) merges

/-- Encode-with-empty-table agrees with char-level encode. -/
theorem bpe_encode_with_merge_table_nil (text : String) :
    bpe_encode_with_merge_table text [] = text.toList.map Char.toNat := by
  simp [bpe_encode_with_merge_table, apply_merge_table]

/-- Encode-with-merge-table length is at most the character count. -/
theorem bpe_encode_with_merge_table_length_le
    (text : String) (merges : List (Token × Token × Token)) :
    (bpe_encode_with_merge_table text merges).length ≤ text.toList.length := by
  simpa [bpe_encode_with_merge_table, List.length_map] using
    apply_merge_table_length_le (text.toList.map Char.toNat) merges

/-- A firing first merge on the char stream leaves room of at most `chars - 1`. -/
theorem bpe_encode_with_merge_table_cons_length_of_occurs
    (text : String) (a b m : Token) (rest : List (Token × Token × Token))
    (hocc : adjacent_pair_occurs (text.toList.map Char.toNat) a b) :
    (bpe_encode_with_merge_table text ((a, b, m) :: rest)).length ≤
      text.toList.length - 1 := by
  simp only [bpe_encode_with_merge_table, apply_merge_table]
  have hstep :=
    apply_merge_once_length_eq_pred_of_occurs' (text.toList.map Char.toNat) a b m hocc
  have hrest :=
    apply_merge_table_length_le
      (apply_merge_once (text.toList.map Char.toNat) a b m) rest
  have hlen :
      (apply_merge_once (text.toList.map Char.toNat) a b m).length =
        text.toList.length - 1 := by
    have hmap : (text.toList.map Char.toNat).length = text.toList.length := by
      simp [List.length_map]
    omega
  omega

/-- When the merge pair does not occur, one step is the identity. -/
theorem apply_merge_once_eq_of_not_occurs
    (tokens : List Token) (a b m : Token)
    (h : ¬ adjacent_pair_occurs tokens a b) :
    apply_merge_once tokens a b m = tokens := by
  induction tokens with
  | nil => rfl
  | cons x xs ih =>
      cases xs with
      | nil => rfl
      | cons y rest =>
          simp only [adjacent_pair_occurs, apply_merge_once, not_or] at h ⊢
          have hxneq : ¬ (x = a ∧ y = b) := h.1
          have hrest : ¬ adjacent_pair_occurs (y :: rest) a b := h.2
          have hbeq : (x == a && y == b) = false := by
            by_cases hx : x = a
            · by_cases hy : y = b
              · exact False.elim (hxneq ⟨hx, hy⟩)
              · simp [hx, hy, beq_iff_eq]
            · simp [hx, beq_iff_eq]
          rw [if_neg (by simp [hbeq])]
          simp only [List.cons.injEq, true_and]
          exact ih hrest

/-- Absent merge pair ⇒ length unchanged (corollary of identity). -/
theorem apply_merge_once_length_eq_of_not_occurs
    (tokens : List Token) (a b m : Token)
    (h : ¬ adjacent_pair_occurs tokens a b) :
    (apply_merge_once tokens a b m).length = tokens.length := by
  rw [apply_merge_once_eq_of_not_occurs tokens a b m h]

/-- Cons form of multi-merge is one step then the remainder (definitional). -/
theorem apply_merge_table_cons
    (tokens : List Token) (a b m : Token) (rest : List (Token × Token × Token)) :
    apply_merge_table tokens ((a, b, m) :: rest) =
      apply_merge_table (apply_merge_once tokens a b m) rest :=
  rfl

/-- Merge schedules compose under list append. -/
theorem apply_merge_table_append
    (tokens : List Token) (m₁ m₂ : List (Token × Token × Token)) :
    apply_merge_table tokens (m₁ ++ m₂) =
      apply_merge_table (apply_merge_table tokens m₁) m₂ := by
  induction m₁ generalizing tokens with
  | nil => simp [apply_merge_table]
  | cons head rest ih =>
      cases head with
      | mk a bm =>
        cases bm with
        | mk b m =>
            simp only [List.cons_append, apply_merge_table]
            exact ih (apply_merge_once tokens a b m)

/-- Encode-with-merge-table schedules compose under append (runtime twin of table append). -/
theorem bpe_encode_with_merge_table_append
    (text : String) (m₁ m₂ : List (Token × Token × Token)) :
    bpe_encode_with_merge_table text (m₁ ++ m₂) =
      apply_merge_table (bpe_encode_with_merge_table text m₁) m₂ := by
  simp only [bpe_encode_with_merge_table]
  exact apply_merge_table_append (text.toList.map Char.toNat) m₁ m₂

/-- Empty merge schedule leaves encode length equal to character count. -/
theorem bpe_encode_with_merge_table_nil_length (text : String) :
    (bpe_encode_with_merge_table text []).length = text.toList.length := by
  simp [bpe_encode_with_merge_table, apply_merge_table, List.length_map]

/-- Correct vocab token-injectivity (distinct keys ⇒ distinct token ids when both found). -/
def is_vocab_token_injective (t : PerfectHashTokenizer) : Prop :=
  ∀ (s₁ s₂ : String) (tok₁ tok₂ : Token),
    t.vocab.find? (fun (str, _) => str == s₁) = some (s₁, tok₁) →
    t.vocab.find? (fun (str, _) => str == s₂) = some (s₂, tok₂) →
    s₁ ≠ s₂ → tok₁ ≠ tok₂

/-- Distinct keys in an assoc list carry distinct mapped tokens. -/
def vocab_keys_token_unique (vocab : List (String × Token)) : Prop :=
  ∀ s₁ s₂ t₁ t₂,
    (s₁, t₁) ∈ vocab → (s₂, t₂) ∈ vocab → s₁ ≠ s₂ → t₁ ≠ t₂

/-- Structural injectivity from unique key→token association (proven; not an axiom). -/
theorem perfect_hash_injective_of_unique
    (t : PerfectHashTokenizer)
    (huniq : vocab_keys_token_unique t.vocab) :
    is_vocab_token_injective t := by
  intro s₁ s₂ tok₁ tok₂ h1 h2 hs
  have mem1 : (s₁, tok₁) ∈ t.vocab := List.mem_of_find?_eq_some h1
  have mem2 : (s₂, tok₂) ∈ t.vocab := List.mem_of_find?_eq_some h2
  exact huniq s₁ s₂ tok₁ tok₂ mem1 mem2 hs

/-- Encode recovers the token when key `find?` succeeds. -/
theorem perfect_hash_encode_eq_of_find
    (t : PerfectHashTokenizer) (s : String) (tok : Token)
    (h : t.vocab.find? (fun (str, _) => str == s) = some (s, tok)) :
    perfect_hash_encode t s = [tok] := by
  simp [perfect_hash_encode, h]

/-- Distinct tokens map to at most one key (decode direction). -/
def vocab_token_keys_unique (vocab : List (String × Token)) : Prop :=
  ∀ s₁ s₂ t₁ t₂,
    (s₁, t₁) ∈ vocab → (s₂, t₂) ∈ vocab → t₁ = t₂ → s₁ = s₂

/-- Decode of a singleton recovers the string when token `find?` succeeds. -/
theorem perfect_hash_decode_singleton_eq_of_find
    (t : PerfectHashTokenizer) (s : String) (tok : Token)
    (h : t.vocab.find? (fun (_, t') => t' == tok) = some (s, tok)) :
    perfect_hash_decode t [tok] = s := by
  simp [perfect_hash_decode, h, String.intercalate]
  rfl

/-- Under token uniqueness, a successful key lookup implies the matching token lookup. -/
theorem find_token_of_find_key_of_token_unique
    (vocab : List (String × Token))
    (huniq : vocab_token_keys_unique vocab)
    (s : String) (tok : Token)
    (h : vocab.find? (fun (str, _) => str == s) = some (s, tok)) :
    vocab.find? (fun (_, t') => t' == tok) = some (s, tok) := by
  have hdecomp := (List.find?_eq_some_iff_append).1 h
  rcases hdecomp with ⟨_, as, bs, hv, hpre⟩
  have smem : (s, tok) ∈ vocab := by
    rw [hv]
    exact List.mem_append_right _ (List.mem_cons_self)
  rw [List.find?_eq_some_iff_append]
  refine ⟨by simp [beq_iff_eq], as, bs, hv, ?_⟩
  intro a ha
  cases htok : (a.snd == tok) with
  | true =>
      have htokeq : a.snd = tok := by simpa [beq_iff_eq] using htok
      have amem : a ∈ vocab := by
        rw [hv]
        exact List.mem_append_left _ ha
      have akey : a.fst = s := by
        cases a with
        | mk s₀ t₀ =>
            change (s₀, t₀) ∈ vocab at amem
            have ht₀ : t₀ = tok := htokeq
            exact huniq s₀ s t₀ tok amem smem ht₀
      have hkey : (!(a.fst == s)) = true := hpre a ha
      simp [akey, beq_iff_eq] at hkey
  | false =>
      simp [htok]

/-- Key round-trip under unique key→token and unique token→key (assoc-list semantics).

Companion to CHD placement injectivity: Lean encode/decode is the string-verify
view after MPH slot lookup; Rust CHD supplies the O(1) placement. -/
theorem perfect_hash_key_roundtrip_of_unique
    (t : PerfectHashTokenizer)
    (_hkt : vocab_keys_token_unique t.vocab)
    (htk : vocab_token_keys_unique t.vocab)
    (s : String) (tok : Token)
    (hfind : t.vocab.find? (fun (str, _) => str == s) = some (s, tok)) :
    perfect_hash_decode t (perfect_hash_encode t s) = s := by
  rw [perfect_hash_encode_eq_of_find t s tok hfind]
  have htok := find_token_of_find_key_of_token_unique t.vocab htk s tok hfind
  exact perfect_hash_decode_singleton_eq_of_find t s tok htok

/-- Test tokenizer with strings -/
def test_tokenizer {α : Type} [Tokenizer α] (t : α) (test_strings : List String) : Bool :=
  test_strings.all fun s =>
    let tokens := Tokenizer.encode t s
    let decoded := Tokenizer.decode t tokens
    decoded == s

/-- Fuzz test tokenizer -/
def fuzz_test_tokenizer {α : Type} [Tokenizer α] (t : α) (num_tests : Nat) : IO Bool := do
  let test_strings := List.range num_tests |>.map fun _ => "test"
  return test_tokenizer t test_strings

end ModelAssetGuard.Token
