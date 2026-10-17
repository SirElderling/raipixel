"""C21 scratch-only metered LZ history, deep match traversal and lazy parsing.

Instrument compression attempts with per-loop Work::charge accounting, on top
of C20 transactional output and C19 fallible reusable buffers. Charge actual
hash chain probes and a conservative upper bound for byte comparisons, lazy
lookahead, rebuilt history, repeated-byte scans and adaptive row scoring.
Assert tight budget (one unit short) produces E_LIMIT with zero caller bytes,
exact consumed budget succeeds, and frozen PNG sizes remain unchanged.
This experiment does not yet impose one shared selection budget across six
separately compiled scratch crates nor certify peak allocations. No src edits.
"""
import binascii
import zlib
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ENCODER = (ROOT / "src/deflate/encode.rs").read_text(encoding="utf-8")
PNG = (ROOT / "src/png.rs").read_text(encoding="utf-8")

HISTORY = r'''
impl Encoder {
    // Scratch only: retain 32 KiB of uncompressed input history across
    // consecutive DEFLATE blocks; encode the current block only.
    pub(crate) fn block_history(&mut self, data: &[u8], last: bool) -> &[u8] {
        const DEPTH: usize = TRIAL_DEPTH;
        debug_assert!(data.len() <= BLOCK);
        let mut joined = Vec::with_capacity(self.history.len() + data.len());
        joined.extend_from_slice(&self.history);
        joined.extend_from_slice(data);
        let start = self.history.len();
        let mut heads = vec![u32::MAX; HASH_SIZE];
        let mut links = vec![u32::MAX; joined.len()];
        for pos in 0..start {
            if pos + 3 > joined.len() { break; }
            let (slot, _) = hash(&joined, pos);
            links[pos] = heads[slot];
            heads[slot] = pos as u32;
        }
        let saved = (self.bits, self.count);
        self.output.clear();
        self.put(u32::from(last) | 2, 3);
        let mut pos = start;
        while pos + 3 <= joined.len() {
            let (slot, _) = hash(&joined, pos);
            let maximum = (joined.len() - pos).min(258);
            let mut best = 0usize;
            let mut best_pos = 0usize;
            let mut prior = heads[slot];
            for _ in 0..DEPTH {
                if prior == u32::MAX { break; }
                let candidate = prior as usize;
                if pos - candidate > 32_768 { break; }
                if joined[candidate..candidate + 3] == joined[pos..pos + 3] {
                    let length = 3 + matching_tail(
                        &joined[candidate + 3..candidate + maximum],
                        &joined[pos + 3..pos + maximum],
                    );
                    if length > best {
                        best = length;
                        best_pos = candidate;
                        if length == maximum { break; }
                    }
                }
                prior = links[candidate];
            }
            links[pos] = heads[slot];
            heads[slot] = pos as u32;
            if pos > 0 && joined[pos] == joined[pos - 1] {
                let length = repeated_length(&joined, pos - 1, pos, maximum);
                if length >= best {
                    best = length;
                    best_pos = pos - 1;
                }
            }
            if best >= 3 {
                self.reference(best, pos - best_pos);
                let end = pos + best;
                pos += 1;
                while pos < end {
                    if pos + 3 <= joined.len() {
                        let (slot, _) = hash(&joined, pos);
                        links[pos] = heads[slot];
                        heads[slot] = pos as u32;
                    }
                    pos += 1;
                }
            } else {
                self.symbol(u32::from(joined[pos]));
                pos += 1;
            }
        }
        for &byte in &joined[pos..] {
            self.symbol(u32::from(byte));
        }
        self.symbol(256);
        if last { self.align(); }
        self.flush_bytes();
        if self.output.len() > data.len() + 5 {
            (self.bits, self.count) = saved;
            self.store(data, last);
        }
        // The decompressor sees stored and Huffman-coded bytes equally:
        // either form contributes to the same sliding history.
        let keep = joined.len().min(32_768);
        self.history.clear();
        self.history.extend_from_slice(&joined[joined.len() - keep..]);
        &self.output
    }
}
'''

def replace_once(text, old, new):
    count = text.count(old)
    if count != 1:
        raise ValueError(f"unstable source anchor {old[:80]!r}, found {count}")
    return text.replace(old, new)

