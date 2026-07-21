//! UNIGRAM lattice: Viterbi, A* n-best, Gumbel-perturbed n-best (WOR), and
//! forward-filter / backward sample.
//!
//! Mirrors official SentencePiece `unigram_model.cc` Lattice semantics closely
//! enough for cross-checks on crafted fixtures. Not a claim of bit-identical
//! floating-point. Pathological agenda growth **refuses by default**; opt-in
//! `EncodeOptions.nbest_stock_compat` matches stock shrink
//! (`min(512, nbest_size*10)`) and timeout→Viterbi fallback.

use crate::sentencepiece::{byte_piece_ids, EncodeOptions, PieceType, SentencePieceModel};
use rand::Rng;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::time::Instant;

const UNK_PENALTY: f32 = 10.0;
const MAX_NBEST: usize = 1024;
const MAX_AGENDA: usize = 10_000;
/// Official stock shrink floor (`kMinAgendaSize` in `unigram_model.cc`).
const MIN_AGENDA: usize = 512;
/// Official default n-best timeout (ms). Default mode refuses; stock-compat
/// falls back to Viterbi only when the agenda also overflows (stock policy).
const DEFAULT_NBEST_TIMEOUT_MS: u128 = 30_000;

/// Limits for A* n-best (production defaults + test overrides).
#[derive(Debug, Clone, Copy)]
struct NbestLimits {
    max_agenda: usize,
    min_agenda: usize,
    timeout_ms: u128,
    stock_compat: bool,
}

impl NbestLimits {
    fn from_opts(opts: &EncodeOptions) -> Self {
        Self {
            max_agenda: MAX_AGENDA,
            min_agenda: MIN_AGENDA,
            timeout_ms: DEFAULT_NBEST_TIMEOUT_MS,
            stock_compat: opts.nbest_stock_compat,
        }
    }
}

/// Internal A* outcome: paths, or stock-compat timeout → Viterbi fallback.
#[derive(Debug)]
enum NbestAstarOutcome {
    Paths(Vec<(Vec<usize>, f32)>),
    ViterbiFallback,
}

/// Official SP: USER_DEFINED pieces get `0.1 * (char_length - 1)` so they win Viterbi.
#[inline]
pub fn user_defined_score(char_len: usize) -> f32 {
    0.1 * (char_len.saturating_sub(1) as f32)
}

#[derive(Debug, Clone)]
struct Node {
    /// Vocab id; `u32::MAX` for BOS/EOS sentinels.
    id: u32,
    /// Start character position.
    pos: usize,
    /// Length in Unicode scalars.
    length: usize,
    score: f32,
    #[allow(dead_code)]
    node_id: usize,
    /// Best path score from BOS through this node (filled by Viterbi).
    backtrace_score: f32,
    /// Prev node id in Viterbi path (`usize::MAX` = none).
    prev: usize,
    /// True when this is an unk / byte_fallback single-char edge.
    is_unk: bool,
}

struct Lattice {
    /// Character count of the sentence.
    len: usize,
    nodes: Vec<Node>,
    begin_nodes: Vec<Vec<usize>>,
    end_nodes: Vec<Vec<usize>>,
}

impl Lattice {
    fn new(char_len: usize) -> Self {
        let mut nodes = Vec::new();
        let mut begin_nodes = vec![Vec::new(); char_len + 1];
        let mut end_nodes = vec![Vec::new(); char_len + 1];

        // BOS
        let bos_id = 0;
        nodes.push(Node {
            id: u32::MAX,
            pos: 0,
            length: 0,
            score: 0.0,
            node_id: bos_id,
            backtrace_score: 0.0,
            prev: usize::MAX,
            is_unk: false,
        });
        end_nodes[0].push(bos_id);

        // EOS
        let eos_id = 1;
        nodes.push(Node {
            id: u32::MAX,
            pos: char_len,
            length: 0,
            score: 0.0,
            node_id: eos_id,
            backtrace_score: 0.0,
            prev: usize::MAX,
            is_unk: false,
        });
        begin_nodes[char_len].push(eos_id);

        Self {
            len: char_len,
            nodes,
            begin_nodes,
            end_nodes,
        }
    }

    fn insert(&mut self, pos: usize, length: usize, id: u32, score: f32, is_unk: bool) -> usize {
        let node_id = self.nodes.len();
        self.nodes.push(Node {
            id,
            pos,
            length,
            score,
            node_id,
            backtrace_score: 0.0,
            prev: usize::MAX,
            is_unk,
        });
        self.begin_nodes[pos].push(node_id);
        self.end_nodes[pos + length].push(node_id);
        node_id
    }

