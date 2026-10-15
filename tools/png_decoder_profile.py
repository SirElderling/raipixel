"""Profile Huffman construction in OUR decoder using an instrumented temp copy.
Standard libraries only. Trusted noninterlaced RGBA8 inputs; no source changes.
Timer/atomic overhead is included, so setup cost is an estimate, not a speedup.
Usage: python tools/png_decoder_profile.py INPUT.png [INPUT.png ...]
"""
import argparse
import binascii
import zlib
import hashlib
import json
import platform
import subprocess
import sys
import tempfile
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--recompress', action='store_true', help='Recompress the same scanlines with stdlib zlib level 6')
parser.add_argument('inputs', nargs='+', type=Path)
args = parser.parse_args()
source = (ROOT / 'src/deflate.rs').read_text()
anchor = '        let mut counts = [0i32; 16];'
assert source.count(anchor) == 1
source = source.replace(anchor, '        let _measurement = SetupMeasurement(std::time::Instant::now());\n' + anchor)
source = source.replace('#[path = "deflate/encode.rs"]', '#[path = ' + json.dumps(str(ROOT / 'src/deflate/encode.rs')) + ']')
source += '''
static SETUP_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static SETUP_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
struct SetupMeasurement(std::time::Instant);
impl Drop for SetupMeasurement {
    fn drop(&mut self) {
        use std::sync::atomic::Ordering::Relaxed;
        SETUP_NS.fetch_add(self.0.elapsed().as_nanos() as u64, Relaxed);
        SETUP_COUNT.fetch_add(1, Relaxed);
    }
}
pub(crate) fn reset_profile() {
    use std::sync::atomic::Ordering::Relaxed;
    SETUP_NS.store(0, Relaxed); SETUP_COUNT.store(0, Relaxed);
}
pub(crate) fn profile() -> (u64,u64) {
    use std::sync::atomic::Ordering::Relaxed;
    (SETUP_NS.load(Relaxed), SETUP_COUNT.load(Relaxed))
}
'''
with tempfile.TemporaryDirectory(prefix='raipixel-decoder-profile-') as temporary:
    temporary = Path(temporary)
    (temporary / 'deflate.rs').write_text(source)
    inputs = []
    for index, path in enumerate(args.inputs):
        data = path.read_bytes()
        if args.recompress:
            offset, idat = 8, bytearray()
            while offset < len(data):
                count = int.from_bytes(data[offset:offset+4], 'big')
                if data[offset+4:offset+8] == b'IDAT':
                    idat.extend(data[offset+8:offset+8+count])
                offset += count + 12
            def chunk(kind, payload):
                return len(payload).to_bytes(4, 'big') + kind + payload + binascii.crc32(kind + payload).to_bytes(4, 'big')
            data = data[:8] + chunk(b'IHDR', data[16:29]) + chunk(b'IDAT', zlib.compress(zlib.decompress(idat), 6)) + chunk(b'IEND', b'')
        target = temporary / f'{index}.png'
        target.write_bytes(data)
        inputs.append(target)
    driver = '#![forbid(unsafe_code)]\n#![allow(dead_code, unused_imports)]\n'
    for name in ['error', 'limits', 'checksum']:
        driver += '#[path=' + json.dumps(str(ROOT / ('src/' + name + '.rs'))) + ']mod ' + name + ';\n'
    driver += '''
pub use error::{Error,Resource};
pub use limits::Work;
mod deflate;
use std::{hint::black_box,time::Instant};
fn main(){for path in std::env::args().skip(1) {
    let png=std::fs::read(path).unwrap();
    assert_eq!(&png[..8],b"\\x89PNG\\r\\n\\x1a\\n");
    assert_eq!(&png[24..29],&[8,6,0,0,0]);
    let width=u32::from_be_bytes(png[16..20].try_into().unwrap())as usize;
    let height=u32::from_be_bytes(png[20..24].try_into().unwrap())as usize;
    let expected=(width*4+1)*height;
    let mut data=Vec::new();let mut offset=8;
    while offset<png.len(){let n=u32::from_be_bytes(png[offset..offset+4].try_into().unwrap())as usize;
        if &png[offset+4..offset+8]==b"IDAT" {data.extend_from_slice(&png[offset+8..offset+8+n]);}
        offset+=n+12;
    }
    black_box(deflate::inflate(&data,expected,&mut Work::default()).unwrap());
    let mut times=Vec::new();let mut setups=Vec::new();let mut counts=Vec::new();
    for _ in 0..7 {
        deflate::reset_profile();let start=Instant::now();
        for _ in 0..128 {black_box(deflate::inflate(black_box(&data),expected,&mut Work::default()).unwrap());}
        times.push(start.elapsed().as_secs_f64()*1e6/128.0);
        let(ns,count)=deflate::profile();setups.push(ns as f64/128000.0);counts.push(count/128);
    }
    println!("{{\\"width\\":{},\\"height\\":{},\\"inflate_us\\":{:?},\\"huffman_setup_us\\":{:?},\\"tables_per_decode\\":{:?}}}",width,height,times,setups,counts);
}}
'''
    (temporary / 'main.rs').write_text(driver)
    exe = temporary / ('profile.exe' if sys.platform == 'win32' else 'profile')
    subprocess.run(['rustc', '--edition=2024', '-O', str(temporary / 'main.rs'), '-o', str(exe)], check=True)
    rows = subprocess.check_output([str(exe), *map(str, inputs)], text=True).splitlines()
    report = dict(experiment='huffman_setup', recompressed=args.recompress, zlib_version=zlib.ZLIB_RUNTIME_VERSION, compiler=subprocess.check_output(['rustc', '--version', '--verbose'], text=True),
                  platform=platform.platform(), instrumentation_overhead_included=True,
                  decoder_sha256=hashlib.sha256((ROOT / 'src/deflate.rs').read_bytes()).hexdigest(),
                  results=[dict(input=path, **json.loads(row)) for path,row in zip(map(str, args.inputs), rows)])
    print(json.dumps(report))
