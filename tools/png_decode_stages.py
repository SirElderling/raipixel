"""Time actual decoder stages in a temporary instrumented copy of OUR library.
Paired uninstrumented calls expose instrumentation/layout overhead. No production
profiling API, source mutation or third-party dependencies are introduced.
Usage: python tools/png_decode_stages.py INPUT.png [INPUT.png ...]
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
with tempfile.TemporaryDirectory(prefix='raipixel-decode-stages-') as temporary:
    temporary = Path(temporary)
    shutil.copytree(ROOT / 'src', temporary / 'src')
    source = (temporary / 'src/png.rs').read_text(encoding='utf-8')
    def replace_once(source, marker, replacement):
        assert source.count(marker) == 1, marker
        return source.replace(marker, replacement)
    source = replace_once(source, '    if input.len() > MAX_INPUT {',
        '    let chunk_start = std::time::Instant::now();\n    if input.len() > MAX_INPUT {')
    marker = '    let inflated = crate::deflate::inflate(&compressed, expected, work)?;'
    source = replace_once(source, marker,
        '    profile_add(0, chunk_start);\n    let inflate_start = std::time::Instant::now();\n' + marker + '\n    profile_add(1, inflate_start);\n    let reconstruct_start = std::time::Instant::now();')
    source = replace_once(source, '    Ok(Decoded {', '    profile_add(2, reconstruct_start);\n    Ok(Decoded {')
    marker = '    if crc32(&[&name, data]) != expected {'
    source = replace_once(source, marker, '    let crc_start = std::time::Instant::now();\n    let actual = crc32(&[&name, data]);\n    profile_add(3, crc_start);\n    if actual != expected {')
    source += """
static PROFILE_NS: [std::sync::atomic::AtomicU64; 5] = [const { std::sync::atomic::AtomicU64::new(0) }; 5];
pub(crate) fn profile_add(index: usize, start: std::time::Instant) {
    PROFILE_NS[index].fetch_add(start.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
}
pub fn reset_profile() {
    for timer in &PROFILE_NS { timer.store(0, std::sync::atomic::Ordering::Relaxed); }
}
pub fn profile() -> [u64; 5] {
    std::array::from_fn(|i| PROFILE_NS[i].load(std::sync::atomic::Ordering::Relaxed))
}
"""
    deflate = (temporary / 'src/deflate.rs').read_text(encoding='utf-8')
    deflate = replace_once(deflate, '    let mut adler = Adler::new();',
        '    let adler_start = std::time::Instant::now();\n    let mut adler = Adler::new();')
    deflate = replace_once(deflate, '    adler.update(&out);',
        '    adler.update(&out);\n    crate::png::profile_add(4, adler_start);')
    (temporary / 'src/deflate.rs').write_text(deflate, encoding='utf-8')
    (temporary / 'src/png.rs').write_text(source, encoding='utf-8')
    manifest = '[package]\nname="raipixel-profile"\nversion="0.0.0"\nedition="2024"\n[dependencies]\nraipixel={path=' + json.dumps(str(ROOT)) + '}\n'
    (temporary / 'Cargo.toml').write_text(manifest, encoding='utf-8')
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
    let batch=|mode,count|->Result<f64,Box<dyn std::error::Error>>{
        let start=Instant::now();for _ in 0..count{
            if mode==0{black_box(plain::png::decode(black_box(&input),plain::Limits::default(),&mut plain::Work::default())?);}
            else{black_box(profiled::png::decode(black_box(&input),profiled::Limits::default(),&mut profiled::Work::default())?);}
        }
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
''', encoding='utf-8')
    subprocess.run(['cargo', 'build', '--release', '--offline', '--manifest-path', str(temporary / 'Cargo.toml'), '--example', 'profile'], check=True)
    exe = temporary / 'target/release/examples' / ('profile.exe' if sys.platform == 'win32' else 'profile')
    rows = subprocess.check_output([str(exe), *sys.argv[1:]], text=True).splitlines()
    print(json.dumps(dict(experiment='actual_decoder_stages',
        stages=['chunks_including_crc_and_idat_join', 'inflate_including_adler', 'reconstruct_and_adopt', 'crc_nested_in_chunks', 'adler_nested_in_inflate'],
        nested_timers_are_not_additive=True,
        rustflags=__import__('os').environ.get('RUSTFLAGS', ''),
        inputs_sha256={path:hashlib.sha256(Path(path).read_bytes()).hexdigest() for path in sys.argv[1:]},
        compiler=subprocess.check_output(['rustc','--version','--verbose'],text=True),
        platform=platform.platform(),
        library_sources_sha256={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(ROOT.glob('src/**/*.rs'))},
        cases=[dict(input=path, **json.loads(row)) for path,row in zip(sys.argv[1:],rows)])))