    fn bos(&self) -> usize {
        0
    }

    fn eos(&self) -> usize {
        1
    }

    /// Left-to-right Viterbi; fills `backtrace_score` / `prev`.
    fn viterbi(&mut self) -> Result<(Vec<usize>, f32), String> {
        let len = self.len;
        for pos in 0..=len {
            let begin = self.begin_nodes[pos].clone();
            for &r_id in &begin {
                let mut best_score = f32::NEG_INFINITY;
                let mut best_prev = usize::MAX;
                for &l_id in &self.end_nodes[pos] {
                    let score = self.nodes[l_id].backtrace_score + self.nodes[r_id].score;
                    if score > best_score {
                        best_score = score;
                        best_prev = l_id;
                    }
                }
                if best_prev == usize::MAX {
                    return Err("UNIGRAM lattice: Viterbi failed to find a path".into());
                }
                self.nodes[r_id].prev = best_prev;
                self.nodes[r_id].backtrace_score = best_score;
            }
        }

        let eos = self.eos();
        let score = self.nodes[eos].backtrace_score;
        let mut path = Vec::new();
        let mut cur = self.nodes[eos].prev;
        while cur != self.bos() {
            if cur == usize::MAX {
                return Err("UNIGRAM lattice: corrupt Viterbi backpointer".into());
            }
            path.push(cur);
            cur = self.nodes[cur].prev;
        }
        path.reverse();
        Ok((path, score))
    }

    fn log_sum_exp(a: f32, b: f32, init: bool) -> f32 {
        if init {
            return b;
        }
        if a == f32::NEG_INFINITY {
            return b;
        }
        if b == f32::NEG_INFINITY {
            return a;
        }
        let (max_v, min_v) = if a > b { (a, b) } else { (b, a) };
        max_v + (min_v - max_v).exp().ln_1p()
    }

    fn forward(&self, inv_theta: f32) -> Vec<f32> {
        let mut alpha = vec![f32::NEG_INFINITY; self.nodes.len()];
        alpha[self.bos()] = 0.0;
        for pos in 0..=self.len {
            for &r_id in &self.begin_nodes[pos] {
                let mut init = true;
                let mut acc = f32::NEG_INFINITY;
                for &l_id in &self.end_nodes[pos] {
                    let cand = inv_theta * self.nodes[l_id].score + alpha[l_id];
                    acc = Self::log_sum_exp(acc, cand, init);
                    init = false;
                }
                alpha[r_id] = acc;
            }
        }
        alpha
    }
}

#[derive(Clone)]
struct Hyp {
    node: usize,
    next: Option<usize>, // index into hyp pool
    fx: f32,
    gx: f32,
}

#[derive(Clone)]
struct HypOrd {
    idx: usize,
    fx: f32,
}

