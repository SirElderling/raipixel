"""Screen original byte-preserving candidates; never read reference-crate code.

Trusted PNG inputs only. Extraction, scalar checks and byte equality are untimed.
--cpu/--features are local diagnostics, not portable defaults or parity claims.
"""
import argparse
import hashlib
import json
import platform
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--cpu')
parser.add_argument('--features')
parser.add_argument('inputs', nargs='+', type=Path)
args = parser.parse_args()
flags = ['--edition=2024', '-O']
if args.cpu:
    flags += ['-C', 'target-cpu=' + args.cpu]
if args.features:
    flags += ['-C', 'target-feature=' + args.features]
source = (ROOT / 'src/deflate/encode.rs').read_text()
narrow = source
for before, after in [
    ('positions: Vec<u32>', 'positions: Vec<u16>'),
    ('positions.resize(HASH_SIZE, u32::MAX)', 'positions.resize(HASH_SIZE, u16::MAX)'),
    ('self.positions.fill(u32::MAX)', 'self.positions.fill(u16::MAX)'),
    ('self.positions[slot] = pos as u32', 'self.positions[slot] = pos as u16'),
]:
    assert narrow.count(before) == 1
    narrow = narrow.replace(before, after)
run_helper = '''
#[inline(never)]
fn repeated_length(data: &[u8], prior: usize, pos: usize, maximum: usize) -> usize {
    let byte = data[prior];
    let view = &data[pos..pos + maximum];
    let mut matched = 0;
    for group in view.as_chunks::<16>().0 {
        if *group != [byte; 16] { break; }
        matched += 16;
    }
    let length = matched + view[matched..].iter().take_while(|&&x| x == byte).count();
    if length < 3 { 0 } else { length }
}
'''


def specialize_run(text):
    marker = 'match_length(data, pos - 1, pos, maximum)'
    assert text.count(marker) == 1
    return text.replace(marker, 'repeated_length(data, pos - 1, pos, maximum)') + run_helper


sources = dict(current=source, narrow=narrow, run=specialize_run(source),
               narrow_run=specialize_run(narrow))
constants = (ROOT / 'src/deflate.rs').read_text()
constants = constants[constants.index('const LENGTH_BASE:'):constants.index('/// Decode one zlib stream')]
with tempfile.TemporaryDirectory(prefix='raipixel-parity-screen-') as temporary:
    temporary = Path(temporary)
    inputs = []
    for i, path in enumerate(args.inputs):
        png = path.read_bytes()
        offset, idat = 8, bytearray()
        while offset < len(png):
            size = int.from_bytes(png[offset:offset + 4], 'big')
            if png[offset + 4:offset + 8] == b'IDAT':
                idat.extend(png[offset + 8:offset + 8 + size])
            offset += size + 12
        raw = zlib.decompress(idat)
        (temporary / f'{i}.raw').write_bytes(raw)
        inputs.append(dict(input=str(path), raw_bytes=len(raw),
                           raw_sha256=hashlib.sha256(raw).hexdigest()))
    driver = '#![forbid(unsafe_code)]\n#![allow(dead_code)]\n'
    driver += '#[path=' + json.dumps(str(ROOT / 'src/error.rs')) + ']mod error;pub use error::{Error,Resource};\n' + constants
    for name, text in sources.items():
        (temporary / (name + '.rs')).write_text(text)
        driver += 'mod ' + name + ';\n'
        driver += 'fn encode_' + name + '(data:&[u8])->Vec<u8>{let mut e=' + name + '::Encoder::new(data.len().min(65535)).unwrap();let mut out=Vec::new();let n=data.len().div_ceil(65535);for(i,b)in data.chunks(65535).enumerate(){out.extend_from_slice(e.block(b,i+1==n));}out}\n'
    driver += '''
use std::{hint::black_box,time::{Instant,Duration}};
fn main(){for path in std::env::args().skip(1){
    let data=std::fs::read(path).unwrap();
    let operations:[fn(&[u8])->Vec<u8>;4]=[encode_current,encode_narrow,encode_run,encode_narrow_run];
    let expected=operations[0](&data);for f in operations{assert_eq!(f(&data),expected);}
    let batch=|mode:usize,count|{let start=Instant::now();for _ in 0..count{black_box(operations[mode](black_box(&data)));}start.elapsed()};
    let mut counts=[1;4];for(mode,count)in counts.iter_mut().enumerate(){while batch(mode,*count)<Duration::from_millis(25){*count*=2;}}
    let mut samples:[Vec<f64>;4]=std::array::from_fn(|_|Vec::new());
    for round in 0..9{for i in 0..4{let mode=(round+i)%4;samples[mode].push(batch(mode,counts[mode]).as_secs_f64()*1e6/counts[mode]as f64);}}
    println!("{{\\"encoded_bytes\\":{},\\"iterations\\":{counts:?},\\"samples_us\\":{samples:?}}}",expected.len());
}}
'''
    (temporary / 'main.rs').write_text(driver)
    reports = {}
    for name, path in [('dictionary_run', temporary / 'main.rs'),
                       ('adler_lanes', ROOT / 'tools/png_adler_lanes_trial.rs')]:
        exe = temporary / (name + ('.exe' if sys.platform == 'win32' else ''))
        subprocess.run(['rustc', *flags, str(path), '-o', str(exe)], check=True)
        rows = subprocess.check_output([str(exe), *[str(temporary / f'{i}.raw') for i in range(len(inputs))]], text=True).splitlines()
        assert len(rows) == len(inputs)
        reports[name] = [dict(**item, **json.loads(row)) for item, row in zip(inputs, rows)]
    print(json.dumps(dict(experiment='parity_candidates',
        diagnostic_cpu_flags=bool(args.cpu or args.features), rustc_flags=flags,
        compiler=subprocess.check_output(['rustc', '--version', '--verbose'], text=True),
        platform=platform.platform(),
        operations=dict(dictionary_run=list(sources),
                        adler_lanes=['current', 'lanes8', 'lanes16', 'lanes32']),
        source_sha256={name: hashlib.sha256(text.encode()).hexdigest() for name, text in sources.items()},
        checksum_sha256=hashlib.sha256((ROOT / 'src/checksum.rs').read_bytes()).hexdigest(),
        driver_sha256=hashlib.sha256(driver.encode()).hexdigest(),
        adler_trial_sha256=hashlib.sha256((ROOT / 'tools/png_adler_lanes_trial.rs').read_bytes()).hexdigest(),
        **reports)))