def candidate(depth, dynamic=True, filter_mode="none", parse_mode="longest"):
    encoder = replace_once(
        ENCODER, "    dictionary: Vec<u32>,\n", "    dictionary: Vec<u32>,\n    history: Vec<u8>,\n"
    )
    encoder = replace_once(
        encoder, "            dictionary,\n            output,", "            dictionary,\n            history: Vec::new(),\n            output,"
    )
    encoder += "\n" + HISTORY.replace("TRIAL_DEPTH", str(depth))
    if dynamic:
        # Import only our own vetted scratch RFC 1951 Huffman routines,
        # never any implementation code from the reference png crate.
        trial_source = (ROOT / "tools/png_dynamic_indexed_guard_trial.py").read_text(encoding="utf-8")
        helper = trial_source.split("HELPER=r'''", 1)[1].split("\n'''", 1)[0]
        start = encoder.index("    pub(crate) fn block_history(")
        unchanged, method = encoder[:start], encoder[start:]
        method = replace_once(method,
            "        let saved = (self.bits, self.count);\n        self.output.clear();\n        self.put(u32::from(last) | 2, 3);",
            "        let saved = (self.bits, self.count);\n        let mut tokens = Vec::<u16>::new();\n        self.output.clear();")
        method = replace_once(method,
            "                self.reference(best, pos - best_pos);",
            "                tokens.push(256 + (best - 3) as u16);\n                tokens.push((pos - best_pos) as u16);")
        method = replace_once(method,
            "                self.symbol(u32::from(joined[pos]));",
            "                tokens.push(u16::from(joined[pos]));")
        method = replace_once(method,
            "        for &byte in &joined[pos..] {\n            self.symbol(u32::from(byte));\n        }",
            "        for &byte in &joined[pos..] { tokens.push(u16::from(byte)); }")
        method = replace_once(method,
            """        self.symbol(256);
        if last { self.align(); }
        self.flush_bytes();
        if self.output.len() > data.len() + 5 {
            (self.bits, self.count) = saved;
            self.store(data, last);
        }""",
            """        let fixed_bits = trial_fixed_bits(&tokens);
        let fixed_bytes = ((saved.1 + fixed_bits + if last { 7 } else { 0 }) / 8) as usize;
        // Skip all tree work for tiny outputs and fall back if no savings.
        let dynamic_ok = data.len() >= 4096 && self.trial_emit_dynamic(&tokens, last);
        if !dynamic_ok || self.output.len() >= fixed_bytes.min(data.len() + 5) {
            self.output.clear();
            (self.bits, self.count) = saved;
            if fixed_bytes > data.len() + 5 {
                self.store(data, last);
            } else {
                self.put(u32::from(last) | 2, 3);
                for token in trial_unpack(&tokens) {
                    match token {
                        TrialToken::Literal(value) => self.symbol(u32::from(value)),
                        TrialToken::Reference(length, distance) => self.reference(length, distance),
                    }
                }
                self.symbol(256);
                if last { self.align(); }
                self.flush_bytes();
            }
        }""")
        encoder = unchanged + method + "\n" + helper
    if parse_mode not in ("longest", "cost", "lazy", "costlazy"):
        raise ValueError("unknown parser mode")
    if parse_mode != "longest":
        front, marker, body = encoder.partition("    pub(crate) fn block_history(")
        if not marker:
            raise ValueError("history method missing")
        if parse_mode in ("cost", "costlazy"):
            body = replace_once(body,
                "            let mut best_pos = 0usize;",
                "            let mut best_pos = 0usize;\n            let mut best_score = 0i64;")
            body = replace_once(body,
                """                    if length > best {
                        best = length;
                        best_pos = candidate;
                        if length == maximum { break; }
                    }""",
                """                    let score = trial_match_score(length, pos - candidate);
                    if score > best_score {
                        best_score = score;
                        best = length;
                        best_pos = candidate;
                    }""")
            body = replace_once(body,
                """                if length >= best {
                    best = length;
                    best_pos = pos - 1;
                }""",
                """                let score = trial_match_score(length, 1);
                if score >= best_score && length >= 3 {
                    best_score = score;
                    best = length;
                    best_pos = pos - 1;
                }""")
            body += """
// Approximate avoided fixed-Huffman literal bits minus actual RFC 1951
// length/distance token bits. The Huffman encoder still optimizes final
// frequencies after tokenization. Score only: output size is authoritative.
fn trial_match_score(length: usize, distance: usize) -> i64 {
    if length < 3 { return -1; }
    let lindex = if length == 258 { 28 } else { usize::from(LENGTH_CODES[length-3]) };
    let dindex = distance_code(distance);
    let lbits = i64::from(FIXED_CODES[257+lindex] >> 9);
    (length as i64)*8 - lbits - i64::from(LENGTH_EXTRA[lindex])
         - 5 - i64::from(DISTANCE_EXTRA[dindex])
}
"""
        if parse_mode in ("lazy", "costlazy"):
            body = replace_once(body,
                "            if best >= 3 {\n                tokens.push(",
                """            if best >= 3 && best < 258 && pos + 4 <= joined.len() {
                let next = pos + 1;
                let (next_slot, _) = hash(&joined, next);
                let mut node = heads[next_slot];
                let mut next_best = 0usize;
                let max_next = (joined.len() - next).min(258);
                for _ in 0..DEPTH {
                    if node == u32::MAX { break; }
                    let candidate = node as usize;
                    if next - candidate > 32_768 { break; }
                    if joined[candidate..candidate+3] == joined[next..next+3] {
                        let length = 3 + matching_tail(
                            &joined[candidate+3..candidate+max_next],
                            &joined[next+3..next+max_next],
                        );
                        next_best = next_best.max(length);
                    }
                    node = links[candidate];
                }
                // One extra literal may permit a longer next match.
                if next_best > best + 1 {
                    tokens.push(u16::from(joined[pos]));
                    pos += 1;
                    continue;
                }
            }
            if best >= 3 {
                tokens.push(""")
        encoder = front + marker + body
    png = replace_once(
        PNG,
        "let compressed = encoder.block_compact(&raw, offset + len == raw_size);",
        "let compressed = encoder.block_history(&raw, offset + len == raw_size);"
    )
    if filter_mode != "none":
        if filter_mode not in ("sub", "up", "paeth", "auto64", "autoall"):
            raise ValueError("unsupported filter")
        # These are indexed packed samples: PNG bytes-per-pixel is always 1.
        # Original packed bytes are saved row-by-row before prediction, even
        # when a DEFLATE block boundary splits a scanline. Scope anchors to
        # the indexed writer, not the later RGBA encode_filtered writer.
        before, separator, after = png.partition("\nfn encode_filtered<")
        if not separator:
            raise ValueError("PNG indexed writer anchor missing")
        png = before
        png = replace_once(
            png, "    let mut offset = 0;\n    let mut adler = Adler::new();",
            """    let mut offset = 0;
    let mut adler = Adler::new();
    let mut prev_row = vec![0u8; row_bytes];
    let mut cur_row = vec![0u8; row_bytes];
    let mut left_pixel = 0u8;""")
        if filter_mode.startswith("auto"):
            sample_max = 64 if filter_mode == "auto64" else 4096
            png = replace_once(
                png,
                "    let mut left_pixel = 0u8;",
                "    let mut left_pixel = 0u8;\n    let mut chosen_filter = 0u8;"
            )
            transform = """        for i in 0..raw.len() {
            let pos = offset + i;
            let col = pos % stride;
            if col == 0 {
                if pos != 0 { std::mem::swap(&mut prev_row, &mut cur_row); }
                left_pixel = 0;
                let sample_count = row_bytes.min(raw.len() - i - 1).min(SAMPLE_MAX);
                let mut best_score = u64::MAX;
                let mut best_filter = 0u8;
                for filter in [0u8, 1, 2, 4] {
                    let mut score = 0u64;
                    for j in 0..sample_count {
                        let original = raw[i + j + 1];
                        let left = if j != 0 { raw[i + j] } else { 0 };
                        let above = prev_row[j];
                        let upper_left = if j != 0 { prev_row[j - 1] } else { 0 };
                        let pred = match filter {
                            0 => 0,
                            1 => left,
                            2 => above,
                            _ => paeth(left, above, upper_left),
                        };
                        let residual = original.wrapping_sub(pred);
                        // Signed magnitude is a cheap, deterministic proxy.
                        // It is not actual DEFLATE bit cost.
                        let absolute = if residual > 127 {
                            256u64 - u64::from(residual)
                        } else {
                            u64::from(residual)
                        };
                        score += absolute;
                    }
                    // Stable first-filter tie rule: 0 > 1 > 2 > 4.
                    if score < best_score {
                        best_score = score;
                        best_filter = filter;
                    }
                }
                chosen_filter = best_filter;
                raw[i] = chosen_filter;
            } else {
                let original = raw[i];
                let above = prev_row[col - 1];
                let upper_left = if col > 1 { prev_row[col - 2] } else { 0 };
                let pred = match chosen_filter {
                    0 => 0,
                    1 => left_pixel,
                    2 => above,
                    4 => paeth(left_pixel, above, upper_left),
                    _ => unreachable!(),
                };
                raw[i] = original.wrapping_sub(pred);
                cur_row[col - 1] = original;
                left_pixel = original;
            }
        }
        adler.update(&raw);""".replace("SAMPLE_MAX", str(sample_max))
            png = replace_once(png, "        adler.update(&raw);", transform)
        else:
            marker = {"sub": 1, "up": 2, "paeth": 4}[filter_mode]
            png = replace_once(
                png, "        adler.update(&raw);",
                """        for (i, byte) in raw.iter_mut().enumerate() {
                let pos = offset + i;
                let col = pos % stride;
                if col == 0 {
                    if pos != 0 { std::mem::swap(&mut prev_row, &mut cur_row); }
                    left_pixel = 0;
                    *byte = FILTER_MARKER;
                } else {
                    let original = *byte;
                    let above = prev_row[col - 1];
                    let above_left = if col > 1 { prev_row[col - 2] } else { 0 };
                    let predictor = match FILTER_MARKER {
                        1 => left_pixel,
                        2 => above,
                        4 => paeth(left_pixel, above, above_left),
                        _ => unreachable!(),
                    };
                    *byte = original.wrapping_sub(predictor);
                    cur_row[col - 1] = original;
                    left_pixel = original;
                }
            }
            adler.update(&raw);""".replace("FILTER_MARKER", str(marker)))
        png += separator + after
    return encoder, png