impl PartialEq for HypOrd {
    fn eq(&self, other: &Self) -> bool {
        self.fx == other.fx
    }
}
impl Eq for HypOrd {}
impl PartialOrd for HypOrd {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for HypOrd {
    fn cmp(&self, other: &Self) -> Ordering {
        // Max-heap by fx
        self.fx.partial_cmp(&other.fx).unwrap_or(Ordering::Equal)
    }
}

/// Official SP `Gumbel()`: `-log(-log(U(0,1) + eps))`.
#[inline]
fn gumbel_noise(rng: &mut impl Rng) -> f32 {
    const EPS: f32 = 1e-7;
    let u: f32 = rng.gen::<f32>();
    -(-(u + EPS).ln()).ln()
}

/// Softplus-style numerically stable `x - softplus(x)` helper used by truncated Gumbel.
#[inline]
fn log_gumbel_truncated_adjust(top_fx: f32, perturbed: f32, max_perturbed: f32) -> f32 {
    // Official: v = top.fx - pert + log1p(-exp(pert - max))
    // adjusted = top.fx - max(0, v) - log1p(exp(-|v|))
    let v = top_fx - perturbed + (-(perturbed - max_perturbed).exp()).ln_1p();
    top_fx - v.max(0.0) - (-v.abs()).exp().ln_1p()
}

/// Log Gumbel survival `log(1 - exp(-exp(x)))` with the official series for small x.
#[inline]
fn log_gumbel_survival(x: f64) -> f64 {
    let y = x.exp();
    if x <= -10.0 {
        // Series expansion of the log Gumbel survival function up to eps.
        x - (y / 2.0) + (y.powi(2) / 24.0) - y.powi(4) / 2880.0
    } else {
        (-((-y).exp_m1())).ln()
    }
}

fn extract_path_from_hyp(pool: &[Hyp], hyp_idx: usize) -> Vec<usize> {
    let mut path = Vec::new();
    let mut cur = pool[hyp_idx].next;
    while let Some(i) = cur {
        // Chain: BOS_hyp -> n1 -> n2 -> ... -> EOS_hyp (next=None).
        if pool[i].next.is_none() {
            break;
        }
        path.push(pool[i].node);
        cur = pool[i].next;
    }
    path
}

fn nbest_astar<R: Rng>(
    lattice: &Lattice,
    nbest_size: usize,
    sample: bool,
    inv_theta: f32,
    alpha_fwd: Option<&[f32]>,
    mut rng: Option<&mut R>,
    limits: NbestLimits,
) -> Result<NbestAstarOutcome, String> {
    if nbest_size == 0 {
        return Ok(NbestAstarOutcome::Paths(Vec::new()));
    }
    if nbest_size == 1 && !sample {
        return Err("nbest_astar: use viterbi for nbest_size==1".into());
    }

    let mut pool: Vec<Hyp> = Vec::new();
    let mut agenda: BinaryHeap<HypOrd> = BinaryHeap::new();
    let start = Instant::now();

    let eos = lattice.eos();
    let eos_fx = if sample {
        let r = rng
            .as_deref_mut()
            .ok_or_else(|| "nbest_astar: sample mode requires rng".to_string())?;
        gumbel_noise(r)
    } else {
        lattice.nodes[eos].backtrace_score
    };
    pool.push(Hyp {
        node: eos,
        next: None,
        fx: eos_fx,
        gx: 0.0,
    });
    agenda.push(HypOrd {
        idx: 0,
        fx: pool[0].fx,
    });

    let mut results: Vec<(Vec<usize>, f32)> = Vec::new();

    while let Some(top) = agenda.pop() {
        let hyp_idx = top.idx;
        let node = pool[hyp_idx].node;

        if node == lattice.bos() {
            let path = extract_path_from_hyp(&pool, hyp_idx);
            results.push((path, pool[hyp_idx].fx));
            if results.len() == nbest_size {
                break;
            }
            continue;
        }

        let ends = &lattice.end_nodes[lattice.nodes[node].pos];
        if sample {
            let alpha = alpha_fwd
                .ok_or_else(|| "nbest_astar: sample mode requires forward marginals".to_string())?;
            let rng = match rng.as_deref_mut() {
                Some(r) => r,
                None => {
                    return Err("nbest_astar: sample mode requires rng".into());
                }
            };
            let z = alpha[node];
            let top_gx = pool[hyp_idx].gx;
            let top_fx = pool[hyp_idx].fx;
            let mut probs = Vec::with_capacity(ends.len());
            let mut perturbed = Vec::with_capacity(ends.len());
            let mut max_score = f32::NEG_INFINITY;
            for &l_id in ends {
                let p = top_gx + alpha[l_id] + inv_theta * lattice.nodes[l_id].score - z;
                let g = p + gumbel_noise(rng);
                if g > max_score {
                    max_score = g;
                }
                probs.push(p);
                perturbed.push(g);
            }
            for (i, &l_id) in ends.iter().enumerate() {
                let fx = log_gumbel_truncated_adjust(top_fx, perturbed[i], max_score);
                let new_idx = pool.len();
                pool.push(Hyp {
                    node: l_id,
                    next: Some(hyp_idx),
                    fx,
                    gx: probs[i],
                });
                agenda.push(HypOrd { idx: new_idx, fx });
            }
        } else {
            for &l_id in ends {
                let gx = lattice.nodes[l_id].score + pool[hyp_idx].gx;
                let fx = lattice.nodes[l_id].backtrace_score + pool[hyp_idx].gx;
                let new_idx = pool.len();
                pool.push(Hyp {
                    node: l_id,
                    next: Some(hyp_idx),
                    fx,
                    gx,
                });
                agenda.push(HypOrd { idx: new_idx, fx });
            }
        }

        // Official SP (`unigram_model.cc` Lattice::NBest): when agenda ≥ 10000,
        // either timeout→Viterbi fallback (if timeout_ms > 0) or shrink to
        // min(512, nbest_size*10). Default here refuses; stock_compat matches.
        if agenda.len() >= limits.max_agenda {
            if limits.stock_compat {
                if limits.timeout_ms > 0 && start.elapsed().as_millis() >= limits.timeout_ms {
                    return Ok(NbestAstarOutcome::ViterbiFallback);
                }
                let keep = limits.min_agenda.min(nbest_size.saturating_mul(10)).max(1);
                let mut kept: Vec<HypOrd> = Vec::with_capacity(keep);
                for _ in 0..keep {
                    if let Some(h) = agenda.pop() {
                        kept.push(h);
                    } else {
                        break;
                    }
                }
                agenda = BinaryHeap::from(kept);
            } else {
                return Err(format!(
                    "UNIGRAM nbest: agenda exceeded {} (pathological lattice); refuse rather than shrink (set nbest_stock_compat=true for stock shrink/timeout fallback)",
                    limits.max_agenda
                ));
            }
        } else if !limits.stock_compat && start.elapsed().as_millis() >= limits.timeout_ms {
            // Stricter default: refuse on wall-clock timeout even without
            // agenda overflow (stock only checks timeout inside the overflow
            // branch). Avoids silently returning a partial n-best list.
            return Err(format!(
                "UNIGRAM nbest: exceeded {}ms timeout; refuse rather than Viterbi fallback (set nbest_stock_compat=true for stock policy)",
                limits.timeout_ms
            ));
        }
    }

    Ok(NbestAstarOutcome::Paths(results))
}

fn expand_path_ids(
    model: &SentencePieceModel,
    chars: &[char],
    lattice: &Lattice,
    path: &[usize],
    opts: EncodeOptions,
) -> Result<Vec<u32>, String> {
    let mut out = Vec::new();
    for &nid in path {
        let node = &lattice.nodes[nid];
        if node.is_unk {
            let surface: String = chars[node.pos..node.pos + node.length].iter().collect();
            if model.byte_fallback {
                out.extend(byte_piece_ids(model, &surface)?);
            } else if opts.allow_silent_unk {
                out.push(model.unk_id);
            } else {
                return Err("UNIGRAM lattice: unk edge without byte_fallback/silent_unk".into());
            }
        } else {
            out.push(node.id);
        }
    }
    Ok(out)
}

fn piece_index_map(model: &SentencePieceModel) -> HashMap<&str, (u32, f32, PieceType)> {
    let mut map = HashMap::new();
    for (i, rec) in model.piece_records.iter().enumerate() {
        // Official UNIGRAM trie excludes reserved CONTROL/UNKNOWN/BYTE.
        if matches!(
            rec.piece_type,
            PieceType::Unused | PieceType::Control | PieceType::Unknown | PieceType::Byte
        ) {
            continue;
        }
        map.insert(rec.piece.as_str(), (i as u32, rec.score, rec.piece_type));
    }
    map
}

fn populate_lattice(
    model: &SentencePieceModel,
    chars: &[char],
    opts: EncodeOptions,
    forced: &HashMap<&str, ()>,
) -> Result<Lattice, String> {
    let n = chars.len();
    let mut lattice = Lattice::new(n);
    if n == 0 {
        return Ok(lattice);
    }

    let map = piece_index_map(model);
    let max_piece_chars = map
        .keys()
        .map(|k| k.chars().count())
        .max()
        .unwrap_or(0)
        .max(1);

    let min_score = map
        .values()
        .map(|(_, s, _)| *s)
        .fold(f32::INFINITY, f32::min);
    let unk_score = if min_score.is_finite() {
        min_score - UNK_PENALTY
    } else {
        -UNK_PENALTY
    };
    let allow_unk_edge = model.byte_fallback || opts.allow_silent_unk;

    for pos in 0..n {
        let mut cand = String::new();
        let max_len = max_piece_chars.min(n - pos);
        let mut has_single = false;
        for len in 1..=max_len {
            cand.push(chars[pos + len - 1]);
            if let Some(&(id, score, ty)) = map.get(cand.as_str()) {
                if ty == PieceType::Unused {
                    continue;
                }
                let edge_score =
                    if ty == PieceType::UserDefined || forced.contains_key(cand.as_str()) {
                        user_defined_score(len)
                    } else {
                        score
                    };
                lattice.insert(pos, len, id, edge_score, false);
                if len == 1 {
                    has_single = true;
                }
            }
        }
        if !has_single && allow_unk_edge {
            lattice.insert(pos, 1, model.unk_id, unk_score, true);
        }
        // else: leave gap — Viterbi / covering check will fail closed
    }

    // Ensure every position that has outgoing begin nodes can connect; if a
    // position has no edges at all, covering will fail.
    Ok(lattice)
}

fn resolve_forced<'a>(
    model: &'a SentencePieceModel,
    opts: &EncodeOptions,
) -> Result<HashMap<&'a str, ()>, String> {
    let mut forced: HashMap<&str, ()> = HashMap::new();
    for e in model.prefix_matcher.entries() {
        forced.insert(e.as_str(), ());
    }
    for p in &opts.forced_pieces {
        if p.is_empty() {
            return Err("forced_pieces: empty piece not allowed".into());
        }
        if !model.pieces.iter().any(|x| x == p) {
            return Err(format!("forced_pieces: {p:?} not in vocab (fail-closed)"));
        }
        // Lifetime: forced_pieces owned by opts; we need owned keys for map.
        // Use piece_records string from model when equal.
        if let Some(rec) = model.piece_records.iter().find(|r| r.piece == *p) {
            forced.insert(rec.piece.as_str(), ());
        }
    }
    Ok(forced)
}

