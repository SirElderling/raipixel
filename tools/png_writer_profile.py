"""Time actual writer stages in a temporary instrumented copy of OUR library.
Paired uninstrumented calls expose instrumentation/layout overhead. No production
profiling API, source mutation or third-party dependencies are introduced.
Usage: python tools/png_writer_profile.py INPUT.png [INPUT.png ...]
"""
import hashlib
import json
import platform
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='raipixel-writer-profile-') as temporary:
    temporary = Path(temporary)
    shutil.copytree(ROOT / 'src', temporary / 'src')
    source = (temporary / 'src/png.rs').read_text()
    start = source.index('pub fn encode_compressed(')
    prefix, source = source[:start], source[start:]
    marker = '    let row_bytes = image.width as usize * 4;'
    assert source.count(marker) == 1
    source = source.replace(marker, '    let setup_start = std::time::Instant::now();\n' + marker)
    marker = '    let mut encoder = crate::deflate::Encoder::new(capacity)?;'
    assert source.count(marker) == 1
    source = source.replace(marker, marker + '\n    profile_add(0, setup_start);')
    source = source.replace('        raw.resize(count, 0);', '        let filter_start = std::time::Instant::now();\n        raw.resize(count, 0);')
    marker = '''        adler.update(&raw);
        let deflate = if COMPACT {
            encoder.block_compact(&raw, offset == raw_size)
        } else {
            encoder.block(&raw, offset == raw_size)
        };
        write_chunk(output, b"IDAT", deflate)?;'''
    assert source.count(marker) == 1
    source = source.replace(marker, '''        profile_add(1, filter_start);
        let adler_start = std::time::Instant::now();
        adler.update(&raw);
        profile_add(2, adler_start);
        let deflate_start = std::time::Instant::now();
        let encoded = if COMPACT {
            encoder.block_compact(&raw, offset == raw_size)
        } else {
            encoder.block(&raw, offset == raw_size)
        };
        profile_add(3, deflate_start);
        let output_start = std::time::Instant::now();
        write_chunk(output, b"IDAT", encoded)?;
        profile_add(4, output_start);''')
    source = prefix + source + '''
static PROFILE_NS: [std::sync::atomic::AtomicU64; 5] = [const { std::sync::atomic::AtomicU64::new(0) }; 5];
fn profile_add(index: usize, start: std::time::Instant) {
    PROFILE_NS[index].fetch_add(start.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
}
pub fn reset_profile() {
    for timer in &PROFILE_NS { timer.store(0, std::sync::atomic::Ordering::Relaxed); }
}
pub fn profile() -> [u64; 5] {
    std::array::from_fn(|i| PROFILE_NS[i].load(std::sync::atomic::Ordering::Relaxed))
}
'''
    (temporary / 'src/png.rs').write_text(source)
    manifest = '[package]\nname="raipixel-profile"\nversion="0.0.0"\nedition="2024"\n[dependencies]\nraipixel={path=' + json.dumps(str(ROOT)) + '}\n'
    (temporary / 'Cargo.toml').write_text(manifest)
    (temporary / 'examples').mkdir()
    (temporary / 'examples/profile.rs').write_text('''
#![forbid(unsafe_code)]
use std::{hint::black_box,time::{Instant,Duration}};
use raipixel as plain;
use raipixel_profile as profiled;
fn main()->Result<(),Box<dyn std::error::Error>>{for path in std::env::args().skip(1){
    let input=std::fs::read(path)?;
    let plain_image=plain::png::decode(&input,plain::Limits::default(),&mut plain::Work::default())?.image;
    let profiled_image=profiled::png::decode(&input,profiled::Limits::default(),&mut profiled::Work::default())?.image;
    assert_eq!(plain_image.pixels(),profiled_image.pixels());
    let encode=|mode|->Result<Vec<u8>,Box<dyn std::error::Error>>{
        let mut output=Vec::new();
        if mode==0{plain::png::encode_compressed(black_box(&plain_image),&mut output,&mut plain::Work::default())?;}
        else{profiled::png::encode_compressed(black_box(&profiled_image),&mut output,&mut profiled::Work::default())?;}
        Ok(output)
    };
    assert_eq!(encode(0)?,encode(1)?);
    let batch=|mode,count|->Result<f64,Box<dyn std::error::Error>>{
        let start=Instant::now();for _ in 0..count{black_box(encode(mode)?);}
        Ok(start.elapsed().as_secs_f64()*1e6/count as f64)
    };
    let mut counts=[1;2];for(mode,count)in counts.iter_mut().enumerate(){
        while batch(mode,*count)? * (*count as f64) < Duration::from_millis(25).as_secs_f64()*1e6 {*count*=2;}
    }
    let mut samples=[Vec::new(),Vec::new()];let mut stages=Vec::new();
    for round in 0..9{for n in 0..2{let mode=(round+n)%2;profiled::png::reset_profile();
        samples[mode].push(batch(mode,counts[mode])?);
        if mode==1{stages.push(profiled::png::profile().map(|ns|ns as f64/counts[1]as f64/1000.0));}
    }}
    println!("{{\\"width\\":{},\\"height\\":{},\\"iterations\\":{:?},\\"plain_profiled_samples_us\\":{:?},\\"stage_samples_us\\":{:?}}}",plain_image.size()[0],plain_image.size()[1],counts,samples,stages);
}Ok(())}
''')
    subprocess.run(['cargo', 'build', '--release', '--offline', '--manifest-path', str(temporary / 'Cargo.toml'), '--example', 'profile'], check=True)
    exe = temporary / 'target/release/examples' / ('profile.exe' if sys.platform == 'win32' else 'profile')
    rows = subprocess.check_output([str(exe), *sys.argv[1:]], text=True).splitlines()
    print(json.dumps(dict(experiment='actual_writer_stages',
        stages=['allocation_and_preflight', 'filter', 'adler', 'deflate', 'idat_crc_and_write'],
        compiler=subprocess.check_output(['rustc','--version','--verbose'],text=True),
        platform=platform.platform(),
        library_sources_sha256={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(ROOT.glob('src/**/*.rs'))},
        cases=[dict(input=path, **json.loads(row)) for path,row in zip(sys.argv[1:],rows)])))
