"""Scratch-only C16 differential PNG selection on diverse, deterministic art.

Challenge C15's five-mode shortlist with transparent sprites, checkerboard
tiles, noisy indexed textures, gradients, scanline-wise transitions, odd
widths, maximal row width, and low/high palette cardinalities. Generate
independent valid RGBA PNGs in Python stdlib zlib with SHA-256 fixture hashes.
All seven encoders undergo raipixel and public png 0.18.1 decoding, CRC and
determinism checks. Report counterexamples (including any seven-mode wins);
never silently assume the five-mode shortlist is universally best.
Nine alternating paired size-selection timing rounds per fixture. Scratch-only,
no production code changes; extra temporary allocations not work-accounted.
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
        if path.contains("indexed-case-") {
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
                1=>{let mut w=c64flat::Work::default();let s=c64flat::png::encode_indexed(&a,&mut outputs[mode],&mut w)?;(s,w.remaining())},
                2=>{let mut w=c32sub::Work::default();let s=c32sub::png::encode_indexed(&b,&mut outputs[mode],&mut w)?;(s,w.remaining())},
                3=>{let mut w=c64lazy::Work::default();let s=c64lazy::png::encode_indexed(&c,&mut outputs[mode],&mut w)?;(s,w.remaining())},
                4=>{let mut w=c64lazysub::Work::default();let s=c64lazysub::png::encode_indexed(&d,&mut outputs[mode],&mut w)?;(s,w.remaining())},
                5=>{let mut w=c64auto64::Work::default();let s=c64auto64::png::encode_indexed(&e,&mut outputs[mode],&mut w)?;(s,w.remaining())},
                _=>{let mut w=c64autoall::Work::default();let s=c64autoall::png::encode_indexed(&f,&mut outputs[mode],&mut w)?;(s,w.remaining())}
            };
            supports[mode]=supported;budgets[mode]=remaining;
        }
        assert!(supports.iter().all(|s|*s==supports[0]),"palette support mismatch");
        assert!(budgets.iter().all(|w|*w==budgets[0]),"work budget mismatch; scratch candidate cannot be promoted");
        if !supports[0] {
            assert!(outputs.iter().all(Vec::is_empty),"unsupported wrote bytes");
            println!("{{\"supported\":false,\"width\":{},\"height\":{}}}",width,height);
            continue;
        }
        let once=|mode:usize|->Result<Vec<u8>,E>{
            let mut out=Vec::new();
            let ok=match mode {
                0=>raipixel::png::encode_indexed(black_box(&base),&mut out,&mut raipixel::Work::default())?,
                1=>c64flat::png::encode_indexed(black_box(&a),&mut out,&mut c64flat::Work::default())?,
                2=>c32sub::png::encode_indexed(black_box(&b),&mut out,&mut c32sub::Work::default())?,
                3=>c64lazy::png::encode_indexed(black_box(&c),&mut out,&mut c64lazy::Work::default())?,
                4=>c64lazysub::png::encode_indexed(black_box(&d),&mut out,&mut c64lazysub::Work::default())?,
                5=>c64auto64::png::encode_indexed(black_box(&e),&mut out,&mut c64auto64::Work::default())?,
                _=>c64autoall::png::encode_indexed(black_box(&f),&mut out,&mut c64autoall::Work::default())?
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
        let (winner5,selected5)=choose(5)?;
        // Differential experiment: a changed best is useful evidence,
        // not a correctness failure. Both must independently roundtrip.
        assert_eq!(reference_rgba(&selected5)?.2.as_slice(),base.pixels().as_flattened(),
                   "public reference rejects five-mode selected output");
        assert_eq!(selected5.len(),outputs[..5].iter().map(Vec::len).min().unwrap());
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
        for (index,&limit) in [5usize,7].iter().enumerate(){
            while {
                let begin=Instant::now();
                for _ in 0..iterations[index]{black_box(choose(limit)?);}
                begin.elapsed()<Duration::from_millis(25)
            } {
                iterations[index]=iterations[index].checked_mul(2).ok_or("selection count overflow")?;
            }
        }
        let mut five_us=Vec::new();
        let mut seven_us=Vec::new();
        for round in 0..9 {
            for item in 0..2 {
                let idx=(item+round)%2;
                let limit=if idx==0{5}else{7};
                let started=Instant::now();
                for _ in 0..iterations[idx]{black_box(choose(limit)?);}
                let value=started.elapsed().as_secs_f64()*1e6/iterations[idx] as f64;
                if idx==0{five_us.push(value);}else{seven_us.push(value);}
            }
        }
        println!("{{\"supported\":true,\"width\":{},\"height\":{},\"bytes\":{:?},\"samples_us\":{:?},\"chosen\":{},\"winner5\":{},\"same_png\":{},\"best_5_bytes\":{},\"best_7_bytes\":{},\"best_of_5_samples_us\":{:?},\"best_of_7_samples_us\":{:?}}}",
            width,height,outputs.map(|o|o.len()),samples,winner,winner5,
            selected5 == selected,selected5.len(),selected.len(),five_us,seven_us);
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
              (0,64,128,255)[(i+seed)%4]) for i in range(colors)]
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
        if case["supported"]:
            case["best_bytes"]=min(case["bytes"])
            case["five_bytes"]=case["best_5_bytes"]
            case["seven_bytes"]=case["best_7_bytes"]
            case["regret_bytes"]=case["five_bytes"]-case["seven_bytes"]
            if case["regret_bytes"]<0:
                raise ValueError("shortlist beat own full candidate set")
            if case["same_png"] != (case["regret_bytes"]==0 and case["winner5"]==case["chosen"]):
                raise ValueError("selected PNG comparison inconsistent")
            if case["chosen"] != case["bytes"].index(case["best_bytes"]):
                raise ValueError("best-of-N chose the wrong file")
            case["best_of_7_median_us"]=sorted(case["best_of_7_samples_us"])[len(case["best_of_7_samples_us"])//2]
            case["best_of_5_median_us"]=sorted(case["best_of_5_samples_us"])[len(case["best_of_5_samples_us"])//2]
            case["five_over_seven_time"]=case["best_of_5_median_us"]/case["best_of_7_median_us"]
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
    print(json.dumps(dict(experiment="indexed_diverse_pixel_art_bestof5_vs7",
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
                                five=c["five_bytes"],seven=c["seven_bytes"],
                                winner7=c["chosen"])
                           for c in cases if c["supported"] and c["regret_bytes"]>0],
          cases=cases)))
