"""Alternating current/XOR-match experiment on our encoder; std libraries only.
Uses a temporary copy of our source, never modifies the production implementation.
Usage: python tools/png_match_trial.py INPUT.png [INPUT.png ...]
"""
import hashlib
import json
import platform
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
source = (ROOT / 'src/deflate/encode.rs').read_text(encoding='utf-8')
a = source.index('fn match_length(')
annotation = '#[inline(always)]\n'
if source[:a].endswith(annotation):
    a -= len(annotation)
b = source.index('\n#[inline(never)]\nfn matching_tail', a)
candidate = source[:a] + '''fn match_length(data: &[u8], prior: usize, pos: usize, maximum: usize) -> usize {
    if prior >= pos || pos-prior > 32_768 || data[prior..prior+3] != data[pos..pos+3] { return 0; }
    let mut length = 3;
    while length + 8 <= maximum {
        let left = u64::from_le_bytes(data[prior+length..prior+length+8].try_into().unwrap());
        let right = u64::from_le_bytes(data[pos+length..pos+length+8].try_into().unwrap());
        let difference = left ^ right;
        if difference != 0 { return length + difference.trailing_zeros() as usize / 8; }
        length += 8;
    }
    while length < maximum && data[prior+length] == data[pos+length] { length += 1; }
    length
}
''' + source[b:]
constants = (ROOT / 'src/deflate.rs').read_text(encoding='utf-8')
constants = constants[constants.index('const LENGTH_BASE:'):constants.index('/// Decode one zlib stream')]
with tempfile.TemporaryDirectory(prefix='raipixel-match-trial-') as temporary:
    temporary = Path(temporary)
    (temporary / 'before.rs').write_text(source, encoding='utf-8')
    (temporary / 'after.rs').write_text(candidate, encoding='utf-8')
    inputs = []
    for i, path in enumerate(sys.argv[1:]):
        data = Path(path).read_bytes()
        offset, idat = 8, bytearray()
        while offset < len(data):
            count = int.from_bytes(data[offset:offset+4], 'big')
            if data[offset+4:offset+8] == b'IDAT':
                idat.extend(data[offset+8:offset+8+count])
            offset += count + 12
        raw = zlib.decompress(idat)
        destination = temporary / f'{i}.raw'
        destination.write_bytes(raw)
        inputs.append(dict(path=path, raw_bytes=len(raw), raw_sha256=hashlib.sha256(raw).hexdigest()))
    driver = '#![forbid(unsafe_code)]\n#![allow(dead_code)]\n#[path=' + json.dumps(str(ROOT / 'src/error.rs')) + ']mod error;\npub use error::{Error,Resource};\n' + constants
    driver += '''
mod before;mod after;
use std::{hint::black_box,time::{Instant,Duration}};
fn main(){
let operations:[fn(&[u8])->Vec<u8>;2]=[
'''
    for module in ['before', 'after']:
        driver += '|data|{let mut e=' + module + '::Encoder::new(data.len().min(65535)).unwrap();let mut out=Vec::new();let n=data.len().div_ceil(65535);for(i,b)in data.chunks(65535).enumerate(){out.extend_from_slice(e.block(b,i+1==n));}out},\n'
    driver += '''
];
for path in std::env::args().skip(1){let data=std::fs::read(path).unwrap();assert_eq!(operations[0](&data),operations[1](&data));
let batch=|mode:usize,count|{let start=Instant::now();for _ in 0..count {black_box(operations[mode](black_box(&data)));}start.elapsed()};
let mut counts=[1;2];for(mode,count)in counts.iter_mut().enumerate(){while batch(mode,*count)<Duration::from_millis(25){*count*=2;}}
let mut samples=[Vec::new(),Vec::new()];
for round in 0..9{for n in 0..2{let mode=(round+n)%2;samples[mode].push(batch(mode,counts[mode]).as_secs_f64()*1e6/counts[mode]as f64);}}
let medians:Vec<_>=samples.iter().map(|s|{let mut v=s.clone();v.sort_by(f64::total_cmp);v[4]}).collect();
println!("{{\\"iterations\\":{:?},\\"median_us\\":{:?},\\"samples_us\\":{:?}}}",counts,medians,samples);
}}
'''
    (temporary / 'main.rs').write_text(driver, encoding='utf-8')
    exe = temporary / ('trial.exe' if sys.platform == 'win32' else 'trial')
    subprocess.run(['rustc', '--edition=2024', '-O', str(temporary / 'main.rs'), '-o', str(exe)], check=True)
    rows = subprocess.check_output([str(exe), *[str(temporary / f'{i}.raw') for i in range(len(inputs))]], text=True).splitlines()
    print(json.dumps(dict(experiment='match_termination_xor', modes=['current', 'xor'],
        compiler=subprocess.check_output(['rustc', '--version', '--verbose'], text=True),
        platform=platform.platform(), encoder_sha256=hashlib.sha256(source.encode()).hexdigest(),
        candidate_sha256=hashlib.sha256(candidate.encode()).hexdigest(),
        cases=[dict(**item, **json.loads(row)) for item,row in zip(inputs,rows)])))