def make_fallible(encoder,png):
    """Fallible scratch buffers: retain codec decisions; no production edits.

    This does not fix Work billing, dynamic bitwriter sizing, output staging,
    or the per-block allocation churn. Do not promote this as fully bounded.
    """
    def one(text,old,new):
        if text.count(old)!=1:
            raise ValueError(f"fallible anchor changed: {old[:90]!r} ({text.count(old)})")
        return text.replace(old,new)

    encoder=one(encoder,
        "        Ok(Self {\n            dictionary,\n            history: Vec::new(),",
        """        let mut history = Vec::new();
        history.try_reserve_exact(32_768).map_err(|_| Error::Allocation)?;
        Ok(Self {
            dictionary,
            history,""")
    encoder=one(encoder,
        "    pub(crate) fn block_history(&mut self, data: &[u8], last: bool) -> &[u8] {",
        "    pub(crate) fn block_history(&mut self, data: &[u8], last: bool) -> Result<&[u8], Error> {")
    encoder=one(encoder,
        """        let mut joined = Vec::with_capacity(self.history.len() + data.len());
        joined.extend_from_slice(&self.history);""",
        """        let mut joined = Vec::new();
        joined.try_reserve_exact(self.history.len() + data.len())
            .map_err(|_| Error::Allocation)?;
        joined.extend_from_slice(&self.history);""")
    encoder=one(encoder,
        """        let mut heads = vec![u32::MAX; HASH_SIZE];
        let mut links = vec![u32::MAX; joined.len()];""",
        """        let mut heads = Vec::new();
        heads.try_reserve_exact(HASH_SIZE).map_err(|_| Error::Allocation)?;
        heads.resize(HASH_SIZE, u32::MAX);
        let mut links = Vec::new();
        links.try_reserve_exact(joined.len()).map_err(|_| Error::Allocation)?;
        links.resize(joined.len(), u32::MAX);""")
    encoder=one(encoder,
        "        let mut tokens = Vec::<u16>::new();",
        """        let mut tokens = Vec::<u16>::new();
        // At most one token word per new raw byte: references take two
        // words for at least three consumed source bytes.
        tokens.try_reserve_exact(data.len()).map_err(|_| Error::Allocation)?;""")
    encoder=one(encoder,
        """        self.history.extend_from_slice(&joined[joined.len() - keep..]);
        &self.output""",
        """        self.history.extend_from_slice(&joined[joined.len() - keep..]);
        Ok(&self.output)""")
    encoder=one(encoder,
        "    let mut nodes=Vec::<TrialNode>::new();",
        """    let mut nodes=Vec::<TrialNode>::new();
    nodes.try_reserve_exact(freq.len().saturating_mul(2)).ok()?;""")
    encoder=one(encoder,
        "    let mut queue=BinaryHeap::<Reverse<(u32,usize)>>::new();",
        """    let mut queue=BinaryHeap::<Reverse<(u32,usize)>>::new();
    queue.try_reserve(freq.len()).ok()?;""")
    encoder=one(encoder,
        "    let mut lengths=vec![0u8;freq.len()];",
        """    let mut lengths=Vec::new();
    lengths.try_reserve_exact(freq.len()).ok()?;
    lengths.resize(freq.len(),0u8);""")
    encoder=one(encoder,
        "    let mut stack=vec![(root,0u8)];",
        """    let mut stack=Vec::new();
    stack.try_reserve_exact(nodes.len()).ok()?;
    stack.push((root,0u8));""")
    encoder=one(encoder,
        "fn trial_codes(lengths:&[u8])->Vec<(u16,u8)> {",
        "fn trial_codes(lengths:&[u8])->Option<Vec<(u16,u8)>> {")
    encoder=one(encoder,
        "    let mut result=vec![(0u16,0u8);lengths.len()];",
        """    let mut result=Vec::new();
    result.try_reserve_exact(lengths.len()).ok()?;
    result.resize(lengths.len(),(0u16,0u8));""")
    encoder=one(encoder,
        """    result
}
fn trial_rle(lengths:&[u8])->Vec<(usize,u32,u32)> {""",
        """    Some(result)
}
fn trial_rle(lengths:&[u8])->Option<Vec<(usize,u32,u32)>> {""")
    encoder=one(encoder,
        """fn trial_rle(lengths:&[u8])->Option<Vec<(usize,u32,u32)>> {
    let mut output=Vec::new();""",
        """fn trial_rle(lengths:&[u8])->Option<Vec<(usize,u32,u32)>> {
    let mut output=Vec::new();
    output.try_reserve_exact(lengths.len()).ok()?;""")
    encoder=one(encoder,
        """    output
}
// Fixed-code bit cost:""",
        """    Some(output)
}
// Fixed-code bit cost:""")
    encoder=one(encoder,
        """        let mut together=lens[..ll_count].to_vec();
        together.extend_from_slice(&dist_lens[..ds_count]);
        let runs=trial_rle(&together);""",
        """        let mut together=Vec::new();
        if together.try_reserve_exact(ll_count+ds_count).is_err() {return false;}
        together.extend_from_slice(&lens[..ll_count]);
        together.extend_from_slice(&dist_lens[..ds_count]);
        let Some(runs)=trial_rle(&together) else{return false;};""")
    encoder=one(encoder,
        """        let lc=trial_codes(&lens);
        let dc=trial_codes(&dist_lens);
        let cc=trial_codes(&code_lens);""",
        """        let Some(lc)=trial_codes(&lens) else{return false;};
        let Some(dc)=trial_codes(&dist_lens) else{return false;};
        let Some(cc)=trial_codes(&code_lens) else{return false;};""")
    png=one(png,
        "let compressed = encoder.block_history(&raw, offset + len == raw_size);",
        "let compressed = encoder.block_history(&raw, offset + len == raw_size)?;")
    if "    let mut prev_row = vec![0u8; row_bytes];" in png:
        png=one(png,
            """    let mut prev_row = vec![0u8; row_bytes];
    let mut cur_row = vec![0u8; row_bytes];""",
            """    let mut prev_row = Vec::new();
    prev_row.try_reserve_exact(row_bytes).map_err(|_| crate::Error::Allocation)?;
    prev_row.resize(row_bytes,0u8);
    let mut cur_row = Vec::new();
    cur_row.try_reserve_exact(row_bytes).map_err(|_| crate::Error::Allocation)?;
    cur_row.resize(row_bytes,0u8);""")
    return encoder,png


