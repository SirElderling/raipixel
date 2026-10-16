"""Isolated native Rust dynamic-Huffman experiment using raipixel's LZ tokens.

A disposable scratch copy of OUR safe Rust fixed-Huffman encoder records its
literal and match tokens, then attempts RFC 1951 dynamic Huffman re-encoding
with code-length RLE and a size fallback. The original encode_indexed and
all production code stay unchanged. The scratch result must preserve exact
RGBA (including hidden transparent RGB), work budget, deterministic bytes and
CRC validation. No new dependencies, unsafe code, or third-party crate
implementation inspected.

Usage: python tools/png_dynamic_indexed_guard_trial.py INPUT.png [INPUT.png ...]
"""
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
SOURCE=(ROOT/"src/deflate/encode.rs").read_text(encoding="utf-8")

def one(src,old,new):
    if src.count(old)!=1:
        raise ValueError("unstable encoder instrumentation anchor "+repr(old[:120])+"; found "+str(src.count(old)))
    return src.replace(old,new)

HELPER=r'''
#[derive(Clone,Copy)]
enum TrialToken { Literal(u8), Reference(usize,usize) }

// Bounded token storage: a literal needs one u16, a match two u16.
// Each match consumes at least three input bytes, so at most 65,535
// words (128 KiB) per PNG block.
fn trial_unpack(tokens:&[u16])->impl Iterator<Item=TrialToken>+'_{
    struct Tokens<'a>{input:&'a[u16],index:usize}
    impl Iterator for Tokens<'_>{
        type Item=TrialToken;
        fn next(&mut self)->Option<TrialToken>{
            let &word=self.input.get(self.index)?;
            self.index+=1;
            if word<256{
                Some(TrialToken::Literal(word as u8))
            }else{
                let &distance=self.input.get(self.index)?;
                self.index+=1;
                Some(TrialToken::Reference((word-256) as usize+3,distance as usize))
            }
        }
    }
    Tokens{input:tokens,index:0}
}

#[derive(Clone)]
struct TrialNode { freq:u32, left:Option<usize>, right:Option<usize>, sym:Option<usize> }

// Scratch-only bounded-size Huffman trees. If an optimal tree exceeds DEFLATE
// length caps, reject the candidate and use accepted fixed/stored output.
fn trial_lengths(freq:&[u32],max_depth:u8)->Option<Vec<u8>> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let mut nodes=Vec::<TrialNode>::new();
    let mut queue=BinaryHeap::<Reverse<(u32,usize)>>::new();
    for (symbol,&weight) in freq.iter().enumerate(){
        if weight!=0{
            let index=nodes.len();
            nodes.push(TrialNode{freq:weight,left:None,right:None,sym:Some(symbol)});
            queue.push(Reverse((weight,index)));
        }
    }
    if queue.is_empty(){return None;}
    let mut lengths=vec![0u8;freq.len()];
    if queue.len()==1 {
        let Reverse((_,index))=queue.pop()?;
        lengths[nodes[index].sym?]=1;
        return Some(lengths);
    }
    while queue.len()>1 {
        let Reverse((left_weight,left))=queue.pop()?;
        let Reverse((right_weight,right))=queue.pop()?;
        let weight=left_weight.checked_add(right_weight)?;
        let index=nodes.len();
        nodes.push(TrialNode{freq:weight,left:Some(left),right:Some(right),sym:None});
        queue.push(Reverse((weight,index)));
    }
    let Reverse((_,root))=queue.pop()?;
    let mut stack=vec![(root,0u8)];
    while let Some((node,depth))=stack.pop(){
        if depth>max_depth{return None;}
        if let Some(symbol)=nodes[node].sym{
            lengths[symbol]=depth;
        }else{
            stack.push((nodes[node].left?,depth+1));
            stack.push((nodes[node].right?,depth+1));
        }
    }
    Some(lengths)
}
fn trial_codes(lengths:&[u8])->Vec<(u16,u8)> {
    let mut count=[0u16;16];
    for &length in lengths {
        if length!=0 {count[usize::from(length)]+=1;}
    }
    let mut next=[0u16;16];
    for n in 1..16{next[n]=(next[n-1]+count[n-1])<<1;}
    let mut result=vec![(0u16,0u8);lengths.len()];
    for (symbol,&length) in lengths.iter().enumerate(){
        if length!=0 {
            let code=next[usize::from(length)];
            next[usize::from(length)]+=1;
            result[symbol]=(code.reverse_bits()>>(16-length),length);
        }
    }
    result
}
fn trial_rle(lengths:&[u8])->Vec<(usize,u32,u32)> {
    let mut output=Vec::new();
    let mut i=0;
    while i<lengths.len(){
        let value=lengths[i];
        let mut end=i+1;
        while end<lengths.len() && lengths[end]==value {end+=1;}
        let mut n=end-i;
        if value==0 {
            while n>=11 {
                let take=n.min(138);
                output.push((18,(take-11) as u32,7));
                n-=take;
            }
            while n>=3 {
                let take=n.min(10);
                output.push((17,(take-3) as u32,3));
                n-=take;
            }
            for _ in 0..n {output.push((0,0,0));}
        }else{
            output.push((usize::from(value),0,0));
            n-=1;
            while n>=3 {
                let take=n.min(6);
                output.push((16,(take-3) as u32,2));
                n-=take;
            }
            for _ in 0..n {output.push((usize::from(value),0,0));}
        }
        i=end;
    }
    output
}
// Fixed-code bit cost: three-bit header, seven-bit end marker and tokens.
fn trial_fixed_bits(tokens: &[u16]) -> u32 {
    let mut bits = 3 + 7;
    for token in trial_unpack(tokens) {
        match token {
            TrialToken::Literal(value) => {
                bits += u32::from(FIXED_CODES[usize::from(value)] >> 9);
            }
            TrialToken::Reference(length, distance) => {
                let index = if length == 258 { 28 } else { usize::from(LENGTH_CODES[length-3]) };
                bits += u32::from(FIXED_CODES[257+index] >> 9) + LENGTH_EXTRA[index];
                let dindex = distance_code(distance);
                bits += 5 + DISTANCE_EXTRA[dindex];
            }
        }
    }
    bits
}

impl Encoder {
    fn trial_put_code(&mut self,codes:&[(u16,u8)],symbol:usize){
        let (code,n)=codes[symbol];
        self.put(u32::from(code),u32::from(n));
    }
    fn trial_emit_dynamic(&mut self,tokens:&[u16],last:bool)->bool{
        const ORDER:[usize;19]=[16,17,18,0,8,7,9,6,10,5,11,4,12,3,13,2,14,1,15];
        let mut lf=[0u32;286];
        let mut df=[0u32;30];
        for token in trial_unpack(tokens) {
            match token {
                TrialToken::Literal(v)=>lf[usize::from(v)]+=1,
                TrialToken::Reference(len,dist)=>{
                    let l=if len==258{285}else{257+usize::from(LENGTH_CODES[len-3])};
                    lf[l]+=1;
                    df[distance_code(dist)]+=1;
                }
            }
        }
        lf[256]+=1;
        if df.iter().all(|&x|x==0){df[0]=1;}
        let Some(lens)=trial_lengths(&lf,15) else{return false;};
        let Some(dist_lens)=trial_lengths(&df,15) else{return false;};
        let ll_count=lens.iter().rposition(|&v|v!=0).unwrap_or(256).max(256)+1;
        let ds_count=dist_lens.iter().rposition(|&v|v!=0).unwrap_or(0)+1;
        let mut together=lens[..ll_count].to_vec();
        together.extend_from_slice(&dist_lens[..ds_count]);
        let runs=trial_rle(&together);
        let mut cf=[0u32;19];
        for &(sym,_,_) in &runs{cf[sym]+=1;}
        if cf.iter().filter(|&&v|v!=0).count()==1{
            if let Some(slot)=cf.iter().position(|&v|v==0){cf[slot]=1;}
        }
        let Some(code_lens)=trial_lengths(&cf,7) else{return false;};
        let cl_count=ORDER.iter().rposition(|&s|code_lens[s]!=0).unwrap_or(3).max(3)+1;
        let lc=trial_codes(&lens);
        let dc=trial_codes(&dist_lens);
        let cc=trial_codes(&code_lens);
        self.put(u32::from(last)|4,3); // BTYPE=10, dynamic.
        self.put((ll_count-257) as u32,5);
        self.put((ds_count-1) as u32,5);
        self.put((cl_count-4) as u32,4);
        for &sym in &ORDER[..cl_count]{self.put(u32::from(code_lens[sym]),3);}
        for &(sym,extra,count) in &runs{
            self.trial_put_code(&cc,sym);
            self.put(extra,count);
        }
        for token in trial_unpack(tokens) {
            match token {
                TrialToken::Literal(v)=>self.trial_put_code(&lc,usize::from(v)),
                TrialToken::Reference(len,dist)=>{
                    let index=if len==258{28}else{usize::from(LENGTH_CODES[len-3])};
                    self.trial_put_code(&lc,257+index);
                    self.put((len-LENGTH_BASE[index]) as u32,LENGTH_EXTRA[index]);
                    let dindex=distance_code(dist);
                    self.trial_put_code(&dc,dindex);
                    self.put((dist-DISTANCE_BASE[dindex]) as u32,DISTANCE_EXTRA[dindex]);
                }
            }
        }
        self.trial_put_code(&lc,256);
        if last{self.align();}
        self.flush_bytes();
        true
    }
}
'''
def modified(original):
    s=original
    s=one(s,
          "        let saved = (self.bits, self.count);\n        self.output.clear();",
          "        let saved = (self.bits, self.count);\n        let mut trial_tokens = Vec::<u16>::new();\n        self.output.clear();")
    s=one(s,
          "        self.put(u32::from(last) | 2, 3); // BTYPE=01, least-significant bit first.",
          "        if !COMPACT { self.put(u32::from(last) | 2, 3); } // fast encoder unchanged")
    s=one(s,
          "                self.reference(length, pos - prior);",
          "                if COMPACT {\n                    trial_tokens.push(256 + (length-3) as u16); trial_tokens.push((pos-prior) as u16);\n                } else {\n                    self.reference(length, pos-prior);\n                }")
    literal="                self.symbol(u32::from(data[pos]));"
    if s.count(literal)!=2:raise ValueError("expected 2 literal positions, got "+str(s.count(literal)))
    s=s.replace(literal,
        "                if COMPACT {\n                    trial_tokens.push(data[pos] as u16);\n                } else {\n"
        +literal+"\n                }")
    s=one(s,
          "        for &byte in &data[pos..] {\n            self.symbol(u32::from(byte));",
          "        for &byte in &data[pos..] {\n            if COMPACT { trial_tokens.push(byte as u16); }\n            else { self.symbol(u32::from(byte)); }")
    s=one(s,
          """        self.symbol(256);
        if last {
            self.align();
        }
        self.flush_bytes();
        // A stored block is preferable when fixed codes expand this input.
        // Restore the pending bits from the preceding block before retrying.
        if self.output.len() > data.len() + 5 {
            (self.bits, self.count) = saved;
            self.store(data, last);
        }
        &self.output""",
          """        if !COMPACT {
            self.symbol(256);
            if last { self.align(); }
            self.flush_bytes();
            if self.output.len() > data.len() + 5 {
                (self.bits, self.count) = saved;
                self.store(data, last);
            }
            return &self.output;
        }
        // A single token parse and buffer; no redundant fixed block
        // emission when dynamic Huffman wins.
        let fixed_bits = trial_fixed_bits(&trial_tokens);
        let fixed_bytes = ((saved.1 + fixed_bits + if last { 7 } else { 0 }) / 8) as usize;
        let dynamic_ok = self.trial_emit_dynamic(&trial_tokens, last);
        if dynamic_ok && self.output.len() < fixed_bytes.min(data.len() + 5) {
            return &self.output;
        }
        self.output.clear();
        (self.bits, self.count) = saved;
        if fixed_bytes > data.len() + 5 {
            self.store(data, last);
            return &self.output;
        }
        self.put(u32::from(last) | 2, 3);
        for token in trial_unpack(&trial_tokens) {
            match token {
                TrialToken::Literal(value) => self.symbol(u32::from(value)),
                TrialToken::Reference(length, distance) => self.reference(length, distance),
            }
        }
        self.symbol(256);
        if last { self.align(); }
        self.flush_bytes();
        &self.output""")
    # Avoid dynamic coding for small indexed chunks.
    # Retain the accepted compact bitstream on those chunks.
    s = one(s,
            "        let saved = (self.bits, self.count);\n        let mut trial_tokens = Vec::<u16>::new();",
            "        let trial_use = COMPACT && data.len() >= 4096;\n        let saved = (self.bits, self.count);\n        let mut trial_tokens = Vec::<u16>::new();")
    s = s.replace("if !COMPACT {", "if !trial_use {")
    s = s.replace("if COMPACT {", "if trial_use {")
    return s+"\n"+HELPER

