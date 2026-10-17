"""Screen DEFLATE emission cost by replaying our actual token choices.
Capture/validation is untimed. Replay has different cache/branch behavior, so
full-minus-replay is NOT an exact measurement of matching time.
Usage: python tools/png_token_replay.py INPUT.png [INPUT.png ...]
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
source = (ROOT / 'src/deflate/encode.rs').read_text()
traced = source.replace('    dictionary: Vec<u32>,','    dictionary: Vec<u32>,\n    trace: Vec<(u16,u16)>,',1)
traced = traced.replace('            dictionary,','            dictionary,\n            trace: Vec::new(),',1)
traced = traced.replace('        let saved = (self.bits, self.count);\n        self.output.clear();',
                        '        let saved = (self.bits, self.count);\n        self.output.clear();\n        self.trace.clear();',1)
# The sampled stored path never creates Huffman tokens. Mark that decision
# explicitly so replay validates the real stored bytes rather than fabricating
# an empty fixed-Huffman block.
if 'Self::probably_stored(data)' in source:
    traced = traced.replace(
        'if data.len() >= 16_384 && Self::probably_stored(data) {',
        'if data.len() >= 16_384 && Self::probably_stored(data) {\n            self.trace.push((u16::MAX, 0));',
        1,
    )
traced = traced.replace('                self.reference(length, pos - prior);','                self.trace.push((length as u16,(pos-prior)as u16));\n                self.reference(length, pos - prior);',1)
# The optional compact lookahead emits its own literal. Instrument the
# normal fast literal fallback instead; the fast mode never takes lookahead.
traced = traced.replace('            } else {\n                self.symbol(u32::from(data[pos]));',
                        '            } else {\n                self.trace.push((0,u16::from(data[pos])));\n                self.symbol(u32::from(data[pos]));',1)
traced = traced.replace('            self.symbol(u32::from(byte));','            self.trace.push((0,u16::from(byte)));\n            self.symbol(u32::from(byte));',1)
fallback = source[source.index('        if self.output.len() > data.len() + 5 {'):source.index('\n#[cfg(test)]')]
# Preserve the complete original fallback and closing method/impl braces.
marker = '    /// Block-local dictionary'
assert traced.count(marker) == 1
traced = traced.replace(marker, '''    pub(crate) fn take_trace(&mut self) -> Vec<(u16,u16)> { std::mem::take(&mut self.trace) }
    pub(crate) fn replay(&mut self, data:&[u8], last:bool, tokens:&[(u16,u16)]) -> &[u8] {
        if tokens.len()==1 && tokens[0].0==u16::MAX {
            self.store(data,last);
            return &self.output;
        }
        let saved=(self.bits,self.count);
        self.output.clear();self.put(u32::from(last)|2,3);
        for &(length,value) in tokens {
            if length==0 {self.symbol(u32::from(value));}
            else {self.reference(usize::from(length),usize::from(value));}
        }
        self.symbol(256);if last {self.align();}
''' + fallback[:fallback.rfind('\n}')] + '\n\n' + marker, 1)
constants = (ROOT / 'src/deflate.rs').read_text()
constants = constants[constants.index('const LENGTH_BASE:'):constants.index('/// Decode one zlib stream')]
with tempfile.TemporaryDirectory(prefix='raipixel-token-replay-') as temporary:
    temporary = Path(temporary)
    (temporary / 'traced.rs').write_text(traced)
    inputs = []
    for i,path in enumerate(sys.argv[1:]):
        data=Path(path).read_bytes();offset=8;idat=bytearray()
        while offset<len(data):
            count=int.from_bytes(data[offset:offset+4],'big')
            if data[offset+4:offset+8]==b'IDAT':idat.extend(data[offset+8:offset+8+count])
            offset+=count+12
        raw=zlib.decompress(idat)
        (temporary/f'{i}.raw').write_bytes(raw)
        inputs.append(dict(input=path,raw_sha256=hashlib.sha256(raw).hexdigest()))
    if 'fn flush_bytes(' in source:
        # Preserve the production writer's end-of-block byte drain.
        traced = traced.replace('self.symbol(256);if last {self.align();}',
                                'self.symbol(256);if last {self.align();}self.flush_bytes();')
        (temporary/'traced.rs').write_text(traced)
    driver='#![forbid(unsafe_code)]\n#![allow(dead_code)]\n'
    driver+='#[path='+json.dumps(str(ROOT/'src/error.rs'))+']mod error;pub use error::{Error,Resource};\n'+constants
    driver+='#[path='+json.dumps(str(ROOT/'src/deflate/encode.rs'))+']mod actual;\nmod traced;\n'
    driver+='''
use std::{hint::black_box,time::{Instant,Duration}};
fn main(){for path in std::env::args().skip(1){
    let data=std::fs::read(path).unwrap();let n=data.len().div_ceil(65535);
    let mut capture=traced::Encoder::new(data.len().min(65535)).unwrap();
    let mut traces=Vec::new();let mut expected=Vec::new();
    for(i,b)in data.chunks(65535).enumerate(){expected.extend_from_slice(capture.block(b,i+1==n));traces.push(capture.take_trace());}
    let literals=traces.iter().flatten().filter(|t|t.0==0).count();
    let references=traces.iter().flatten().filter(|t|t.0!=0).count();
    let operation=|mode|{let mut out=Vec::new();
        if mode==0{let mut e=actual::Encoder::new(data.len().min(65535)).unwrap();for(i,b)in data.chunks(65535).enumerate(){out.extend_from_slice(e.block(black_box(b),i+1==n));}}
        else{let mut e=traced::Encoder::new(data.len().min(65535)).unwrap();for(i,b)in data.chunks(65535).enumerate(){out.extend_from_slice(e.replay(black_box(b),i+1==n,black_box(&traces[i])));}}
        out
    };
    assert_eq!(operation(0),expected);assert_eq!(operation(1),expected);
    let batch=|mode,count|{let start=Instant::now();for _ in 0..count{black_box(operation(mode));}start.elapsed()};
    let mut counts=[1;2];for(mode,count)in counts.iter_mut().enumerate(){while batch(mode,*count)<Duration::from_millis(25){*count*=2;}}
    let mut samples=[Vec::new(),Vec::new()];for round in 0..9{for x in 0..2{let mode=(round+x)%2;samples[mode].push(batch(mode,counts[mode]).as_secs_f64()*1e6/counts[mode]as f64);}}
    println!("{{\\"raw_bytes\\":{},\\"literals\\":{},\\"references\\":{},\\"encoded_bytes\\":{},\\"iterations\\":{:?},\\"samples_us\\":{:?}}}",data.len(),literals,references,expected.len(),counts,samples);
}}
'''
    (temporary/'main.rs').write_text(driver)
    exe=temporary/('replay.exe' if sys.platform=='win32' else 'replay')
    subprocess.run(['rustc','--edition=2024','-O',str(temporary/'main.rs'),'-o',str(exe)],check=True)
    rows=subprocess.check_output([str(exe),*[str(temporary/f'{i}.raw') for i in range(len(inputs))]],text=True).splitlines()
    print(json.dumps(dict(experiment='token_replay',modes=['full_deflate','emission_replay'],
        platform=platform.platform(),compiler=subprocess.check_output(['rustc','--version','--verbose'],text=True),
        encoder_sha256=hashlib.sha256(source.encode()).hexdigest(),
        cases=[dict(**item,**json.loads(row)) for item,row in zip(inputs,rows)])))