def make_reusable(encoder):
    """Own reusable block capacities in the encoder; no unsafe or dependencies.

    This retains per-block Huffman-tree small allocations. Match-work charging
    and transactionally staged final PNG output remain deliberately unmodified.
    """
    def one(old,new):
        nonlocal encoder
        count=encoder.count(old)
        if count!=1:
            raise ValueError(f"workspace anchor count {count}: {old[:90]!r}")
        encoder=encoder.replace(old,new)
    one("    history: Vec<u8>,\n",
        """    history: Vec<u8>,
    trial_joined: Vec<u8>,
    trial_heads: Vec<u32>,
    trial_links: Vec<u32>,
    trial_tokens: Vec<u16>,
""")
    one("        Ok(Self {\n            dictionary,\n            history,",
        """        // One set of block scratch allocations, reused for all IDAT blocks.
        // Because new(max_block) is bounded by DEFLATE's 65,535 bytes,
        // every joined history+block can fit these preflighted capacities.
        let joined_capacity = max_block.checked_add(32_768)
            .ok_or(Error::Allocation)?;
        let mut trial_joined = Vec::new();
        trial_joined.try_reserve_exact(joined_capacity)
            .map_err(|_| Error::Allocation)?;
        let mut trial_heads = Vec::new();
        trial_heads.try_reserve_exact(HASH_SIZE)
            .map_err(|_| Error::Allocation)?;
        let mut trial_links = Vec::new();
        trial_links.try_reserve_exact(joined_capacity)
            .map_err(|_| Error::Allocation)?;
        let mut trial_tokens = Vec::new();
        trial_tokens.try_reserve_exact(max_block)
            .map_err(|_| Error::Allocation)?;
        Ok(Self {
            dictionary,
            history,
            trial_joined,
            trial_heads,
            trial_links,
            trial_tokens,""")
    one("""        let mut joined = Vec::new();
        joined.try_reserve_exact(self.history.len() + data.len())
            .map_err(|_| Error::Allocation)?;
        joined.extend_from_slice(&self.history);""",
        """        let mut joined = std::mem::take(&mut self.trial_joined);
        joined.clear();
        // Guaranteed by preallocation: history <= 32 KiB and data <= max_block.
        assert!(joined.capacity() >= self.history.len() + data.len());
        joined.extend_from_slice(&self.history);""")
    one("""        let mut heads = Vec::new();
        heads.try_reserve_exact(HASH_SIZE).map_err(|_| Error::Allocation)?;
        heads.resize(HASH_SIZE, u32::MAX);
        let mut links = Vec::new();
        links.try_reserve_exact(joined.len()).map_err(|_| Error::Allocation)?;
        links.resize(joined.len(), u32::MAX);""",
        """        let mut heads = std::mem::take(&mut self.trial_heads);
        heads.clear();
        assert!(heads.capacity() >= HASH_SIZE);
        heads.resize(HASH_SIZE, u32::MAX);
        let mut links = std::mem::take(&mut self.trial_links);
        links.clear();
        assert!(links.capacity() >= joined.len());
        links.resize(joined.len(), u32::MAX);""")
    one("""        let mut tokens = Vec::<u16>::new();
        // At most one token word per new raw byte: references take two
        // words for at least three consumed source bytes.
        tokens.try_reserve_exact(data.len()).map_err(|_| Error::Allocation)?;""",
        """        let mut tokens = std::mem::take(&mut self.trial_tokens);
        tokens.clear();
        // A match takes two words for >=3 source bytes; literals take one.
        assert!(tokens.capacity() >= data.len());""")
    one("""        self.history.extend_from_slice(&joined[joined.len() - keep..]);
        Ok(&self.output)""",
        """        self.history.extend_from_slice(&joined[joined.len() - keep..]);
        // Return workspaces to the encoder instead of freeing them each block.
        // No additional capacity growth is needed on any subsequent block.
        self.trial_joined = joined;
        self.trial_heads = heads;
        self.trial_links = links;
        self.trial_tokens = tokens;
        Ok(&self.output)""")
    return encoder



def make_metered(encoder,png):
    """Incremental Work accounting tied to matcher behavior, not fixed image size.

    Up to 258 bytes may be inspected by one match candidate; charge lengths
    + 16 pessimistically for vectorized 16-byte groups. Charge 4 per hash
    candidate and one per rebuilt history slot/position. Failed work charges
    propagate to C20's transactionally staged sink and cannot publish bytes.
    """
    def one(text,old,new):
        count=text.count(old)
        if count!=1:
            raise ValueError(f"work-metering anchor count {count}: {old[:90]!r}")
        return text.replace(old,new)

    encoder=one(encoder,
        "pub(crate) fn block_history(&mut self, data: &[u8], last: bool) -> Result<&[u8], Error>",
        "pub(crate) fn block_history(&mut self, data: &[u8], last: bool, work: &mut crate::Work) -> Result<&[u8], Error>")
    encoder=one(encoder,
        "        for pos in 0..start {\n",
        "        work.charge(start as u64)?;\n        for pos in 0..start {\n")
    encoder=one(encoder,
        "        while pos + 3 <= joined.len() {\n",
        "        while pos + 3 <= joined.len() {\n            work.charge(4)?;\n")
    encoder=one(encoder,
        "                if joined[candidate..candidate + 3] == joined[pos..pos + 3] {\n",
        "                work.charge(4)?;\n                if joined[candidate..candidate + 3] == joined[pos..pos + 3] {\n")
    encoder=one(encoder,
        """                        &joined[pos + 3..pos + maximum],
                    );
                    if length > best {""",
        """                        &joined[pos + 3..pos + maximum],
                    );
                    // Up to one extra vectorized 16-byte probe may be checked.
                    work.charge((length + 16) as u64)?;
                    if length > best {""")
    encoder=one(encoder,
        """            if pos > 0 && joined[pos] == joined[pos - 1] {
                let length = repeated_length""",
        """            work.charge(1)?;
            if pos > 0 && joined[pos] == joined[pos - 1] {
                // Bound byte scanning before any search; may overcharge.
                work.charge(maximum as u64)?;
                let length = repeated_length""")
    encoder=one(encoder,
        """                let end = pos + best;
                pos += 1;""",
        """                let end = pos + best;
                // Updating hashes for interior match bytes also consumes work.
                work.charge((best - 1) as u64)?;
                pos += 1;""")
    if "if best >= 3 && best < 258 && pos + 4 <= joined.len()" in encoder:
        encoder=one(encoder,
            """            if best >= 3 && best < 258 && pos + 4 <= joined.len() {
                    let next = pos + 1;""",
            """            if best >= 3 && best < 258 && pos + 4 <= joined.len() {
                    work.charge(4)?;
                    let next = pos + 1;""")
        encoder=one(encoder,
            """                    if joined[candidate..candidate+3] == joined[next..next+3] {""",
            """                    work.charge(4)?;
                        if joined[candidate..candidate+3] == joined[next..next+3] {""")
        encoder=one(encoder,
            """                            &joined[next+3..next+max_next],
                            );
                            next_best = next_best.max(length);""",
            """                            &joined[next+3..next+max_next],
                            );
                            work.charge((length + 16) as u64)?;
                            next_best = next_best.max(length);""")
    png=one(png,
        "encoder.block_history(&raw, offset + len == raw_size)?",
        "encoder.block_history(&raw, offset + len == raw_size, work)?")
    if "let sample_count = row_bytes.min(raw.len() - i - 1).min(" in png:
        # One per filter (None/Sub/Up/Paeth), per sampled byte. Within Work.
        marker="                let mut best_score = u64::MAX;"
        png=one(png,marker,
            "                work.charge((sample_count as u64) * 4)?;\n"+marker)
    return encoder,png

