"""Scratch-only best-of-N PNG: dynamic Huffman, deeper rolling LZ and indexed filters.

Uses only raipixel code and RFC 1951/PNG filter formulas. Benchmarks actual
safe-Rust encodes and validates every candidate, including full RGBA,
hidden RGB, deterministic bytes, Work charge and CRC rejection before timing.
The additional match-chain and row-buffer memory is NOT yet charged as
production work. Never promote this diagnostic code unchanged.
Run: python tools/png_quality_mix_trial.py INPUT.png ...
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

def candidate(depth, dynamic=True, filter_mode="none"):
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
    png = replace_once(
        PNG,
        "let compressed = encoder.block_compact(&raw, offset + len == raw_size);",
        "let compressed = encoder.block_history(&raw, offset + len == raw_size);"
    )
    if filter_mode != "none":
        if filter_mode not in ("sub", "up", "paeth"):
            raise ValueError("unsupported filter")
        # These are indexed packed samples: PNG bytes-per-pixel is always 1.
        # Original packed bytes are saved row-by-row before prediction, even
        # when a DEFLATE block boundary splits a scanline.
        png = replace_once(
            png, "    let mut offset = 0;\n    let mut adler = Adler::new();",
            """    let mut offset = 0;
    let mut adler = Adler::new();
    let mut prev_row = vec![0u8; row_bytes];
    let mut cur_row = vec![0u8; row_bytes];
    let mut left_pixel = 0u8;""")
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
    return encoder, png

DRIVER = r'''
#![forbid(unsafe_code)]
use std::{hint::black_box, time::{Duration, Instant}};
type E = Box<dyn std::error::Error>;
fn main()->Result<(),E>{
    for path in std::env::args().skip(1) {
        let source=std::fs::read(&path)?;
        let base=raipixel::png::decode(&source,raipixel::Limits::default(),&mut raipixel::Work::default())?.image;
        let [width,height]=base.size();
        if path.contains("indexed-case-") {
            let expected=std::fs::read(format!("{path}.rgba"))?;
            assert_eq!(base.pixels().as_flattened(),expected.as_slice(),"independent input RGBA mismatch");
        }
        let a=c16::Image::from_pixels(width,height,base.pixels(),c16::Limits::default(),&mut c16::Work::default())?;
        let b=c32::Image::from_pixels(width,height,base.pixels(),c32::Limits::default(),&mut c32::Work::default())?;
        let c=c64::Image::from_pixels(width,height,base.pixels(),c64::Limits::default(),&mut c64::Work::default())?;
        let d=c32sub::Image::from_pixels(width,height,base.pixels(),c32sub::Limits::default(),&mut c32sub::Work::default())?;
        let e=c32up::Image::from_pixels(width,height,base.pixels(),c32up::Limits::default(),&mut c32up::Work::default())?;
        let f=c32paeth::Image::from_pixels(width,height,base.pixels(),c32paeth::Limits::default(),&mut c32paeth::Work::default())?;
        let mut outputs:[Vec<u8>;7]=std::array::from_fn(|_|Vec::new());
        let mut supports=[false;7];
        let mut budgets=[0u64;7];
        for mode in 0..7 {
            let (supported,remaining)=match mode {
                0=>{let mut w=raipixel::Work::default();let s=raipixel::png::encode_indexed(&base,&mut outputs[mode],&mut w)?;(s,w.remaining())},
                1=>{let mut w=c16::Work::default();let s=c16::png::encode_indexed(&a,&mut outputs[mode],&mut w)?;(s,w.remaining())},
                2=>{let mut w=c32::Work::default();let s=c32::png::encode_indexed(&b,&mut outputs[mode],&mut w)?;(s,w.remaining())},
                3=>{let mut w=c64::Work::default();let s=c64::png::encode_indexed(&c,&mut outputs[mode],&mut w)?;(s,w.remaining())},
                4=>{let mut w=c32sub::Work::default();let s=c32sub::png::encode_indexed(&d,&mut outputs[mode],&mut w)?;(s,w.remaining())},
                5=>{let mut w=c32up::Work::default();let s=c32up::png::encode_indexed(&e,&mut outputs[mode],&mut w)?;(s,w.remaining())},
                _=>{let mut w=c32paeth::Work::default();let s=c32paeth::png::encode_indexed(&f,&mut outputs[mode],&mut w)?;(s,w.remaining())}
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
                1=>c16::png::encode_indexed(black_box(&a),&mut out,&mut c16::Work::default())?,
                2=>c32::png::encode_indexed(black_box(&b),&mut out,&mut c32::Work::default())?,
                3=>c64::png::encode_indexed(black_box(&c),&mut out,&mut c64::Work::default())?,
                4=>c32sub::png::encode_indexed(black_box(&d),&mut out,&mut c32sub::Work::default())?,
                5=>c32up::png::encode_indexed(black_box(&e),&mut out,&mut c32up::Work::default())?,
                _=>c32paeth::png::encode_indexed(black_box(&f),&mut out,&mut c32paeth::Work::default())?
            };
            assert!(ok);
            Ok(out)
        };
        for mode in 0..outputs.len() {
            assert_eq!(raipixel::png::decode(&outputs[mode],raipixel::Limits::default(),&mut raipixel::Work::default())?.image,base,"RGBA mismatch mode {}",mode);
            assert_eq!(once(mode)?,outputs[mode],"non-deterministic mode {}",mode);
            let mut bad=outputs[mode].clone();bad[29]^=1;
            assert!(matches!(raipixel::png::decode(&bad,raipixel::Limits::default(),&mut raipixel::Work::default()),Err(raipixel::png::Error::Crc)),"invalid CRC accepted in mode {}",mode);
        }
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
        println!("{{\"supported\":true,\"width\":{},\"height\":{},\"bytes\":{:?},\"samples_us\":{:?}}}",
            width,height,outputs.map(|o|o.len()),samples);
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

with tempfile.TemporaryDirectory(prefix="raipixel-quality-mix-") as temp:
    folder=Path(temp)
    deps=['raipixel={path='+json.dumps(str(ROOT))+'}']
    hashes={}
    configs=[("c16",16,"none"),("c32",32,"none"),("c64",64,"none"),
             ("c32sub",32,"sub"),("c32up",32,"up"),("c32paeth",32,"paeth")]
    for name,depth,filter_mode in configs:
        scratch=folder/name
        shutil.copytree(ROOT/"src",scratch/"src")
        rust_deflate,rust_png=candidate(depth,True,filter_mode)
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
    subprocess.run(["cargo","build","--release","--offline","--manifest-path",str(folder/"Cargo.toml")],check=True)
    exe=folder/"target/release"/("raipixel-quality-mix.exe" if sys.platform=="win32" else "raipixel-quality-mix")
    paths=list(sys.argv[1:])
    paths.extend(independent_rgba_fixture(folder,colors,width,height)
        for colors,width,height in [(1,7,11),(3,13,11),(5,17,11),
                                    (256,257,7),(257,257,11),(3,1024,300)])
    lines=subprocess.check_output([str(exe),*paths],text=True).splitlines()
    if len(lines)!=len(paths):raise ValueError("lost benchmark output cases")
    cases=[]
    for path,line in zip(paths,lines):
        case=json.loads(line)
        case["input"]=path
        case["input_sha256"]=hashlib.sha256(Path(path).read_bytes()).hexdigest()
        if case["supported"]:
            medians=[sorted(a)[len(a)//2] for a in case["samples_us"]]
            case["median_us"]=medians
            case["size_over_indexed"]=[a/case["bytes"][0] for a in case["bytes"]]
            case["time_over_indexed"]=[a/medians[0] for a in medians]
        cases.append(case)
    print(json.dumps(dict(experiment="indexed_history_dynamic_16_32_64_filter_mix",
          candidates=["accepted-indexed"]+[name for name,_,_ in configs],
          hashes=hashes,platform=platform.platform(),
          compiler=subprocess.check_output(["rustc","--version","--verbose"],text=True),
          rounds=7,batch_min_ms=25,
          includes_actual_combined_candidate_rgba_crc_determinism_checks=True,
          candidate_extra_scratch_not_production_accounted=True,
          production_changed=False,cases=cases)))