/// Build effective PrefixMatcher (model USER_DEFINED ∪ runtime forced_pieces).
pub fn effective_matcher(
    model: &SentencePieceModel,
    opts: &EncodeOptions,
) -> Result<crate::sp_normalizer::PrefixMatcher, String> {
    if opts.forced_pieces.is_empty() {
        return Ok(model.prefix_matcher.clone());
    }
    for p in &opts.forced_pieces {
        if p.is_empty() {
            return Err("forced_pieces: empty piece not allowed".into());
        }
        if !model.pieces.iter().any(|x| x == p) {
            return Err(format!("forced_pieces: {p:?} not in vocab (fail-closed)"));
        }
    }
    model.prefix_matcher.with_extra(opts.forced_pieces.clone())
}

/// UNIGRAM 1-best via lattice Viterbi (USER_DEFINED-aware).
pub fn encode_unigram_lattice(
    model: &SentencePieceModel,
    normalized: &str,
    opts: EncodeOptions,
) -> Result<Vec<u32>, String> {
    let chars: Vec<char> = normalized.chars().collect();
    if chars.is_empty() {
        return Ok(Vec::new());
    }
    let forced = resolve_forced(model, &opts)?;
    let mut lattice = populate_lattice(model, &chars, opts.clone(), &forced)?;
    // Check coverage: every position must be reachable.
    let (path, _) = lattice.viterbi().map_err(|_| {
        "UNIGRAM encode: no covering segmentation (fail-closed; no unk/byte_fallback)".to_string()
    })?;
    expand_path_ids(model, &chars, &lattice, &path, opts)
}

