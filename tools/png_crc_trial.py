"""Screen safe slicing widths derived from our own polynomial CRC code.
No third-party source/dependencies; temporary files only. The slicing technique
is credited to Joglekar, Kounavis and Berry (FAST 2005) in algorithm-notes.md.
Usage: python tools/png_crc_trial.py > crc-trial.json
"""
import hashlib
import json
import os
import platform
import subprocess
import sys
import tempfile
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
BASELINE = '09c7359f4bb8f5d870963aabbcd7cefd4666d7e6'
source = subprocess.check_output(['git', 'show', BASELINE + ':src/checksum.rs'], cwd=ROOT).decode('utf-8')
with tempfile.TemporaryDirectory(prefix='raipixel-crc-trial-') as temporary:
    folder = Path(temporary)
    modules = []
    for width in [8, 16, 32]:
        candidate = source
        if width != 8:
            candidate = candidate.replace('[[u32; 256]; 8]', f'[[u32; 256]; {width}]').replace('[[0; 256]; 8]', f'[[0; 256]; {width}]').replace('while level < 8', f'while level < {width}')
            start = candidate.index('        let (bulk, tail) = part.as_chunks::<8>();')
            end = candidate.index('        for &byte in tail {', start)
            terms = [f'CRC[{width - 1 - i}][((head >> {8*i}) & 255) as usize]' for i in range(4)]
            terms += [f'CRC[{width - 1 - i}][bytes[{i}] as usize]' for i in range(4, width)]
            candidate = candidate[:start] + f'''        let (bulk, tail) = part.as_chunks::<{width}>();
        for bytes in bulk {{
            let head = u32::from_le_bytes(bytes[..4].try_into().unwrap()) ^ crc;
            crc = ''' + '\n                ^ '.join(terms) + ';\n        }\n' + candidate[end:]
        (folder / f'crc{width}.rs').write_text(candidate, encoding='utf-8')
        modules.append(f'#[allow(dead_code)] mod crc{width};')
    driver = '\n'.join(modules) + r'''
use std::{hint::black_box,time::{Instant,Duration}};
fn scalar(parts:&[&[u8]])->u32{
 let mut crc=u32::MAX;
 for part in parts{for &b in *part{crc^=u32::from(b);for _ in 0..8{crc=(crc>>1)^if crc&1!=0{0xedb88320}else{0};}}} !crc
}
fn main(){
 let functions:[fn(&[&[u8]])->u32;3]=[crc8::crc32,crc16::crc32,crc32::crc32];
 let mut seed=17u32;
 let data:Vec<u8>=(0..4*1024*1024).map(|_|{seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;seed as u8}).collect();
 // Independent polynomial recurrence and fragmentation around every slicing
 // boundary. No candidate is used as the correctness oracle.
 for n in (0..=129).chain([255,256,257,4096,65535,65536,65537]){
  let expected=scalar(&[&data[..n]]);
  for split in 0..=n.min(65){for f in functions{assert_eq!(f(&[&data[..split],&[],&data[split..n]]),expected);}}
 }
 for len in [64,4096,262144,4*1024*1024]{let input=&data[..len];
  let batch=|mode:usize,count:usize|{let start=Instant::now();for _ in 0..count{black_box(functions[mode](&[black_box(input)]));}start.elapsed()};
  let mut counts=[1;3];for(mode,count)in counts.iter_mut().enumerate(){while batch(mode,*count)<Duration::from_millis(25){*count*=2;}}
  let mut samples=[Vec::new(),Vec::new(),Vec::new()];
  for round in 0..9{for n in 0..3{let mode=(round+n)%3;samples[mode].push(batch(mode,counts[mode]).as_secs_f64()*1e6/counts[mode]as f64);}}
  println!("{{\"bytes\":{},\"iterations\":{:?},\"samples_us\":{:?}}}",len,counts,samples);
 }
}
'''
    (folder / 'main.rs').write_text(driver, encoding='utf-8')
    exe = folder / ('trial.exe' if sys.platform == 'win32' else 'trial')
    subprocess.run(['rustc', '--edition=2024', '-O', str(folder / 'main.rs'), '-o', str(exe)], check=True)
    rows = subprocess.check_output([str(exe)], text=True).splitlines()
    print(json.dumps(dict(experiment='safe_crc_slicing_widths', widths=[8,16,32], source_revision=BASELINE,
        compiler=subprocess.check_output(['rustc','--version','--verbose'],text=True), platform=platform.platform(),
        rustflags=os.environ.get('RUSTFLAGS',''), checksum_sha256=hashlib.sha256(source.encode()).hexdigest(),
        driver_sha256=hashlib.sha256(driver.encode()).hexdigest(), cases=list(map(json.loads,rows)))))