STAGER = r'''
/// Scratch-only transactional staging. A logical byte cap is NOT a proven
/// allocator-capacity/peak heap bound; the Vec allocator may over-allocate.
struct TrialStage {
    bytes: Vec<u8>,
    max_bytes: usize,
    allocation_failed: bool,
    limit_failed: bool,
}
impl std::io::Write for TrialStage {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let fits = self.bytes.len().checked_add(bytes.len())
            .is_some_and(|total| total <= self.max_bytes);
        if !fits {
            self.limit_failed = true;
            return Err(std::io::Error::other("staged PNG exceeds output byte limit"));
        }
        if self.bytes.try_reserve(bytes.len()).is_err() {
            self.allocation_failed = true;
            return Err(std::io::Error::other("fallible staging allocation failed"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}
pub fn trial_encode_indexed_staged(
    image: &Image,
    output: &mut impl std::io::Write,
    work: &mut Work,
) -> Result<bool, Error> {
    trial_encode_indexed_staged_limited(image,output,work,usize::MAX)
}
/// Testing hook for budget-failure injection; no production API contract.
pub fn trial_encode_indexed_staged_limited(
    image: &Image,
    output: &mut impl std::io::Write,
    work: &mut Work,
    requested_max: usize,
) -> Result<bool, Error> {
    // Conservative cap: 8-bit exact palette worst-case packed rows,
    // stored DEFLATE block overhead, PNG palette/tRNS/chunks and margin.
    let stride=(image.width as usize).checked_add(1)
        .ok_or(Error::Resource(crate::Error::Limit(crate::Resource::Bytes)))?;
    let raw=stride.checked_mul(image.height as usize)
        .ok_or(Error::Resource(crate::Error::Limit(crate::Resource::Bytes)))?;
    let blocks=raw.div_ceil(crate::deflate::BLOCK);
    let logical_max=blocks.checked_mul(17)
        .and_then(|n| n.checked_add(raw))
        .and_then(|n| n.checked_add(2048))
        .ok_or(Error::Resource(crate::Error::Limit(crate::Resource::Bytes)))?;
    let mut staged=TrialStage {
        bytes:Vec::new(),
        max_bytes:logical_max.min(requested_max),
        allocation_failed:false,
        limit_failed:false,
    };
    let supported=match encode_indexed(image,&mut staged,work) {
        Ok(value)=>value,
        Err(error) => {
            if staged.allocation_failed {
                return Err(Error::Resource(crate::Error::Allocation));
            }
            if staged.limit_failed {
                return Err(Error::Resource(crate::Error::Limit(crate::Resource::Bytes)));
            }
            return Err(error);
        }
    };
    if supported { output.write_all(&staged.bytes)?; }
    Ok(supported)
}
'''
def make_staged_png(png):
    # Append beside trial codec in scratch copied src/png.rs only.
    if "trial_encode_indexed_staged(" in png:
        raise ValueError("staged API duplicated")
    return png + "\n" + STAGER + "\n"