/// Lattice A* n-best. Returns `(token_ids, path_score)` ordered best-first.
pub fn nbest_encode_unigram(
    model: &SentencePieceModel,
    normalized: &str,
    nbest_size: usize,
    opts: EncodeOptions,
) -> Result<Vec<(Vec<u32>, f32)>, String> {
    let nbest_size = nbest_size.clamp(1, MAX_NBEST);
    let chars: Vec<char> = normalized.chars().collect();
    if chars.is_empty() {
        return Ok(vec![(Vec::new(), 0.0)]);
    }
    let forced = resolve_forced(model, &opts)?;
    let mut lattice = populate_lattice(model, &chars, opts.clone(), &forced)?;
    let (best_path, best_score) = lattice.viterbi().map_err(|_| {
        "UNIGRAM nbest: no covering segmentation (fail-closed; no unk/byte_fallback)".to_string()
    })?;

    if nbest_size == 1 {
        let ids = expand_path_ids(model, &chars, &lattice, &best_path, opts)?;
        return Ok(vec![(ids, best_score)]);
    }

    // For the non-sample call site, any Rng type works; use StdRng as a dummy type param.
    let limits = NbestLimits::from_opts(&opts);
    let outcome =
        nbest_astar::<rand::rngs::StdRng>(&lattice, nbest_size, false, 0.0, None, None, limits)?;
    let raw = match outcome {
        NbestAstarOutcome::ViterbiFallback => {
            let ids = expand_path_ids(model, &chars, &lattice, &best_path, opts)?;
            return Ok(vec![(ids, best_score)]);
        }
        NbestAstarOutcome::Paths(paths) => paths,
    };
    let mut out = Vec::with_capacity(raw.len());
    for (path, score) in raw {
        let ids = expand_path_ids(model, &chars, &lattice, &path, opts.clone())?;
        out.push((ids, score));
    }
    if out.is_empty() {
        // Fallback: at least return Viterbi best.
        let ids = expand_path_ids(model, &chars, &lattice, &best_path, opts)?;
        out.push((ids, best_score));
    }
    Ok(out)
}

