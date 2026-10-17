"""Compare our baseline/current DEFLATE and Adler stages in one executable.
Standard libraries only; PNG extraction/validation/capture is outside timing.
Usage: python tools/png_bottleneck_compare.py --baseline SHA INPUT.png [...]
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
parser.add_argument('--baseline', required=True)
parser.add_argument('inputs', nargs='+', type=Path)
args = parser.parse_args()
baseline = subprocess.check_output(['git','rev-parse','--verify',args.baseline+'^{commit}'],cwd=ROOT,text=True).strip()
sources = {}
for module, path in [('encode','src/deflate/encode.rs'),('checksum','src/checksum.rs')]:
    sources['before_'+module] = subprocess.check_output(['git','show',baseline+':'+path],cwd=ROOT,text=True)
    sources['after_'+module] = (ROOT/path).read_text()
constants = (ROOT/'src/deflate.rs').read_text()
constants = constants[constants.index('const LENGTH_BASE:'):constants.index('/// Decode one zlib stream')]
with tempfile.TemporaryDirectory(prefix='raipixel-bottleneck-') as temporary:
    temporary = Path(temporary)
    for name,source in sources.items():(temporary/(name+'.rs')).write_text(source)
    inputs = []
    for index,path in enumerate(args.inputs):
        data=path.read_bytes();offset=8;idat=bytearray()
        while offset<len(data):
            count=int.from_bytes(data[offset:offset+4],'big')
            if data[offset+4:offset+8]==b'IDAT':idat.extend(data[offset+8:offset+8+count])
            offset+=count+12
        raw=zlib.decompress(idat)
        (temporary/f'{index}.raw').write_bytes(raw)
        inputs.append(dict(input=str(path),raw_bytes=len(raw),raw_sha256=hashlib.sha256(raw).hexdigest()))
    driver='#![forbid(unsafe_code)]\n#![allow(dead_code)]\n'
    driver+='#[path='+json.dumps(str(ROOT/'src/error.rs'))+']mod error;pub use error::{Error,Resource};\n'+constants
    for name in sources:driver+='mod '+name+';\n'
    for name in ['before','after']:
        driver+='fn encode_'+name+'(data:&[u8])->Vec<u8>{let mut e='+name+'_encode::Encoder::new(data.len().min(65535)).unwrap();let mut out=Vec::new();let n=data.len().div_ceil(65535);for(i,b)in data.chunks(65535).enumerate(){out.extend_from_slice(e.block(b,i+1==n));}out}\n'
    driver+='''
use std::{hint::black_box,time::{Instant,Duration}};
fn main(){for path in std::env::args().skip(1){
    let data=std::fs::read(path).unwrap();assert_eq!(encode_before(&data),encode_after(&data));
    let mut reference_a=1u32;let mut reference_b=0u32;
    for &byte in &data{reference_a=(reference_a+u32::from(byte))%65521;reference_b=(reference_b+reference_a)%65521;}
    let expected=(reference_b<<16)|reference_a;
    let mut old=before_checksum::Adler::new();old.update(&data);assert_eq!(old.value(),expected);
    let mut new=after_checksum::Adler::new();new.update(&data);assert_eq!(new.value(),expected);
    let operations:[fn(&[u8]);4]=[
        |d|{black_box(encode_before(d));},|d|{black_box(encode_after(d));},
        |d|{let mut a=before_checksum::Adler::new();a.update(d);black_box(a.value());},
        |d|{let mut a=after_checksum::Adler::new();a.update(d);black_box(a.value());}
    ];
    let batch=|mode:usize,count|{let start=Instant::now();for _ in 0..count{operations[mode](black_box(&data));}start.elapsed()};
    let mut counts=[1;4];for(mode,count)in counts.iter_mut().enumerate(){while batch(mode,*count)<Duration::from_millis(50){*count*=2;}}
    let mut samples=[Vec::new(),Vec::new(),Vec::new(),Vec::new()];
    for round in 0..9{for n in 0..4{let mode=(round+n)%4;samples[mode].push(batch(mode,counts[mode]).as_secs_f64()*1e6/counts[mode]as f64);}}
    let medians:Vec<_>=samples.iter().map(|s|{let mut v=s.clone();v.sort_by(f64::total_cmp);v[4]}).collect();
    println!("{{\\"iterations\\":{:?},\\"median_us\\":{:?},\\"samples_us\\":{:?}}}",counts,medians,samples);
}}
'''
    (temporary/'main.rs').write_text(driver)
    exe=temporary/('stages.exe' if sys.platform=='win32' else 'stages')
    subprocess.run(['rustc','--edition=2024','-O',str(temporary/'main.rs'),'-o',str(exe)],check=True)
    rows=subprocess.check_output([str(exe),*[str(temporary/f'{i}.raw') for i in range(len(inputs))]],text=True).splitlines()
    print(json.dumps(dict(experiment='bottleneck_stages',baseline=baseline,
        operations=['deflate_baseline','deflate_current','adler_baseline','adler_current'],
        compiler=subprocess.check_output(['rustc','--version','--verbose'],text=True),platform=platform.platform(),
        source_sha256={name:hashlib.sha256(source.encode()).hexdigest() for name,source in sources.items()},
        cases=[dict(**item,**json.loads(row)) for item,row in zip(inputs,rows)])))