DRIVER = r'''
#![forbid(unsafe_code)]
use std::{hint::black_box, io::Cursor, time::{Duration, Instant}};
type E = Box<dyn std::error::Error>;
fn reference_rgba(data:&[u8])->Result<(u32,u32,Vec<u8>),E>{
    let mut decoder=png::Decoder::new(Cursor::new(data));
    decoder.set_transformations(png::Transformations::EXPAND);
    decoder.ignore_checksums(false);
    decoder.set_ignore_text_chunk(true);
    decoder.set_ignore_iccp_chunk(true);
    let mut reader=decoder.read_info()?;
    let mut output=vec![0u8;reader.output_buffer_size().ok_or("reference decode size")?];
    let info=reader.next_frame(&mut output)?;
    output.truncate(info.buffer_size());
    reader.finish()?;
    let rgba=match info.color_type {
        png::ColorType::Rgba => output,
        png::ColorType::Rgb => output.chunks_exact(3)
            .flat_map(|x|[x[0],x[1],x[2],255]).collect(),
        _ => return Err("reference decoder did not expand exact palette".into()),
    };
    Ok((info.width,info.height,rgba))
}
fn main()->Result<(),E>{
    for path in std::env::args().skip(1) {
        let source=std::fs::read(&path)?;
        let base=raipixel::png::decode(&source,raipixel::Limits::default(),&mut raipixel::Work::default())?.image;
        let [width,height]=base.size();
        if path.contains("indexed-case-") || path.contains("art-") {
            let expected=std::fs::read(format!("{path}.rgba"))?;
            assert_eq!(base.pixels().as_flattened(),expected.as_slice(),"independent input RGBA mismatch");
        }
        let a=c64flat::Image::from_pixels(width,height,base.pixels(),c64flat::Limits::default(),&mut c64flat::Work::default())?;
        let b=c32sub::Image::from_pixels(width,height,base.pixels(),c32sub::Limits::default(),&mut c32sub::Work::default())?;
        let c=c64lazy::Image::from_pixels(width,height,base.pixels(),c64lazy::Limits::default(),&mut c64lazy::Work::default())?;
        let d=c64lazysub::Image::from_pixels(width,height,base.pixels(),c64lazysub::Limits::default(),&mut c64lazysub::Work::default())?;
        let e=c64auto64::Image::from_pixels(width,height,base.pixels(),c64auto64::Limits::default(),&mut c64auto64::Work::default())?;
        let f=c64autoall::Image::from_pixels(width,height,base.pixels(),c64autoall::Limits::default(),&mut c64autoall::Work::default())?;
        let mut outputs:[Vec<u8>;7]=std::array::from_fn(|_|Vec::new());
        let mut supports=[false;7];
        let mut budgets=[0u64;7];
        for mode in 0..7 {
            let (supported,remaining)=match mode {
                0=>{let mut w=raipixel::Work::default();let s=raipixel::png::encode_indexed(&base,&mut outputs[mode],&mut w)?;(s,w.remaining())},
                1=>{let mut w=c64flat::Work::default();let s=c64flat::png::trial_encode_indexed_staged(&a,&mut outputs[mode],&mut w)?;(s,w.remaining())},
                2=>{let mut w=c32sub::Work::default();let s=c32sub::png::trial_encode_indexed_staged(&b,&mut outputs[mode],&mut w)?;(s,w.remaining())},
                3=>{let mut w=c64lazy::Work::default();let s=c64lazy::png::trial_encode_indexed_staged(&c,&mut outputs[mode],&mut w)?;(s,w.remaining())},
                4=>{let mut w=c64lazysub::Work::default();let s=c64lazysub::png::trial_encode_indexed_staged(&d,&mut outputs[mode],&mut w)?;(s,w.remaining())},
                5=>{let mut w=c64auto64::Work::default();let s=c64auto64::png::trial_encode_indexed_staged(&e,&mut outputs[mode],&mut w)?;(s,w.remaining())},
                _=>{let mut w=c64autoall::Work::default();let s=c64autoall::png::trial_encode_indexed_staged(&f,&mut outputs[mode],&mut w)?;(s,w.remaining())}
            };
            supports[mode]=supported;budgets[mode]=remaining;
        }
        assert!(supports.iter().all(|s|*s==supports[0]),"palette support mismatch");
        // Production accepted writer does not bill scratch's deeper search.
        // Each candidate must bill more than accepted-indexed on input that
        // exercises matching; no longer assert equality of Work budgets.
        assert!(budgets[1..].iter().all(|&value|value < budgets[0]),
                "one or more scratch candidates still underbill matcher Work");
        if !supports[0] {
            assert!(outputs.iter().all(Vec::is_empty),"unsupported wrote bytes");

            let mut original=vec![0xD3_u8];
            let result=c64auto64::png::trial_encode_indexed_staged(
                &e,&mut original,&mut c64auto64::Work::default())?;
            assert!(!result,"unsupported RGBA palette falsely accepted");
            assert_eq!(original,vec![0xD3_u8],"unsupported palette wrote output");
            println!("{{\"supported\":false,\"width\":{},\"height\":{}}}",width,height);
            continue;
        }

        // A resource error must not publish even the first PNG signature byte.
        let mut untouched=vec![0xA5_u8];
        let mut exhausted=c64auto64::Work::new(0)?;
        assert!(c64auto64::png::trial_encode_indexed_staged(
            &e,&mut untouched,&mut exhausted).is_err());
        assert_eq!(untouched,vec![0xA5_u8],"low work budget wrote output");

        // Explicit output cap failure must remain internal to the staging sink.
        let mut capped=vec![0xB6_u8];
        assert!(c64auto64::png::trial_encode_indexed_staged_limited(
            &e,&mut capped,&mut c64auto64::Work::default(),1).is_err());
        assert_eq!(capped,vec![0xB6_u8],"staging output cap wrote output");

        // A genuine caller I/O failure, by contrast, occurs only on commit.
        struct RejectWriter { calls:usize }
        impl std::io::Write for RejectWriter {
            fn write(&mut self,_:&[u8])->std::io::Result<usize>{
                self.calls+=1;
                Err(std::io::Error::other("intentional commit sink failure"))
            }
            fn flush(&mut self)->std::io::Result<()>{Ok(())}
        }
        let mut reject=RejectWriter{calls:0};
        let result=c64auto64::png::trial_encode_indexed_staged(
            &e,&mut reject,&mut c64auto64::Work::default());
        assert!(result.is_err(),"caller sink must fail");
        assert_eq!(reject.calls,1,"commit must occur once after full encode");

        // Exact consumption and one-unit-too-tight failure: same input,
        // same candidate output bytes, caller sentinel unchanged on failure.
        let spent = 1_000_000_000u64.checked_sub(budgets[5]).unwrap();
        assert!(spent > 1,"matcher must consume billed work");
        let mut one_short=c64auto64::Work::new(spent-1)?;
        let mut marker=vec![0x47_u8];
        let err=c64auto64::png::trial_encode_indexed_staged(
            &e,&mut marker,&mut one_short).unwrap_err();
        assert_eq!(err.code(),"E_LIMIT","tight Work must fail with E_LIMIT");
        assert_eq!(marker,[0x47_u8],"tight Work published partial PNG");
        let mut precise=c64auto64::Work::new(spent)?;
        let mut exact=Vec::new();
        assert!(c64auto64::png::trial_encode_indexed_staged(
            &e,&mut exact,&mut precise)?);
        assert_eq!(precise.remaining(),0,"charged units must be deterministic");
        assert_eq!(exact,outputs[5],"metering changed PNG bytes");
        let once=|mode:usize|->Result<Vec<u8>,E>{
            let mut out=Vec::new();
            let ok=match mode {
                0=>raipixel::png::encode_indexed(black_box(&base),&mut out,&mut raipixel::Work::default())?,
                1=>c64flat::png::trial_encode_indexed_staged(black_box(&a),&mut out,&mut c64flat::Work::default())?,
                2=>c32sub::png::trial_encode_indexed_staged(black_box(&b),&mut out,&mut c32sub::Work::default())?,
                3=>c64lazy::png::trial_encode_indexed_staged(black_box(&c),&mut out,&mut c64lazy::Work::default())?,
                4=>c64lazysub::png::trial_encode_indexed_staged(black_box(&d),&mut out,&mut c64lazysub::Work::default())?,
                5=>c64auto64::png::trial_encode_indexed_staged(black_box(&e),&mut out,&mut c64auto64::Work::default())?,
                _=>c64autoall::png::trial_encode_indexed_staged(black_box(&f),&mut out,&mut c64autoall::Work::default())?
            };
            assert!(ok);
            Ok(out)
        };
        for mode in 0..outputs.len() {
            assert_eq!(raipixel::png::decode(&outputs[mode],raipixel::Limits::default(),&mut raipixel::Work::default())?.image,base,"RGBA mismatch mode {}",mode);
            let (rw,rh,rgba)=reference_rgba(&outputs[mode])?;
            assert_eq!([rw,rh],[width,height],"reference dimensions differ mode {}",mode);
            assert_eq!(rgba.as_slice(),base.pixels().as_flattened(),
                       "independent reference PNG decoder differs mode {}",mode);
            assert_eq!(once(mode)?,outputs[mode],"non-deterministic mode {}",mode);
            let mut bad=outputs[mode].clone();bad[29]^=1;
            assert!(matches!(raipixel::png::decode(&bad,raipixel::Limits::default(),&mut raipixel::Work::default()),Err(raipixel::png::Error::Crc)),"invalid CRC accepted in mode {}",mode);
        }
        // Stable best-of-N: byte length first, fixed candidate order second.
        // Encode all seven output candidates on demand; no display-only
        // postprocessing or unmeasured byte-count heuristic.
        let choose=|limit:usize|->Result<(usize,Vec<u8>),E>{
            let mut best_index=0usize;
            let mut best:Option<Vec<u8>>=None;
            for mode in 0..limit{
                let encoded=once(mode)?;
                if best.as_ref().is_none_or(|old|encoded.len()<old.len()){
                    best_index=mode;
                    best=Some(encoded);
                }
            }
            let selected=best.ok_or("no PNG candidates")?;
            Ok((best_index,selected))
        };
        let (winner,selected)=choose(outputs.len())?;
        let (winner6,selected6)=choose(6)?;
        // Differential experiment: a changed best is useful evidence,
        // not a correctness failure. Both must independently roundtrip.
        assert_eq!(reference_rgba(&selected6)?.2.as_slice(),base.pixels().as_flattened(),
                   "public reference rejects six-mode selected output");
        assert_eq!(selected6.len(),outputs[..6].iter().map(Vec::len).min().unwrap());
        assert_eq!(selected,outputs[winner],"best-of-seven changed candidate bytes");
        assert_eq!(selected.len(),outputs.iter().map(Vec::len).min().unwrap());
        assert_eq!(raipixel::png::decode(&selected,raipixel::Limits::default(),
            &mut raipixel::Work::default())?.image,base,"best-of-seven lost RGBA");
        assert_eq!(reference_rgba(&selected)?.2.as_slice(),base.pixels().as_flattened(),
                   "reference decoder rejected selected PNG");
        let batch=|mode:usize,n:usize|->Result<Duration,E>{
            let started=Instant::now();
            for _ in 0..n{black_box(once(mode)?);}
            Ok(started.elapsed())
        };
        let mut count=[1usize;7];
        for mode in 0..7 {
            while batch(mode,count[mode])?<Duration::from_millis(25){
                count[mode]=count[mode].checked_mul(2).ok_or("benchmark count overflow")?;
            }
        }
        let mut samples:[Vec<f64>;7]=std::array::from_fn(|_|Vec::new());
        for round in 0..7 {
            for i in 0..7{
                let mode=(round+i)%7;
                samples[mode].push(batch(mode,count[mode])?.as_secs_f64()*1e6/count[mode] as f64);
            }
        }
        // Both policies use independently calibrated batches and alternate
        // execution order by round to avoid systematic frequency/thermal drift.
        let mut iterations=[1usize;2];
        for (index,&limit) in [6usize,7].iter().enumerate(){
            while {
                let begin=Instant::now();
                for _ in 0..iterations[index]{black_box(choose(limit)?);}
                begin.elapsed()<Duration::from_millis(25)
            } {
                iterations[index]=iterations[index].checked_mul(2).ok_or("selection count overflow")?;
            }
        }
        let mut six_us=Vec::new();
        let mut seven_us=Vec::new();
        for round in 0..9 {
            for item in 0..2 {
                let idx=(item+round)%2;
                let limit=if idx==0{6}else{7};
                let started=Instant::now();
                for _ in 0..iterations[idx]{black_box(choose(limit)?);}
                let value=started.elapsed().as_secs_f64()*1e6/iterations[idx] as f64;
                if idx==0{six_us.push(value);}else{seven_us.push(value);}
            }
        }
        println!("{{\"supported\":true,\"width\":{},\"height\":{},\"bytes\":{:?},\"work_spent\":{:?},\"samples_us\":{:?},\"chosen\":{},\"winner6\":{},\"same_png\":{},\"best_6_bytes\":{},\"best_7_bytes\":{},\"best_of_6_samples_us\":{:?},\"best_of_7_samples_us\":{:?}}}",
            width,height,outputs.map(|o|o.len()),budgets.map(|v|1_000_000_000u64-v),samples,winner,winner6,
            selected6 == selected,selected6.len(),selected.len(),six_us,seven_us);
    }
    Ok(())
}
'''