/// Sample one segmentation: lattice forward-filter / backward sample (`nbest_size < 0`
/// path in official SP when SampleEncodeAvailable).
pub fn sample_encode_unigram_lattice(
    model: &SentencePieceModel,
    normalized: &str,
    alpha: f32,
    opts: EncodeOptions,
    rng: &mut impl Rng,
) -> Result<Vec<u32>, String> {
    if !(alpha.is_finite() && alpha >= 0.0) {
        return Err("sample_encode: alpha must be finite and >= 0".into());
    }
    let chars: Vec<char> = normalized.chars().collect();
    if chars.is_empty() {
        return Ok(Vec::new());
    }
    let forced = resolve_forced(model, &opts)?;
    let lattice = populate_lattice(model, &chars, opts.clone(), &forced)?;
    // Ensure the lattice connects; run a coverage check via attempting Viterbi on a clone.
    {
        let mut check = populate_lattice(model, &chars, opts.clone(), &forced)?;
        let _ = check.viterbi().map_err(|_| {
            "UNIGRAM sample: no covering segmentation (fail-closed; no unk/byte_fallback)"
                .to_string()
        })?;
    }

    let inv_theta = alpha;
    let alpha_fwd = lattice.forward(inv_theta);
    let mut results = Vec::new();
    let mut z = alpha_fwd[lattice.eos()];
    let mut node = lattice.eos();
    loop {
        let ends = &lattice.end_nodes[lattice.nodes[node].pos];
        if ends.is_empty() {
            return Err("UNIGRAM sample: empty end_nodes".into());
        }
        let mut probs = Vec::with_capacity(ends.len());
        for &l_id in ends {
            let p = (alpha_fwd[l_id] + inv_theta * lattice.nodes[l_id].score - z).exp();
            probs.push(p.max(0.0));
        }
        let sum: f32 = probs.iter().sum();
        if sum <= 0.0 || !sum.is_finite() {
            return Err("UNIGRAM sample: invalid probability mass".into());
        }
        let mut r = rng.gen::<f32>() * sum;
        let mut choice = ends[0];
        for (i, &l_id) in ends.iter().enumerate() {
            r -= probs[i];
            if r <= 0.0 {
                choice = l_id;
                break;
            }
        }
        node = choice;
        if node == lattice.bos() {
            break;
        }
        z = alpha_fwd[node];
        results.push(node);
    }
    results.reverse();
    expand_path_ids(model, &chars, &lattice, &results, opts)
}

/// With-replacement SampleEncodeAndScore: draw `num_samples` lattice samples.
/// Score = `Σ α·node.score − marginal` (official SP `wor=false` path).
pub fn sample_encode_and_score_unigram(
    model: &SentencePieceModel,
    normalized: &str,
    num_samples: usize,
    alpha: f32,
    opts: EncodeOptions,
    rng: &mut impl Rng,
) -> Result<Vec<(Vec<u32>, f32)>, String> {
    if !(alpha.is_finite() && alpha >= 0.0) {
        return Err("sample_encode_and_score: alpha must be finite and >= 0".into());
    }
    if num_samples == 0 {
        return Err("sample_encode_and_score: num_samples must be >= 1".into());
    }
    let chars: Vec<char> = normalized.chars().collect();
    if chars.is_empty() {
        return Ok((0..num_samples).map(|_| (Vec::new(), 0.0)).collect());
    }
    let forced = resolve_forced(model, &opts)?;
    let lattice = populate_lattice(model, &chars, opts.clone(), &forced)?;
    {
        let mut check = populate_lattice(model, &chars, opts.clone(), &forced)?;
        let _ = check.viterbi().map_err(|_| {
            "UNIGRAM sample_encode_and_score: no covering segmentation (fail-closed)".to_string()
        })?;
    }

    let inv_theta = alpha;
    let alpha_fwd = lattice.forward(inv_theta);
    let marginal = alpha_fwd[lattice.eos()];

    let mut out = Vec::with_capacity(num_samples);
    for _ in 0..num_samples {
        let (path, path_score) = sample_path_nodes(&lattice, inv_theta, &alpha_fwd, rng)?;
        let ids = expand_path_ids(model, &chars, &lattice, &path, opts.clone())?;
        out.push((ids, path_score - marginal));
    }
    Ok(out)
}

