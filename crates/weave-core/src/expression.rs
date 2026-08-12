//! The expression merge: a statement is a sequence of tokens.
//!
//! `region.rs` tiles a file into entities, `container.rs` tiles a class body
//! into members, `statement.rs` tiles a function body into statements. This
//! module is the same move once more, and it is the floor: when both sides
//! edited the SAME statement and diff3 refused it and the block fold could not
//! separate them, the statement is tiled into TOKENS, the two sides' token
//! edits are aligned against base, and a composition is emitted only when the
//! two edits are separated.
//!
//! The three properties `statement.rs` rests on hold here too, and one more
//! that only exists at this level:
//!
//!   1. **Partition identity.** `Σ(gap + text) + tail` reproduces the input
//!      byte for byte, per language, asserted by
//!      `a_tokenization_tiles_the_text_it_came_from`.
//!
//!   2. **Refinement, never trade.** The fold is reached from exactly one
//!      place: the arm of `statement::statement_merge_inner` that was about to
//!      write a conflict marker. Every verdict it can change is therefore
//!      CONFLICT → something. That one-way move is control flow, not measurement.
//!
//!   3. **Linear claims.** A base token is spent by at most one hunk of each
//!      side, and a [`TokenClaim`] is neither `Copy` nor `Clone`.
//!
//!   4. **The output is a SPLICE.** Every byte this module emits is a
//!      contiguous substring of base, of ours, or of theirs. It never
//!      concatenates two half-tokens, never inserts a separator of its own,
//!      never re-renders. So the failure mode this module exists to prevent —
//!      "a syntactically valid sentence neither side wrote" — cannot arrive
//!      through *characters*; it can only arrive through the arrangement of
//!      whole tokens, which is what the Guards section below is about.
//!
//! # Guards
//!
//! Four refusals. Each one guards a specific way whole-token composition can
//! silently produce bytes neither side wrote:
//!
//! - **Atomic literals and comments.** A string, char, or comment is ONE token.
//!   Consider a version constant with base `"4.0.0-beta-1-SNAPSHOT"`, ours
//!   `"4.0.0-beta-2-SNAPSHOT"`, theirs `"4.0.0-beta-1"`. Split into words those
//!   two edits look disjoint and compose to `"4.0.0-beta-2"` — a version
//!   *neither side wrote and nobody released*. Whole-literal tokens make it
//!   what it is: both sides replaced the same token, which is an overlap,
//!   which is a conflict.
//!
//! - **Trivia lines are not statements.** Two sides rewording one comment or
//!   docstring line: prose has no grammar this reader models, so two
//!   "disjoint" edits to a sentence compose into a sentence neither wrote. A
//!   hunk that lands on a line [`is_trivia_line`] recognises is refused.
//!
//! - **Anchoring, not merely non-overlap.** Every edit must be anchored to a
//!   base token the other side left standing. Consider ours inserting a call
//!   in the middle of an expression theirs replaces wholesale: the insertion
//!   point is *interior* to the replacement, so its anchor is a token theirs
//!   deleted; a non-overlap test on half-open ranges lets it through and
//!   composes the insertion nested inside the half-deleted expression, a
//!   shape neither side wrote. See [`separated`].
//!
//! - **Arity is not composable.** Two insertions into one positional argument
//!   list are two insertions into one gap even when they land at different
//!   positions: `f(a, b)` becoming `f(x, a, b)` and `f(a, b, d)` composes to a
//!   four-argument call no caller and no callee ever agreed on. Whenever two
//!   opposite-side hunks sit inside the SAME innermost bracket group, neither
//!   may change the number of commas at that group's level. Substituting
//!   *within* two different arguments is untouched by this, and that is the
//!   case the fold exists for.
//!
//! # Where it is NOT called, and why that was measured
//!
//! The obvious second home for a token fold is `statement::merge_trivia`, which
//! merges a body's *declaration* and reaches its conflict arm from the same
//! already-a-conflict position. A method signature is one line that two sides
//! routinely edit for unrelated reasons — one appends a `throws`, the other adds
//! a parameter — and composing it looks like the same argument.
//!
//! It was built and then removed. Composing declaration-line edits converted
//! more conflicts into signatures nobody wrote than into ones both sides
//! actually meant, and a signature is a contract that callers elsewhere in
//! the tree depend on — this module can see one line of it, not the callers.
//! The risk outweighed the gain, so the level is where it stays.
//!
//! The fold is otherwise order-preserving: it never permutes.
//! Base's token order is the output's token order, with each side's replacement
//! standing where base's tokens stood. Positional argument lists, parameter
//! lists and statement sequences are therefore ORDER-EFFECTFUL and stay so
//! without a rule saying they are — the only ordering question this module can
//! be asked is the same-gap one, and it answers it by refusing.

use crate::binding::is_trivia_line;

// ---------------------------------------------------------------------------
// Claims
// ---------------------------------------------------------------------------

/// A move-once handle naming one base token. Neither `Copy` nor `Clone`,
/// minted only by [`tokenize`], spent at most once per side by the fold — the
/// same trick `statement::BasePart` and `v2::types::Claim` play, one level down.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TokenClaim(usize);

impl TokenClaim {
    fn index(&self) -> usize {
        self.0
    }
}

// ---------------------------------------------------------------------------
// Tokens
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// An identifier or keyword.
    Word,
    /// A numeric literal, whole.
    Number,
    /// A string or character literal, whole — quotes included.
    Str,
    /// A line or block comment, whole.
    Comment,
    /// Punctuation or an operator.
    Op,
}