with tempfile.TemporaryDirectory(prefix="raipixel-indexed-guard-") as temp:
    folder=Path(temp)
    scratch=folder/"candidate"
    shutil.copytree(ROOT/"src",scratch/"src")
    candidate=modified(SOURCE)
    (scratch/"src/deflate/encode.rs").write_text(candidate,encoding="utf-8")
    (scratch/"Cargo.toml").write_text(
        '[package]\nname="raipixel_dynamic_candidate"\nversion="0.0.0"\nedition="2024"\n'
        '[lib]\npath="src/lib.rs"\n',encoding="utf-8")
    (folder/"src").mkdir()
    (folder/"Cargo.toml").write_text(
        '[package]\nname="raipixel-indexed-guard-screen"\nversion="0.0.0"\nedition="2024"\n'
        '[dependencies]\nraipixel={path='+json.dumps(str(ROOT))+'}\n'
        'candidate={package="raipixel_dynamic_candidate",path='+json.dumps(str(scratch))+'}\n',
        encoding="utf-8")
    driver=r'''
#![forbid(unsafe_code)]
use std::{hint::black_box,time::{Duration,Instant}};
fn main()->Result<(),Box<dyn std::error::Error>>{
    for path in std::env::args().skip(1){
        let bytes=std::fs::read(&path)?;
        let base_image=raipixel::png::decode(&bytes,raipixel::Limits::default(),&mut raipixel::Work::default())?.image;
        let [w,h]=base_image.size();
        let test_image=candidate::Image::from_pixels(w,h,base_image.pixels(),candidate::Limits::default(),&mut candidate::Work::default())?;
        let mut accepted=Vec::new();
        let mut bwork=raipixel::Work::default();
        let supported=raipixel::png::encode_indexed(&base_image,&mut accepted,&mut bwork)?;
        let mut proposed=Vec::new();
        let mut twork=candidate::Work::default();
        let same_support=candidate::png::encode_indexed(&test_image,&mut proposed,&mut twork)?;
        assert_eq!(bwork.remaining(),twork.remaining(),"scratch changed Work charge");
        assert_eq!(supported,same_support,"palette detection disagreement");
        if !supported {
            assert!(accepted.is_empty() && proposed.is_empty(),"unsupported image wrote bytes");
            println!("{{\"supported\":false,\"width\":{},\"height\":{}}}",w,h);
            continue;
        }
        assert_eq!(raipixel::png::decode(&proposed,raipixel::Limits::default(),&mut raipixel::Work::default())?.image,base_image);
        let mut repeated=Vec::new();
        candidate::png::encode_indexed(&test_image,&mut repeated,&mut candidate::Work::default())?;
        assert_eq!(proposed,repeated,"non-deterministic output");
        assert!(proposed.len()<=accepted.len(),"unexpected expansion");
        let mut invalid=proposed.clone();
        invalid[29]^=1;
        assert!(matches!(raipixel::png::decode(&invalid,raipixel::Limits::default(),
            &mut raipixel::Work::default()),Err(raipixel::png::Error::Crc)));
        let batch=|mode:usize,count:usize|->Result<Duration,Box<dyn std::error::Error>>{
            let start=Instant::now();
            for _ in 0..count{
                let mut out=Vec::new();
                if mode==0 {
                    raipixel::png::encode_indexed(black_box(&base_image),&mut out,&mut raipixel::Work::default())?;
                }else{
                    candidate::png::encode_indexed(black_box(&test_image),&mut out,&mut candidate::Work::default())?;
                }
                black_box(out);
            }
            Ok(start.elapsed())
        };
        let mut counts=[1usize;2];
        for (mode,count) in counts.iter_mut().enumerate(){
            while batch(mode,*count)?<Duration::from_millis(25){
                *count=count.checked_mul(2).ok_or("iteration overflow")?;
            }
        }
        let mut samples=[Vec::new(),Vec::new()];
        for round in 0..9{
            for mode in [round%2,(round+1)%2]{
                samples[mode].push(batch(mode,counts[mode])?.as_secs_f64()*1e6/counts[mode] as f64);
            }
        }
        println!("{{\"supported\":true,\"accepted_bytes\":{},\"dynamic_bytes\":{},\"width\":{},\"height\":{},\"samples_us\":{:?}}}",
            accepted.len(),proposed.len(),w,h,samples);
    }
    Ok(())
}
'''
    (folder/"src/main.rs").write_text(driver,encoding="utf-8")
    subprocess.run(["cargo","build","--release","--offline","--manifest-path",str(folder/"Cargo.toml")],check=True)
    exe=folder/"target/release"/("raipixel-indexed-guard-screen.exe" if sys.platform=="win32" else "raipixel-indexed-guard-screen")
    lines=subprocess.check_output([str(exe),*sys.argv[1:]],text=True).splitlines()
    assert len(lines)==len(sys.argv[1:])
    cases=[]
    for path,line in zip(sys.argv[1:],lines):
        case=json.loads(line)
        if case["supported"]:
            medians=[sorted(sample)[len(sample)//2] for sample in case["samples_us"]]
            case.update(median_us=medians,
                        size_over_indexed=case["dynamic_bytes"]/case["accepted_bytes"],
                        time_over_indexed=medians[1]/medians[0])
        case.update(input=path,
                    input_sha256=hashlib.sha256(Path(path).read_bytes()).hexdigest())
        cases.append(case)
    print(json.dumps(dict(experiment="native_safe_dynamic_huffman_indexed_guard_4096",
                          compiler=subprocess.check_output(["rustc","--version","--verbose"],text=True),
                          platform=platform.platform(),source_sha256=hashlib.sha256(SOURCE.encode()).hexdigest(),
                          candidate_sha256=hashlib.sha256(candidate.encode()).hexdigest(),
                          driver_sha256=hashlib.sha256(driver.encode()).hexdigest(),
                          cases=cases,all_rgba_crc_budget_determinism_checks_passed=True,
                          rounds=9,minimum_batch_ms=25,production_changed=False)))