/// Without-replacement SampleEncodeAndScore (official SP `wor=true`).
///
/// Draws Gumbel-perturbed A* n-best of size `num_samples + 1`, uses the
/// (k+1)-th perturbed score as κ for log inclusion probabilities
/// (`log Gumbel survival`). Optional `include_best` prepends the Viterbi path
/// with score `0` (inclusion probability 1) and removes it from the Gumbel draw.
pub fn sample_encode_and_score_unigram_wor(
    model: &SentencePieceModel,
    normalized: &str,
    num_samples: usize,
    alpha: f32,
    include_best: bool,
    opts: EncodeOptions,
    rng: &mut impl Rng,
) -> Result<Vec<(Vec<u32>, f32)>, String> {
    if !(alpha.is_finite() && alpha >= 0.0) {
        return Err("sample_encode_and_score: alpha must be finite and >= 0".into());
    }
    if num_samples == 0 {
        return Err("sample_encode_and_score: num_samples must be >= 1".into());
    }
    let chars: Vec<char> = normalized.chars().collect();
    if chars.is_empty() {
        return Ok((0..num_samples).map(|_| (Vec::new(), 0.0)).collect());
    }
    let forced = resolve_forced(model, &opts)?;
    let mut lattice = populate_lattice(model, &chars, opts.clone(), &forced)?;
    let (best_path, _) = lattice.viterbi().map_err(|_| {
        "UNIGRAM sample_encode_and_score WOR: no covering segmentation (fail-closed)".to_string()
    })?;

    let inv_theta = alpha;
    let alpha_fwd = lattice.forward(inv_theta);
    let marginal = alpha_fwd[lattice.eos()];

    let mut results: Vec<(Vec<u32>, f32)> = Vec::new();
    if include_best {
        let ids = expand_path_ids(model, &chars, &lattice, &best_path, opts.clone())?;
        // Inclusion probability 1 → log-prob 0.
        results.push((ids, 0.0));
    }

    // Need k+1 Gumbel-perturbed paths; κ is the (k+1)-th perturbed fx.
    // WOR sampling keeps default refuse on pathological agenda (stock_compat
    // still honored when explicitly requested via EncodeOptions).
    let limits = NbestLimits::from_opts(&opts);
    let outcome = nbest_astar(
        &lattice,
        num_samples + 1,
        true,
        inv_theta,
        Some(&alpha_fwd),
        Some(rng),
        limits,
    )?;
    let mut nbest_samples = match outcome {
        NbestAstarOutcome::ViterbiFallback => {
            // Stock timeout under agenda pressure: return Viterbi-only score row.
            let ids = expand_path_ids(model, &chars, &lattice, &best_path, opts)?;
            return Ok(vec![(ids, 0.0)]);
        }
        NbestAstarOutcome::Paths(paths) => paths,
    };

    if include_best {
        if let Some(idx) = nbest_samples.iter().position(|(p, _)| p == &best_path) {
            nbest_samples.remove(idx);
        } else if !nbest_samples.is_empty() {
            nbest_samples.pop();
        }
    }

    if nbest_samples.is_empty() {
        // Single unique path (or nothing left after removing best): match SP
        // post-#1218 — return whatever was already added (best if include_best).
        return Ok(results);
    }

    let kappa = f64::from(nbest_samples.last().unwrap().1);
    nbest_samples.pop();

    for (path, _perturbed_fx) in nbest_samples {
        let mut path_score = 0.0f32;
        for &nid in &path {
            path_score += inv_theta * lattice.nodes[nid].score;
        }
        let ids = expand_path_ids(model, &chars, &lattice, &path, opts.clone())?;
        results.push((ids, path_score - marginal));
    }

    // Convert path log-probs into log inclusion probabilities (skip exact 0.0
    // best entry, matching official `it.second != 0.0` guard).
    for (_ids, score) in &mut results {
        if *score != 0.0 {
            let x = f64::from(*score) - kappa;
            *score = log_gumbel_survival(x) as f32;
        }
    }

    Ok(results)
}

fn sample_path_nodes(
    lattice: &Lattice,
    inv_theta: f32,
    alpha_fwd: &[f32],
    rng: &mut impl Rng,
) -> Result<(Vec<usize>, f32), String> {
    let mut results = Vec::new();
    let mut path_score = 0.0f32;
    let mut z = alpha_fwd[lattice.eos()];
    let mut node = lattice.eos();
    loop {
        let ends = &lattice.end_nodes[lattice.nodes[node].pos];
        if ends.is_empty() {
            return Err("UNIGRAM sample: empty end_nodes".into());
        }
        let mut probs = Vec::with_capacity(ends.len());
        for &l_id in ends {
            let p = (alpha_fwd[l_id] + inv_theta * lattice.nodes[l_id].score - z).exp();
            probs.push(p.max(0.0));
        }
        let sum: f32 = probs.iter().sum();
        if sum <= 0.0 || !sum.is_finite() {
            return Err("UNIGRAM sample: invalid probability mass".into());
        }
        let mut r = rng.gen::<f32>() * sum;
        let mut choice = ends[0];
        for (i, &l_id) in ends.iter().enumerate() {
            r -= probs[i];
            if r <= 0.0 {
                choice = l_id;
                break;
            }
        }
        node = choice;
        if node == lattice.bos() {
            break;
        }
        path_score += inv_theta * lattice.nodes[node].score;
        z = alpha_fwd[node];
        results.push(node);
    }
    results.reverse();
    Ok((results, path_score))
}