/// One token and the whitespace that precedes it, as byte offsets into the
/// source. `src[gap..text]` is whitespace; `src[text..end]` is the token.
#[derive(Debug, Clone, Copy)]
struct Tok {
    gap: usize,
    text: usize,
    end: usize,
    kind: Kind,
}

/// Operators that are one token even though they are several characters.
/// Longest match wins, so the table is scanned in descending length.
///
/// This is a fact about the languages in scope, not a grammar: `!=` is one
/// operator in every one of them, and splitting it would let one side edit the
/// `!` while the other edits the `=`.
const OPERATORS: &[&str] = &[
    ">>>=", "<<=", ">>=", ">>>", "...", "&&=", "||=", "**=", "//=", "===", "!==", "??=", "==",
    "!=", "<=", ">=", "&&", "||", "->", "=>", "::", "++", "--", "+=", "-=", "*=", "/=", "%=", "&=",
    "|=", "^=", "<<", ">>", "**", "??", "?.", "..", "!!", ":=", "<-", "|>",
];

/// A statement with more tokens than this is not composed, and `tokenize` stops
/// as soon as it knows that — so the refusal costs a bounded scan rather than a
/// full one.
///
/// The bound is a design statement before it is a budget. This level composes a
/// *statement*, and a statement is a sentence: 256 tokens is already a very long
/// line. A run of text longer than that which reached this arm is a BLOCK, and a
/// block is `statement.rs`'s recursion to partition, not this module's to align.
///
/// It is also the latency bound. The alignment is quadratic and runs once per
/// statement per recursion level, so the cap is what keeps a 500-line file with
/// one large `switch` from paying for an alignment of text that was never a
/// sentence. Raising it further does not change any resolution it was tested
/// against, so the recall it costs is zero and the bound is free.
const MAX_TOKENS: usize = 256;

/// Tile `src` into tokens, or `None` if this reader does not understand it.
///
/// `None` costs the caller the conflict it was already going to raise, which is
/// why every uncertainty answers `None`. The one construct that is genuinely
/// unrecoverable is an unterminated literal or block comment: everything after
/// it was read as code that is not code.
fn tokenize(src: &str) -> Option<Vec<Tok>> {
    let b = src.as_bytes();
    // Does this text write a lifetime anywhere it cannot be mistaken for a
    // quote? See `is_lifetime`.
    let rustish = b.windows(2).any(|w| {
        // `windows(2)` yields pairs, so both indices are in range by construction.
        matches!(w[0], b'&' | b'<') && w[1] == b'\''
    });
    let mut out: Vec<Tok> = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        let gap = i;
        while i < b.len() && (b[i] as char).is_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let text = i;
        let rest = &src[i..];
        let kind;

        if rest.starts_with("//") {
            kind = Kind::Comment;
            i += rest.find('\n').unwrap_or(rest.len());
        } else if rest.starts_with("/*") {
            kind = Kind::Comment;
            // An unterminated block comment is the state this reader cannot
            // recover from — see `statement::depths`, same rule, same reason.
            i += rest.find("*/").map(|p| p + 2)?;
        } else if b[i] == b'#' && !rest.starts_with("#[") && !rest.starts_with("#!") {
            // `#` opens a comment in Python, Ruby, shell. `#[` is a Rust
            // attribute and `#!` a shebang or inner attribute; both are code.
            kind = Kind::Comment;
            i += rest.find('\n').unwrap_or(rest.len());
        } else if rest.starts_with("\"\"\"") || rest.starts_with("'''") {
            let q = &rest[..3];
            kind = Kind::Str;
            i += 3 + rest[3..].find(q).map(|p| p + 3)?;
        } else if b[i] == b'\'' && is_lifetime(b, i, rustish) {
            // `&'a str`, `<'a>` — an apostrophe that opens nothing.
            kind = Kind::Op;
            i += 1;
        } else if b[i] == b'"' || b[i] == b'\'' || b[i] == b'`' {
            let quote = b[i];
            match close_on_line(b, i, quote) {
                Some(end) => {
                    kind = Kind::Str;
                    i = end + 1;
                }
                // No closer on this line: a literal this reader cannot see the
                // end of, and guessing where it ends is how a composition lands
                // inside a string. Refuse the statement rather than the token —
                // reading on would mean reading text as code that is not code,
                // the one state `statement::depths` also treats as fatal.
                None => return None,
            }
        } else if b[i].is_ascii_digit() {
            kind = Kind::Number;
            i += 1;
            while i < b.len() {
                let c = b[i];
                let continues = c.is_ascii_alphanumeric()
                    || c == b'_'
                    // `1.5` is one number, but `1..5` is a range and `t.0.1` a
                    // field path — so a dot continues a number only in front of
                    // a digit.
                    || (c == b'.' && b.get(i + 1).is_some_and(|d| d.is_ascii_digit()))
                    // `1e-3`: the sign belongs to the exponent, not to the
                    // expression around it.
                    || ((c == b'+' || c == b'-')
                        && matches!(b[i - 1], b'e' | b'E')
                        && b.get(i + 1).is_some_and(|d| d.is_ascii_digit()));
                if !continues {
                    break;
                }
                i += 1;
            }
        } else if b[i].is_ascii_alphabetic() || b[i] == b'_' || b[i] == b'$' {
            kind = Kind::Word;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
                i += 1;
            }
        } else if !b[i].is_ascii() {
            // A non-ASCII run is one token: an identifier in a language that
            // allows them, or text inside something this reader mis-read.
            // Either way it is not something to compose *inside* of.
            kind = Kind::Word;
            while i < b.len() && !b[i].is_ascii() {
                i += 1;
            }
        } else {
            kind = Kind::Op;
            match OPERATORS.iter().find(|op| rest.starts_with(**op)) {
                Some(op) => i += op.len(),
                None => i += 1,
            }
        }

        if out.len() >= MAX_TOKENS {
            return None;
        }
        out.push(Tok {
            gap,
            text,
            end: i,
            kind,
        });
    }
    Some(out)
}