def png_chunk(kind, data):
    return len(data).to_bytes(4,"big")+kind+data+binascii.crc32(kind+data).to_bytes(4,"big")

def independent_rgba_fixture(folder, colors, width, height):
    rgba=bytearray()
    for y in range(height):
        for x in range(width):
            index=(x*7+y*11)%colors
            rgba.extend((index&255,(index*41+7)&255,(index*73+17)&255,
                         (0,127,255)[index%3]))
    stride=width*4
    samples=b"".join(b"\x00"+rgba[y*stride:(y+1)*stride] for y in range(height))
    ihdr=width.to_bytes(4,"big")+height.to_bytes(4,"big")+bytes((8,6,0,0,0))
    content=(b"\x89PNG\r\n\x1a\n"+png_chunk(b"IHDR",ihdr)
             +png_chunk(b"IDAT",zlib.compress(samples,6))+png_chunk(b"IEND",b""))
    path=folder/f"indexed-case-{colors}-{width}x{height}.png"
    path.write_bytes(content)
    (folder/(path.name+".rgba")).write_bytes(rgba)
    return str(path)


def art_fixture(folder, name, colors, width, height, pattern, seed):
    """Self-contained exact-RGBA unfiltered PNG; no external image corpus."""
    def mix(x, y):
        z=((x+1)*0x9e3779b1 ^ (y+1)*0x85ebca6b ^ seed*0xc2b2ae35)&0xffffffff
        z^=z>>16
        z=(z*0x7feb352d)&0xffffffff
        z^=z>>15
        z=(z*0x846ca68b)&0xffffffff
        return (z^(z>>16))&0xffffffff
    palette=[(i&255,(i*37+seed*3)&255,(i*83+seed*7)&255,
              (0,64,128,255)[(i+seed+(i//256))%4]) for i in range(colors)]
    rgba=bytearray()
    for y in range(height):
        for x in range(width):
            h=mix(x,y)
            if pattern=="checker":
                index=((x//4)^(y//4))%colors
            elif pattern=="borders":
                index=(1 if x<3 or y<3 or x>=width-3 or y>=height-3
                       else 2 if (x//31+y//17)%11==0 else 0)%colors
            elif pattern=="grid":
                index=((x//16)%4+(y//16)%4+(x%16==0)+(y%16==0))%colors
            elif pattern=="diagonal":
                index=((x+y)//7)%colors
            elif pattern=="bands":
                index=((y//4)+(x//73))%colors
            elif pattern=="noise":
                index=h%colors
            elif pattern=="tiles":
                index=(((x%32)//4)^((y%32)//4)+(x//32+y//32)%3)%colors
            elif pattern=="alpha":
                index=(0 if h%11<7 else 1+(h%7))%colors
            elif pattern=="mixed":
                index=(0 if h%13<9 else (x//9+y//11+h%5))%colors
            elif pattern=="gradient":
                index=((x*colors)//width+(y//8))%colors
            elif pattern=="alternate":
                index=(0 if y%8<4 else (x//5+y//32)%colors)
            elif pattern=="hybrid":
                index=(h%colors if y<height//3 else
                       ((x//11)+(y//9))%colors if y<2*height//3
                       else ((x+y)//6)%colors)
            else:
                raise ValueError("unknown art pattern")
            rgba.extend(palette[index])
    stride=width*4
    raw=b"".join(b"\x00"+rgba[y*stride:(y+1)*stride] for y in range(height))
    header=width.to_bytes(4,"big")+height.to_bytes(4,"big")+bytes((8,6,0,0,0))
    encoded=(b"\x89PNG\r\n\x1a\n"+png_chunk(b"IHDR",header)
             +png_chunk(b"IDAT",zlib.compress(raw,6))+png_chunk(b"IEND",b""))
    path=folder/f"art-{name}-{colors}-{width}x{height}.png"
    path.write_bytes(encoded)
    (folder/(path.name+".rgba")).write_bytes(rgba)
    return str(path)

with tempfile.TemporaryDirectory(prefix="raipixel-quality-mix-") as temp:
    folder=Path(temp)
    deps=['raipixel={path='+json.dumps(str(ROOT))+'}','png="=0.18.1"']
    hashes={}
    configs=[("c64flat",64,"longest","none"),
             ("c32sub",32,"longest","sub"),
             ("c64lazy",64,"lazy","none"),
             ("c64lazysub",64,"lazy","sub"),
             ("c64auto64",64,"lazy","auto64"),
             ("c64autoall",64,"lazy","autoall")]
    for name,depth,parse_mode,filter_mode in configs:
        scratch=folder/name
        shutil.copytree(ROOT/"src",scratch/"src")
        rust_deflate,rust_png=candidate(depth,True,filter_mode,parse_mode)
        rust_deflate,rust_png=make_fallible(rust_deflate,rust_png)
        rust_deflate=make_reusable(rust_deflate)
        rust_deflate,rust_png=make_metered(rust_deflate,rust_png)
        rust_png=make_staged_png(rust_png)
        (scratch/"src/deflate/encode.rs").write_text(rust_deflate,encoding="utf-8")
        (scratch/"src/png.rs").write_text(rust_png,encoding="utf-8")
        (scratch/"Cargo.toml").write_text(
            f'[package]\nname="raipixel_{name}"\nversion="0.0.0"\nedition="2024"\n'
            '[lib]\npath="src/lib.rs"\n',encoding="utf-8")
        deps.append(f'{name}={{package="raipixel_{name}",path='+json.dumps(str(scratch))+'}')
        hashes[name]=hashlib.sha256((rust_deflate+rust_png).encode()).hexdigest()
    (folder/"src").mkdir()
    (folder/"Cargo.toml").write_text(
        '[package]\nname="raipixel-quality-mix"\nversion="0.0.0"\nedition="2024"\n'
        '[dependencies]\n'+'\n'.join(deps)+'\n',encoding="utf-8")
    (folder/"src/main.rs").write_text(DRIVER,encoding="utf-8")
    subprocess.run(["cargo","build","--release","--manifest-path",str(folder/"Cargo.toml")],check=True)
    exe=folder/"target/release"/("raipixel-quality-mix.exe" if sys.platform=="win32" else "raipixel-quality-mix")
    paths=list(sys.argv[1:])
    paths.extend(independent_rgba_fixture(folder,colors,width,height)
        for colors,width,height in [(1,7,11),(3,13,11),(5,17,11),
                                    (256,257,7),(257,257,11),(3,1024,300)])
    art_cases=[
      ("checker",2,128,128,"checker",1),
      ("border",3,257,127,"borders",2),
      ("grid",4,384,192,"grid",3),
      ("diagonal",16,257,129,"diagonal",4),
      ("band",16,512,128,"bands",5),
      ("noisy",16,255,127,"noise",6),
      ("repeating",16,768,96,"tiles",7),
      ("alpha",8,257,96,"alpha",8),
      ("mixed",8,512,192,"mixed",9),
      ("gradient",64,256,127,"gradient",10),
      ("noise256",256,257,127,"noise",11),
      ("alternating",4,512,192,"alternate",12),
      ("hybrid",16,384,192,"hybrid",13),
      ("maxwide",4,4096,4,"alternate",14),
      ("unsupported",257,129,129,"noise",15),
    ]
    paths.extend(art_fixture(folder,*case) for case in art_cases)
    lines=subprocess.check_output([str(exe),*paths],text=True).splitlines()
    if len(lines)!=len(paths):raise ValueError("lost benchmark output cases")
    cases=[]
    for path,line in zip(paths,lines):
        case=json.loads(line)
        case["input"]=path
        case["input_sha256"]=hashlib.sha256(Path(path).read_bytes()).hexdigest()
        if "art-unsupported-" in path and case["supported"]:
            raise ValueError("257 exactly distinct RGBA entries were silently accepted")
        if case["supported"]:
            frozen_sizes={"icon-64-compressed.png":[172,157,160,158,160,167,167],"sheet-1024-compressed.png":[4396,1668,2503,1653,2575,1673,1673],"comparison-sheet-1024.png":[9912,3629,4027,3570,3351,3586,3586],"indexed-case-256-257x7.png":[1451,1451,1175,1451,1168,1181,1178],"indexed-case-3-1024x300.png":[1006,800,729,800,730,760,760],"art-diagonal-16-257x129.png":[1247,736,843,732,718,517,517],"art-band-16-512x128.png":[1126,582,535,582,576,445,450]}
            name=Path(path).name
            if name in frozen_sizes and case["bytes"] != frozen_sizes[name]:
                raise ValueError(f"fallible retrofit changed byte counts for {name}: {case['bytes']} vs {frozen_sizes[name]}")
            case["best_bytes"]=min(case["bytes"])
            case["six_bytes"]=case["best_6_bytes"]
            case["seven_bytes"]=case["best_7_bytes"]
            case["regret_bytes"]=case["six_bytes"]-case["seven_bytes"]
            if case["regret_bytes"]<0:
                raise ValueError("shortlist beat own full candidate set")
            if case["same_png"] != (case["regret_bytes"]==0 and case["winner6"]==case["chosen"]):
                raise ValueError("selected PNG comparison inconsistent")
            if case["chosen"] != case["bytes"].index(case["best_bytes"]):
                raise ValueError("best-of-N chose the wrong file")
            case["best_of_7_median_us"]=sorted(case["best_of_7_samples_us"])[len(case["best_of_7_samples_us"])//2]
            case["best_of_6_median_us"]=sorted(case["best_of_6_samples_us"])[len(case["best_of_6_samples_us"])//2]
            case["six_over_seven_time"]=case["best_of_6_median_us"]/case["best_of_7_median_us"]
            case["best_mode"]=(["accepted-indexed","depth64-none","depth32-sub","depth64-lazy-none","depth64-lazy-sub","depth64-lazy-auto64","depth64-lazy-autoall"]
                               [case["bytes"].index(case["best_bytes"])])
            fixed_floor={"sheet-1024-compressed.png":1653,
                         "comparison-sheet-1024.png":3351,
                         "icon-64-compressed.png":157,
                         "indexed-case-256-257x7.png":1168,
                         "indexed-case-3-1024x300.png":729}
            # All frozen fallback paths (C11, C12 and prior Sub with depth32)
            # are included in this matrix and must not be lost.
            tracked=Path(path).name
            if tracked in fixed_floor:
                assert case["best_bytes"] <= fixed_floor[tracked], (
                    f"best-known fallback regressed for {tracked}: "
                    f"{case['best_bytes']} > {fixed_floor[tracked]}")
            medians=[sorted(a)[len(a)//2] for a in case["samples_us"]]
            case["median_us"]=medians
            case["size_over_indexed"]=[a/case["bytes"][0] for a in case["bytes"]]
            case["time_over_indexed"]=[a/medians[0] for a in medians]
        cases.append(case)
    print(json.dumps(dict(experiment="indexed_metered_lz_work_regression",
          candidates=["accepted-indexed"]+[name for name,_,_,_ in configs],
          hashes=hashes,platform=platform.platform(),
          compiler=subprocess.check_output(["rustc","--version","--verbose"],text=True),
          rounds=9,batch_min_ms=25,
          includes_actual_combined_candidate_rgba_crc_determinism_checks=True,
          independently_decoded_by_png_0_18_1_public_api=True,
          candidate_extra_scratch_not_production_accounted=True,
          production_changed=False,
          corpus_seeded_cases=len(art_cases),
          counterexamples=[dict(input=c["input"],sha256=c["input_sha256"],
                                six=c["six_bytes"],seven=c["seven_bytes"],
                                winner7=c["chosen"])
                           for c in cases if c["supported"] and c["regret_bytes"]>0],
          cases=cases)))