/// Sample from the top-`nbest_size` lattice paths with temperature `alpha`
/// (official SP SampleEncode when `nbest_size > 1`).
pub fn sample_encode_from_nbest(
    model: &SentencePieceModel,
    normalized: &str,
    nbest_size: usize,
    alpha: f32,
    opts: EncodeOptions,
    rng: &mut impl Rng,
) -> Result<Vec<u32>, String> {
    if !(alpha.is_finite() && alpha >= 0.0) {
        return Err("sample_encode: alpha must be finite and >= 0".into());
    }
    let nbests = nbest_encode_unigram(model, normalized, nbest_size, opts)?;
    if nbests.is_empty() {
        return Err("sample_encode: empty nbest".into());
    }
    let log_probs: Vec<f32> = nbests.iter().map(|(_, s)| alpha * s).collect();
    let max_lp = log_probs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let weights: Vec<f32> = log_probs.iter().map(|&lp| (lp - max_lp).exp()).collect();
    let sum: f32 = weights.iter().sum();
    let mut r = rng.gen::<f32>() * sum;
    for (i, w) in weights.iter().enumerate() {
        r -= w;
        if r <= 0.0 {
            return Ok(nbests[i].0.clone());
        }
    }
    Ok(nbests.last().unwrap().0.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sentencepiece::{load_model_proto, ModelProtoFixture};

    #[test]
    fn nbest_stock_compat_shrinks_under_tiny_agenda_cap() {
        // Crafted UNIGRAM with multiple paths; force shrink with max_agenda=3.
        let fixture = ModelProtoFixture {
            pieces: vec![
                ("\u{2581}".into(), Some(-1.0), Some(1)),
                ("a".into(), Some(-1.0), Some(1)),
                ("b".into(), Some(-1.0), Some(1)),
                ("ab".into(), Some(-0.5), Some(1)),
                ("\u{2581}a".into(), Some(-0.5), Some(1)),
                ("\u{2581}ab".into(), Some(-0.1), Some(1)),
            ],
            model_type: Some(1),
            add_dummy_prefix: Some(true),
            escape_whitespaces: Some(true),
            remove_extra_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&fixture.encode()).unwrap();
        let normalized = "\u{2581}ab";
        let chars: Vec<char> = normalized.chars().collect();
        let opts = EncodeOptions {
            nbest_stock_compat: true,
            ..Default::default()
        };
        let forced = resolve_forced(&model, &opts).unwrap();
        let mut lattice = populate_lattice(&model, &chars, opts.clone(), &forced).unwrap();
        let _ = lattice.viterbi().unwrap();
        let limits = NbestLimits {
            max_agenda: 3,
            min_agenda: 2,
            timeout_ms: 30_000,
            stock_compat: true,
        };
        let outcome =
            nbest_astar::<rand::rngs::StdRng>(&lattice, 5, false, 0.0, None, None, limits).unwrap();
        match outcome {
            NbestAstarOutcome::Paths(paths) => {
                assert!(!paths.is_empty());
                assert!(paths.len() <= 5);
            }
            NbestAstarOutcome::ViterbiFallback => panic!("unexpected Viterbi fallback"),
        }
    }

    #[test]
    fn nbest_default_refuses_tiny_agenda_overflow() {
        let fixture = ModelProtoFixture {
            pieces: vec![
                ("\u{2581}".into(), Some(-1.0), Some(1)),
                ("a".into(), Some(-1.0), Some(1)),
                ("b".into(), Some(-1.0), Some(1)),
                ("ab".into(), Some(-0.5), Some(1)),
                ("\u{2581}a".into(), Some(-0.5), Some(1)),
                ("\u{2581}ab".into(), Some(-0.1), Some(1)),
            ],
            model_type: Some(1),
            add_dummy_prefix: Some(true),
            escape_whitespaces: Some(true),
            remove_extra_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&fixture.encode()).unwrap();
        let normalized = "\u{2581}ab";
        let chars: Vec<char> = normalized.chars().collect();
        let opts = EncodeOptions::default();
        let forced = resolve_forced(&model, &opts).unwrap();
        let mut lattice = populate_lattice(&model, &chars, opts.clone(), &forced).unwrap();
        let _ = lattice.viterbi().unwrap();
        let limits = NbestLimits {
            max_agenda: 3,
            min_agenda: 2,
            timeout_ms: 30_000,
            stock_compat: false,
        };
        let err = nbest_astar::<rand::rngs::StdRng>(&lattice, 5, false, 0.0, None, None, limits)
            .unwrap_err();
        assert!(err.contains("refuse rather than shrink"), "{err}");
    }
}