/// Is the apostrophe at `i` a Rust lifetime rather than a literal?
///
/// The ambiguity is real and the two mistakes are not symmetric. Reading a
/// lifetime as a string makes one large atomic token out of code — clumsy, and
/// it costs compositions, but it cannot compose *inside* anything. Reading a
/// Python `'a b'` as a lifetime does the opposite: it splits a literal into
/// words, and two sides editing different words of one string then compose into
/// a string neither wrote — the same failure the atomic-literal guard exists
/// to prevent. So the default answer is "literal", and this returns true only
/// on the shape Rust
/// actually writes: an apostrophe DIRECTLY after `&` or `<` (or after `<` and a
/// comma, as in `<'a, 'b>`), followed by an identifier that is not closed by a
/// second apostrophe.
///
/// `'a'` — a char literal — is excluded by the trailing-quote test, and no
/// language in scope writes `&'word` or `<'word` to mean a string.
///
/// `rustish` carries the one concession to `<'a, 'b>`, where the second
/// lifetime is preceded by a comma and a comma proves nothing on its own
/// (`f(a, 'b c')` is a Python call). It is set only when the SAME text already
/// contains an unambiguous `&'` or `<'`, so a language that never writes one
/// never gets the concession.
fn is_lifetime(b: &[u8], i: usize, rustish: bool) -> bool {
    let immediate = i > 0 && matches!(b[i - 1], b'&' | b'<');
    if !immediate && !rustish {
        return false;
    }
    let mut j = i + 1;
    if j >= b.len() || !(b[j].is_ascii_alphabetic() || b[j] == b'_') {
        return false;
    }
    while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
        j += 1;
    }
    // `'a'` closes: that is a char literal, not a lifetime.
    b.get(j) != Some(&b'\'')
}

/// Index of the closing quote matching the opener at `start`, on this line.
/// The twin of `statement::close_on_line`, on bytes.
fn close_on_line(b: &[u8], start: usize, quote: u8) -> Option<usize> {
    let mut i = start + 1;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'\n' => return None,
            c if c == quote => return Some(i),
            _ => i += 1,
        }
    }
    None
}

fn tok_text<'a>(src: &'a str, t: &Tok) -> &'a str {
    &src[t.text..t.end]
}

/// The identity of a token: what it is and what it says. Whitespace is not part
/// of it, so a reformatting produces no hunks at all.
fn key<'a>(src: &'a str, t: &Tok) -> (Kind, &'a str) {
    (t.kind, tok_text(src, t))
}

// ---------------------------------------------------------------------------
// Alignment
// ---------------------------------------------------------------------------

/// One contiguous edit: base tokens `[b0, b1)` became this side's `[s0, s1)`.
/// `b0 == b1` is an insertion before base token `b0`; `s0 == s1` a deletion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Hunk {
    b0: usize,
    b1: usize,
    s0: usize,
    s1: usize,
}

/// Cells the token LCS may fill. Tighter than `statement::MAX_LCS` because a
/// statement is reached once per statement per level, and there are many more
/// statements than there are bodies.
const MAX_LCS: usize = 60_000;

/// Longest common subsequence of two token-key sequences, as index pairs.
/// Deterministic: ties break toward advancing `a`, so the answer is a function
/// of the two sequences only, not of iteration or hashing order.
fn lcs_pairs(a: &[(Kind, &str)], b: &[(Kind, &str)]) -> Option<Vec<(usize, usize)>> {
    if a.len().saturating_mul(b.len()) > MAX_LCS {
        return None;
    }
    let (n, m) = (a.len(), b.len());
    let mut table = vec![0u32; (n + 1) * (m + 1)];
    let at = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[at(i, j)] = if a[i] == b[j] {
                table[at(i + 1, j + 1)] + 1
            } else {
                table[at(i + 1, j)].max(table[at(i, j + 1)])
            };
        }
    }
    let mut pairs = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if a[i] == b[j] {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if table[at(i + 1, j)] >= table[at(i, j + 1)] {
            i += 1;
        } else {
            j += 1;
        }
    }
    Some(pairs)
}

/// The maximal runs where one side differs from base.
fn hunks(bk: &[(Kind, &str)], sk: &[(Kind, &str)]) -> Option<Vec<Hunk>> {
    let pairs = lcs_pairs(bk, sk)?;
    let mut out = Vec::new();
    let (mut bi, mut si) = (0usize, 0usize);
    let mut anchors = pairs;
    anchors.push((bk.len(), sk.len()));
    for (ab, as_) in anchors {
        if bi < ab || si < as_ {
            out.push(Hunk {
                b0: bi,
                b1: ab,
                s0: si,
                s1: as_,
            });
        }
        bi = ab + 1;
        si = as_ + 1;
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// Guards
// ---------------------------------------------------------------------------

/// The innermost bracket group each token belongs to: the index of the opening
/// bracket token that encloses it, or `None` at the statement's own level.
///
/// A closing bracket is assigned to the group it CLOSES, so `)` in `f(a, b)`
/// belongs to the `(` group and an insertion before it is an insertion into the
/// argument list, which is precisely what the arity guard needs to see.
fn groups(src: &str, toks: &[Tok]) -> Vec<Option<usize>> {
    let mut out = Vec::with_capacity(toks.len());
    let mut stack: Vec<usize> = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        let s = tok_text(src, t);
        if t.kind == Kind::Op && matches!(s, "(" | "[" | "{") {
            out.push(stack.last().copied());
            stack.push(i);
        } else if t.kind == Kind::Op && matches!(s, ")" | "]" | "}") {
            out.push(stack.last().copied());
            stack.pop();
        } else {
            out.push(stack.last().copied());
        }
    }
    out
}

/// Is each edit anchored to something the other side left standing?
///
/// This is subtler than "do the ranges overlap". What makes an edit placeable
/// is that it is ANCHORED: some base token, surviving into the output, says
/// where it goes. Two shapes of edit are anchored differently, so they get
/// different tests.
///
/// - A **substitution** owns base tokens `[b0, b1)`. Its anchor is those tokens'
///   own position, which nothing else can move. Two substitutions therefore need
///   nothing between them, only to own different tokens: `a + b` with one side
///   writing `c + b` and the other `a - b` composes to `c - b`, and there is no
///   ordering question in it to get wrong.
///
/// - An **insertion** (`b0 == b1`) owns nothing. It is anchored to the base
///   token it sits BEFORE, so it is placeable exactly while that token survives.
///   An insertion at `p` and a substitution over `[c, d)` are compatible when
///   `p < c` or `p >= d`, and refused for `c <= p < d` — where the anchor is a
///   token the other side deleted. That case is real: one side inserting a
///   method call into the middle of an expression the other side replaced
///   wholesale is exactly the shape where a plain non-overlap test on
///   half-open ranges would let it through and compose two edits that were
///   never meant to combine.
///
/// - Two **insertions** at one point have no separator between them at all:
///   every order is a program and none of them is one a side wrote. At different
///   points each is anchored to a different surviving token and their order is
///   base's. What still refuses two additions to one argument list is
///   [`changes_arity`], which is the right guard for it — the hazard there is
///   the arity, not the placement.
fn separated(o: &Hunk, t: &Hunk) -> bool {
    let anchored = |p: usize, h: &Hunk| p < h.b0 || p >= h.b1;
    match (o.b0 == o.b1, t.b0 == t.b1) {
        (true, true) => o.b0 != t.b0,
        (true, false) => anchored(o.b0, t),
        (false, true) => anchored(t.b0, o),
        (false, false) => o.b1 <= t.b0 || t.b1 <= o.b0,
    }
}

/// Commas at the hunk's own bracket level, in one side's replacement text.
///
/// Only commas at relative depth zero count: a comma inside a nested call the
/// hunk introduced is that call's arity, not this group's.
fn top_level_commas(src: &str, toks: &[Tok], lo: usize, hi: usize) -> usize {
    let mut depth = 0i32;
    let mut n = 0usize;
    for t in &toks[lo..hi] {
        if t.kind != Kind::Op {
            continue;
        }
        match tok_text(src, t) {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            "," if depth == 0 => n += 1,
            _ => {}
        }
    }
    n
}

/// Does this hunk change the number of items in the list it sits in?
fn changes_arity(base: &str, btoks: &[Tok], side: &str, stoks: &[Tok], h: &Hunk) -> bool {
    top_level_commas(base, btoks, h.b0, h.b1) != top_level_commas(side, stoks, h.s0, h.s1)
}

/// Does any part of this hunk land on a line that is trivia — a comment, a
/// javadoc continuation, an annotation?
///
/// Prose is not a grammar, so two "disjoint" edits to one sentence compose into
/// a sentence neither side wrote. Annotations are excluded for a different
/// reason: `container.rs` merges decorators as a set at its own level, and a
/// token fold reaching into one is the wrong level answering.
fn touches_trivia(src: &str, toks: &[Tok], lo: usize, hi: usize) -> bool {
    // A text with no tokens at all is not a statement, and "is it trivia" is
    // the honest yes rather than a vacuous no.
    if toks.is_empty() {
        return true;
    }
    // An insertion has no tokens of its own. The line it lands on is the one
    // carrying the token it sits before — or, for an insertion past the end,
    // the line of the last token. Written as three cases rather than as
    // arithmetic that clamps, because the three cases are what is meant and a
    // clamp reads as a repair of an index that was never wrong.
    let probe: &[Tok] = if lo < hi {
        &toks[lo..hi]
    } else if lo < toks.len() {
        &toks[lo..lo + 1]
    } else {
        &toks[toks.len() - 1..]
    };
    probe.iter().any(|t| {
        let line_start = src[..t.text].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let line_end = src[t.text..]
            .find('\n')
            .map(|p| t.text + p)
            .unwrap_or(src.len());
        is_trivia_line(&src[line_start..line_end])
    })
}

/// Net bracket depth change over a token run — the arithmetic the composition
/// has to conserve.
fn net_depth(src: &str, toks: &[Tok]) -> i32 {
    let mut d = 0i32;
    for t in toks {
        if t.kind != Kind::Op {
            continue;
        }
        match tok_text(src, t) {
            "(" | "[" | "{" => d += 1,
            ")" | "]" | "}" => d -= 1,
            _ => {}
        }
    }
    d
}

/// Outside its own edits, did this side leave base's whitespace alone?
///
/// It has to be asked, because the composition emits base's gaps for every
/// token neither side replaced. If ours re-indented the statement and theirs
/// edited a token in it, splicing theirs onto base's columns would silently
/// discard ours' re-indentation — and in an indentation-scoped language like
/// Python, a dedent can move a statement into a different block, so silently
/// dropping it is a silent change of program structure, not just of style.
/// So a side that touched whitespace it did not otherwise edit refuses the
/// composition.
fn gaps_faithful(base: &str, btoks: &[Tok], side: &str, stoks: &[Tok], hs: &[Hunk]) -> bool {
    // Base token indices that begin an unchanged run following one of this
    // side's hunks: the gap in front of those legitimately changed.
    let after: Vec<usize> = hs.iter().map(|h| h.b1).collect();
    let mut bi = 0usize;
    let mut si = 0usize;
    for h in hs {
        while bi < h.b0 {
            if !after.contains(&bi)
                && base[btoks[bi].gap..btoks[bi].text] != side[stoks[si].gap..stoks[si].text]
            {
                return false;
            }
            bi += 1;
            si += 1;
        }
        bi = h.b1;
        si = h.s1;
    }
    while bi < btoks.len() {
        if si >= stoks.len() {
            return false;
        }
        if !after.contains(&bi)
            && base[btoks[bi].gap..btoks[bi].text] != side[stoks[si].gap..stoks[si].text]
        {
            return false;
        }
        bi += 1;
        si += 1;
    }
    true
}

// ---------------------------------------------------------------------------
// The fold
// ---------------------------------------------------------------------------

/// One accepted composition, with the lines it created.
#[derive(Debug, Clone)]
pub(crate) struct Composition {
    pub text: String,
    /// The non-blank output lines that no version wrote verbatim. The statement
    /// fold's no-fabrication backstop licenses exactly these and nothing else,
    /// so a line this module did not account for still fails the check.
    pub composed_lines: Vec<String>,
}

/// Compose two edits to one statement, or `None`.
///
/// Reached from exactly one place — the arm of `statement::statement_merge_inner`
/// that was about to emit a conflict marker — so a `None` here costs nothing
/// that was not already lost, and a `Some` can only have turned a conflict into
/// a resolution.
pub(crate) fn expression_merge(base: &str, ours: &str, theirs: &str) -> Option<Composition> {
    let btoks = tokenize(base)?;
    let otoks = tokenize(ours)?;
    let ttoks = tokenize(theirs)?;
    if btoks.is_empty() {
        return None;
    }

    let bk: Vec<(Kind, &str)> = btoks.iter().map(|t| key(base, t)).collect();
    let ok: Vec<(Kind, &str)> = otoks.iter().map(|t| key(ours, t)).collect();
    let tk: Vec<(Kind, &str)> = ttoks.iter().map(|t| key(theirs, t)).collect();

    let ohs = hunks(&bk, &ok)?;
    let ths = hunks(&bk, &tk)?;
    // Both sides must actually have edited a token. If one of them changed only
    // whitespace, this is not a composition and `formatting_tie` one level up
    // is the right reader for it.
    if ohs.is_empty() || ths.is_empty() {
        return None;
    }

    // Mint one claim per base token. From here a base token is reached only
    // through the claim that names it.
    let claims: Vec<TokenClaim> = (0..btoks.len()).map(TokenClaim).collect();

    // Guard: a separator, not merely no overlap.
    for o in &ohs {
        for t in &ths {
            if !separated(o, t) {
                return None;
            }
        }
    }

    // Guard: trivia is not a statement.
    for h in &ohs {
        if touches_trivia(base, &btoks, h.b0, h.b1) || touches_trivia(ours, &otoks, h.s0, h.s1) {
            return None;
        }
    }
    for h in &ths {
        if touches_trivia(base, &btoks, h.b0, h.b1) || touches_trivia(theirs, &ttoks, h.s0, h.s1) {
            return None;
        }
    }

    // Guard: arity is not composable. Two hunks inside one bracket group may
    // each rewrite an item; neither may add or drop one.
    let bgroups = groups(base, &btoks);
    let group_of = |h: &Hunk| -> Option<usize> {
        let i = if h.b0 < h.b1 {
            h.b0
        } else {
            h.b0.min(btoks.len() - 1)
        };
        bgroups.get(i).copied().flatten()
    };
    for o in &ohs {
        for t in &ths {
            if group_of(o) == group_of(t)
                && (changes_arity(base, &btoks, ours, &otoks, o)
                    || changes_arity(base, &btoks, theirs, &ttoks, t))
            {
                return None;
            }
        }
    }

    // Guard: a side that touched whitespace it did not otherwise edit is not
    // composable, because the splice emits base's gaps.
    if !gaps_faithful(base, &btoks, ours, &otoks, &ohs)
        || !gaps_faithful(base, &btoks, theirs, &ttoks, &ths)
    {
        return None;
    }

    // --- the splice ---------------------------------------------------------
    //
    // One pass over base's tokens. Every emitted byte is a contiguous substring
    // of base, of ours, or of theirs.
    // Exactly ONE gap is emitted in front of every token, and it is always some
    // input's own bytes. That is the whole discipline here, and getting it wrong
    // is not cosmetic: emitting the inserted run's leading gap AND base's gap
    // for the token behind it can double an indentation gap that belonged to
    // only one side, producing a line neither side wrote. So an insertion
    // takes BASE's gap as its own lead, and hands the base token behind it the
    // side's gap that followed the inserted run.
    struct Edit<'a> {
        src: &'a str,
        toks: &'a [Tok],
        s0: usize,
        s1: usize,
        insertion: bool,
    }
    fn place<'a>(
        plan: &mut [Option<Edit<'a>>],
        h: &Hunk,
        src: &'a str,
        toks: &'a [Tok],
    ) -> Option<()> {
        let slot = plan.get_mut(h.b0)?;
        if slot.is_some() {
            return None; // two hunks of different sides on one slot
        }
        *slot = Some(Edit {
            src,
            toks,
            s0: h.s0,
            s1: h.s1,
            insertion: h.b0 == h.b1,
        });
        Some(())
    }
    let mut plan: Vec<Option<Edit>> = (0..=btoks.len()).map(|_| None).collect();
    for h in &ohs {
        place(&mut plan, h, ours, &otoks)?;
    }
    for h in &ths {
        place(&mut plan, h, theirs, &ttoks)?;
    }
    // Which base tokens a replace/drop consumed, so they are not emitted twice.
    let mut consumed = vec![false; btoks.len()];
    for h in ohs.iter().chain(ths.iter()) {
        consumed[h.b0..h.b1].fill(true);
    }

    let mut out = String::with_capacity(base.len() + ours.len() + theirs.len());
    for claim in &claims {
        let i = claim.index();
        // Base's own gap, unless an insertion at this slot takes it and hands
        // back its own.
        let mut gap: &str = &base[btoks[i].gap..btoks[i].text];
        if let Some(e) = &plan[i] {
            if e.s0 < e.s1 {
                if e.insertion {
                    // An inserted run has TWO seams, and the side is the only
                    // version that has ever had these things adjacent — so both
                    // gaps are the side's, and base's gap at this slot is not
                    // emitted at all. Emitting base's as well is what wrote
                    // `throw    FUNCAST`; using base's instead of the side's is
                    // what wrote `howFull()< 0.1`. There is one gap per seam and
                    // the side owns both of these.
                    out.push_str(&e.src[e.toks[e.s0].gap..e.toks[e.s1 - 1].end]);
                    gap = e.src.get(e.toks[e.s1 - 1].end..e.toks.get(e.s1)?.text)?;
                } else {
                    // A substitution stands exactly where base's tokens stood,
                    // so the gap in front of it is base's — which is what keeps
                    // a re-indentation from riding in on a token edit.
                    out.push_str(gap);
                    out.push_str(&e.src[e.toks[e.s0].text..e.toks[e.s1 - 1].end]);
                }
            }
            // `s0 == s1` is a deletion: the run and the gap in front of it go.
        }
        if !consumed[i] {
            out.push_str(gap);
            out.push_str(&base[btoks[i].text..btoks[i].end]);
        }
    }
    if let Some(e) = &plan[btoks.len()] {
        if e.s0 < e.s1 {
            // An insertion past the last base token brings its own lead, since
            // there is no base gap in front of it to take.
            out.push_str(&e.src[e.toks[e.s0].gap..e.toks[e.s1 - 1].end]);
        }
    }
    // Base's trailing bytes: the newline the statement ends with, and any
    // trailing whitespace. Partition identity says they are base's to give.
    out.push_str(&base[btoks[btoks.len() - 1].end..]);

    // --- guards on the finished bytes ---------------------------------------
    //
    // The composition must conserve the bracket arithmetic: whatever each side
    // did to the depth, the composition did both.
    let ctoks = tokenize(&out)?;
    let want = net_depth(ours, &otoks) + net_depth(theirs, &ttoks) - net_depth(base, &btoks);
    if net_depth(&out, &ctoks) != want {
        return None;
    }
    // A composition equal to one side's own text is not a composition; the arm
    // above would already have taken it, and admitting it here would let the
    // fold claim a resolution it did not make.
    if out == ours || out == theirs || out == base {
        return None;
    }

    let known: std::collections::HashSet<&str> = base
        .lines()
        .chain(ours.lines())
        .chain(theirs.lines())
        .map(|l| l.trim_end())
        .collect();
    let composed_lines: Vec<String> = out
        .lines()
        .map(|l| l.trim_end())
        .filter(|l| !l.trim().is_empty() && !known.contains(l))
        .map(|l| l.to_string())
        .collect();
    // A composition that created no new line changed nothing worth the risk.
    if composed_lines.is_empty() {
        return None;
    }

    Some(Composition {
        text: out,
        composed_lines,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Per language, with the constructs whose tokenization is not obvious:
    /// Rust lifetimes, Python triple quotes, Java generics, TS template
    /// literals, Go raw strings, escaped quotes, block comments.
    const SAMPLES: &[&str] = &[
        // java
        "    return (var.type != null ? var.type.cast(v, ctx, input) : v).iter(ctx);\n",
        "    private static final String VERSION = \"4.0.0-beta-1-SNAPSHOT\";\n",
        "    Map<String, List<Integer>> m = new HashMap<>(); // trailing\n",
        // rust
        "    fn h<'a>(s: &'a str) -> &'a str { s.trim() }\n",
        "    let x = a >>= b; /* block */ let y = 0x1f_u8;\n",
        "    #[derive(Debug, Clone)]\n",
        // python
        "    s = \"\"\"triple\n    quoted\"\"\"  # comment\n",
        "    d = {'a': 1, 'b': 2}\n",
        // typescript
        "  const q = `tpl ${x}`, r = a?.b ?? c;\n",
        "  export default function f(a: number = 1e-3): void {}\n",
        // go
        "\ttotal := 0\n\tfor _, i := range x {\n\t\ttotal += i\n\t}\n",
        // escapes and unicode
        "    print(\"he said \\\"hi\\\"\", 'x', \"héllo wörld\")\n",
    ];

    /// The property everything else rests on.
    #[test]
    fn a_tokenization_tiles_the_text_it_came_from() {
        for src in SAMPLES {
            let toks = tokenize(src).unwrap_or_else(|| panic!("tokenizes: {src:?}"));
            let mut rebuilt = String::new();
            for t in &toks {
                rebuilt.push_str(&src[t.gap..t.end]);
            }
            if let Some(last) = toks.last() {
                rebuilt.push_str(&src[last.end..]);
            }
            assert_eq!(rebuilt, *src, "tokenization lost or duplicated text");
        }
    }

    #[test]
    fn literals_and_comments_are_one_token_each() {
        let src = "x = \"a b c\"; // one comment\n";
        let toks = tokenize(src).unwrap();
        let strs: Vec<&str> = toks
            .iter()
            .filter(|t| t.kind == Kind::Str)
            .map(|t| tok_text(src, t))
            .collect();
        assert_eq!(strs, vec!["\"a b c\""]);
        let cs: Vec<&str> = toks
            .iter()
            .filter(|t| t.kind == Kind::Comment)
            .map(|t| tok_text(src, t))
            .collect();
        assert_eq!(cs, vec!["// one comment"]);
    }

    #[test]
    fn a_rust_lifetime_is_not_a_string() {
        let src = "fn h<'a>(s: &'a str) {}\n";
        let toks = tokenize(src).unwrap();
        assert!(
            toks.iter().all(|t| t.kind != Kind::Str),
            "a lifetime was read as a literal"
        );
    }

    #[test]
    fn an_unterminated_literal_is_refused() {
        assert!(tokenize("x = \"open\n").is_none());
        assert!(tokenize("x = 1; /* open\n").is_none());
    }

    // --- concrete merge cases -------------------------------------------

    /// Two disjoint token edits to the same statement: the base case the
    /// fold exists to compose.
    #[test]
    fn disjoint_token_edits_to_one_statement_compose() {
        let b = "    return (var.type != null ? var.type.cast(v, ctx, input) : v).iter(ctx);\n";
        let o = "    return (var.type != null ? var.type.cast(v, ctx, input) : v).iter();\n";
        let t = "    return (ret != null ? ret.cast(v, ctx, input) : v).iter(ctx);\n";
        let c = expression_merge(b, o, t).expect("composes");
        assert_eq!(
            c.text,
            "    return (ret != null ? ret.cast(v, ctx, input) : v).iter();\n"
        );
    }

    /// Both sides replace the same string literal; a literal is one token, so
    /// this is an overlap and not a composition.
    #[test]
    fn a_string_literal_edited_by_both_sides_is_not_composable() {
        let b = "\tprivate static final String VERSION = \"4.0.0-beta-1-SNAPSHOT\";\n";
        let o = "\tprivate static final String VERSION = \"4.0.0-beta-2-SNAPSHOT\";\n";
        let t = "\tprivate static final String VERSION = \"4.0.0-beta-1\";\n";
        assert!(
            expression_merge(b, o, t).is_none(),
            "composed a version neither side released"
        );
    }

    /// An insertion interior to a replacement: non-overlap is not
    /// separation.
    #[test]
    fn an_insertion_interior_to_a_replacement_is_not_separated() {
        let b = "\t\tString timestamp = DATE_FORMAT_14_FORMATTER.format(date);\n";
        let o = "\t\tString timestamp = DATE_FORMAT_14_FORMATTER.get().format(date);\n";
        let t = "\t\tString timestamp = ArchiveUtils.get14DigitDate(date);\n";
        assert!(
            expression_merge(b, o, t).is_none(),
            "composed ArchiveUtils.get().get14DigitDate"
        );
    }

    /// Two rewordings of one doc-comment line.
    #[test]
    fn two_rewordings_of_one_comment_line_are_not_composable() {
        let b = "   * @throws java.lang.IllegalArgumentException if the String value is not a legal Base64 encoded value\n";
        let o = "   * @throws java.lang.IllegalArgumentException if the value is not a legal Base64 encoded String\n";
        let t = "   * @throws java.lang.IllegalArgumentException if the String is not a legal Base64 encoded value\n";
        assert!(
            expression_merge(b, o, t).is_none(),
            "composed a sentence neither side wrote"
        );
    }

    /// One side renames the field, the other changes its type argument.
    /// Genuinely independent.
    #[test]
    fn a_field_rename_beside_an_unrelated_type_change_composes() {
        let b = "\tprivate @Mock Event<ControllerMethod> event;\n";
        let o = "\tprivate @Mock Event<ControllerMethod> controllerMethodEvent;\n";
        let t = "\tprivate @Mock Event<ControllerFound> event;\n";
        let c = expression_merge(b, o, t).expect("composes");
        assert_eq!(
            c.text,
            "\tprivate @Mock Event<ControllerFound> controllerMethodEvent;\n"
        );
    }

    /// One side changes the log level, the other fixes a typo inside the
    /// message. The literal is one token and only one side touched it, so it
    /// composes.
    #[test]
    fn a_log_level_change_beside_a_message_fix_composes() {
        let b = "            getLog().info( \"android.device parameter not set\" );\n";
        let o = "            getLog().debug( \"android.device parameter not set\" );\n";
        let t = "            getLog().info( \"android.devices parameter not set\" );\n";
        let c = expression_merge(b, o, t).expect("composes");
        assert_eq!(
            c.text,
            "            getLog().debug( \"android.devices parameter not set\" );\n"
        );
    }

    /// Two insertions into one positional argument list are two insertions into
    /// one gap, wherever in the list they land.
    #[test]
    fn two_arguments_added_to_one_call_are_not_composable() {
        let b = "    f(a, b);\n";
        let o = "    f(x, a, b);\n";
        let t = "    f(a, b, d);\n";
        assert!(
            expression_merge(b, o, t).is_none(),
            "composed a four-argument call nobody wrote"
        );
    }

    /// ...but rewriting two different arguments of one call is exactly the
    /// case this module exists for.
    #[test]
    fn two_arguments_rewritten_in_one_call_compose() {
        let b = "    f(alpha, beta);\n";
        let o = "    f(gamma, beta);\n";
        let t = "    f(alpha, delta);\n";
        let c = expression_merge(b, o, t).expect("composes");
        assert_eq!(c.text, "    f(gamma, delta);\n");
    }

    /// Two SUBSTITUTIONS need no token between them: each is anchored to the
    /// base token it replaced, and nothing can move that.
    #[test]
    fn adjacent_substitutions_compose() {
        let b = "    let x = a + b;\n";
        let o = "    let x = c + b;\n";
        let t = "    let x = a - b;\n";
        let c = expression_merge(b, o, t).expect("composes");
        assert_eq!(c.text, "    let x = c - b;\n");
    }

    /// An INSERTION is anchored to the token it sits before, so it is refused
    /// when the other side deleted that token — even though the ranges do not
    /// overlap on a half-open reading.
    #[test]
    fn an_insertion_anchored_to_a_deleted_token_is_refused() {
        let b = "    let x = alpha.beta(q);\n";
        let o = "    let x = alpha.mid().beta(q);\n";
        let t = "    let x = gamma.delta(q);\n";
        assert!(expression_merge(b, o, t).is_none());
    }

    /// A side that re-indented is a side that edited whitespace, and the splice
    /// emits base's columns. Refuse rather than discard it silently.
    #[test]
    fn a_reindent_beside_an_edit_is_refused() {
        let b = "\t    player.equip(new Item(x));\n";
        let o = "\t    player.equip(itemRepo.get(x));\n";
        let t = "            player.equip(new Item(x));\n";
        assert!(expression_merge(b, o, t).is_none());
    }

    /// Theirs puts a `throw` in front of a call ours edited the arguments
    /// of. The composition is the developer's own line — and it is here
    /// because the first version of the splice emitted the inserted run's
    /// leading gap AND base's gap for the token behind it, and wrote
    /// `throw    FUNCAST` with theirs' four spaces of indentation in the
    /// middle of the line. Nobody wrote that. One gap per token, always.
    #[test]
    fn an_insertion_does_not_double_the_gap_at_its_seam() {
        let b = "    FUNCAST.thrw(ii, Type.BLN, str);\n";
        let o = "    FUNCAST.thrw(ii, AtomType.BLN, str);\n";
        let t = "    throw FUNCAST.thrw(ii, Type.BLN, str);\n";
        let c = expression_merge(b, o, t).expect("composes");
        assert_eq!(c.text, "    throw FUNCAST.thrw(ii, AtomType.BLN, str);\n");
    }

    /// The same triple gives the same bytes, every time.
    #[test]
    fn the_expression_merge_is_a_function_of_its_inputs() {
        let b = "    return (var.type != null ? var.type.cast(v, ctx, input) : v).iter(ctx);\n";
        let o = "    return (var.type != null ? var.type.cast(v, ctx, input) : v).iter();\n";
        let t = "    return (ret != null ? ret.cast(v, ctx, input) : v).iter(ctx);\n";
        let first = expression_merge(b, o, t).map(|c| c.text);
        for _ in 0..24 {
            assert_eq!(first, expression_merge(b, o, t).map(|c| c.text));
        }
        // and symmetric in the two sides, up to which edit lands where
        let swapped = expression_merge(b, t, o).map(|c| c.text);
        assert_eq!(first, swapped, "composition depends on which side is ours");
    }

    /// The load-bearing claim this module makes: every output byte is a
    /// contiguous substring of one of the three inputs.
    #[test]
    fn every_composed_token_comes_from_some_side() {
        let b = "    return f(a, b, c);\n";
        let o = "    return f(a2, b, c);\n";
        let t = "    return f(a, b, c2);\n";
        let c = expression_merge(b, o, t).expect("composes");
        let toks = tokenize(&c.text).unwrap();
        for t2 in &toks {
            let s = tok_text(&c.text, t2);
            assert!(
                b.contains(s) || o.contains(s) || t.contains(s),
                "token {s:?} came from nowhere"
            );
        }
    }
}
