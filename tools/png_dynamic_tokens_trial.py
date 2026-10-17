"""Recode raipixel's OWN fixed-Huffman DEFLATE tokens using dynamic Huffman.

This is an isolated diagnostic, not a production change or a runtime
dependency. It constructs no new match sequences: input tokens, lengths,
distances, filtered scanlines, and Adler32 are IDENTICAL to the accepted
encode_compact output. The reference png crate implementation is not read.

Each reconstructed PNG must pass Python standard-library zlib decompression
and raipixel's production Rust PNG decoder with EXACT hidden-RGB preservation.
Runs offline with Rust 1.99.1 on native Linux / Windows / ARM64.

Usage: python tools/png_dynamic_tokens_trial.py image.png [...]
"""
import binascii
import collections
import hashlib
import heapq
import json
import os
import platform
import subprocess
import sys
import tempfile
import time
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LB = [3,4,5,6,7,8,9,10,11,13,15,17,19,23,27,31,35,43,51,59,67,83,99,115,131,163,195,227,258]
LE = [0,0,0,0,0,0,0,0,1,1,1,1,2,2,2,2,3,3,3,3,4,4,4,4,5,5,5,5,0]
DB = [1,2,3,4,5,7,9,13,17,25,33,49,65,97,129,193,257,385,513,769,1025,1537,2049,3073,4097,6145,8193,12289,16385,24577]
DE = [0,0,0,0,1,1,2,2,3,3,4,4,5,5,6,6,7,7,8,8,9,9,10,10,11,11,12,12,13,13]
ORDER = [16,17,18,0,8,7,9,6,10,5,11,4,12,3,13,2,14,1,15]

def reverse_bits(value, width):
    result = 0
    for _ in range(width):
        result = (result << 1) | (value & 1)
        value >>= 1
    return result

def canonical(lengths):
    counts = collections.Counter(x for x in lengths if x)
    next_code = {}
    code = 0
    for bits in range(1, max(counts, default=0) + 1):
        code = (code + counts[bits-1]) << 1
        next_code[bits] = code
    result = {}
    for symbol, length in enumerate(lengths):
        if length:
            result[symbol] = (reverse_bits(next_code[length], length), length)
            next_code[length] += 1
    return result

FIXED_LL = canonical([8]*144 + [9]*112 + [7]*24 + [8]*8)
FIXED_DS = canonical([5]*32)
FIXED_LL_READ = {(n,v): s for s,(v,n) in FIXED_LL.items()}
FIXED_DS_READ = {(n,v): s for s,(v,n) in FIXED_DS.items()}

class Reader:
    def __init__(self, data):
        self.data = data
        self.bit = 0
    def take(self, n):
        assert self.bit+n <= len(self.data)*8, "truncated DEFLATE"
        answer = 0
        for offset in range(n):
            answer |= ((self.data[(self.bit+offset)>>3] >> ((self.bit+offset)&7))&1) << offset
        self.bit += n
        return answer
    def align(self):
        self.bit = (self.bit+7)&~7
    def take_bytes(self, n):
        assert self.bit % 8 == 0
        start = self.bit>>3
        self.bit += n*8
        assert self.bit <= len(self.data)*8
        return self.data[start:start+n]
    def symbol(self, decoder, max_length):
        value = 0
        for width in range(1, max_length+1):
            value |= self.take(1) << (width-1)
            found = decoder.get((width,value))
            if found is not None:
                return found
        raise ValueError("bad fixed Huffman symbol")

class Writer:
    def __init__(self):
        self.buffer = bytearray()
        self.bits = 0
        self.count = 0
        self.written = 0
    def put(self,value,count):
        assert 0 <= value < (1<<count) if count else value == 0
        self.bits |= value<<self.count
        self.count += count
        self.written += count
        while self.count >= 8:
            self.buffer.append(self.bits & 255)
            self.bits >>= 8
            self.count -= 8
    def symbol(self,sym, codes):
        value,n=codes[sym]
        self.put(value,n)
    def align(self):
        if self.count:
            self.put(0,8-self.count)
    def raw(self,data):
        assert self.count == 0
        self.buffer.extend(data)
        self.written += len(data)*8
    def bytes(self):
        self.align()
        return bytes(self.buffer)

def fixed_tokens(reader):
    tokens=[]
    while True:
        sym=reader.symbol(FIXED_LL_READ,9)
        if sym < 256:
            tokens.append(sym)
        elif sym == 256:
            return tokens
        else:
            assert 257 <= sym <= 285
            length = LB[sym-257] + reader.take(LE[sym-257])
            dcode = reader.symbol(FIXED_DS_READ,5)
            assert dcode < 30
            distance = DB[dcode]+reader.take(DE[dcode])
            tokens.append((length,distance))

def parse_blocks(compressed):
    r=Reader(compressed)
    blocks=[]
    while True:
        final = r.take(1)
        kind = r.take(2)
        if kind == 0:
            r.align()
            length = r.take(16)
            complement = r.take(16)
            assert length ^ complement == 65535
            payload = r.take_bytes(length)
            blocks.append((final,"stored",payload))
        elif kind == 1:
            tokens = fixed_tokens(r)
            blocks.append((final,"fixed",tokens))
        else:
            raise ValueError(f"expected accepted fixed/stored block, got {kind}")
        if final:
            break
    assert r.bit <= len(compressed)*8
    assert r.bit + 7 >= len(compressed)*8, "unexpected trailing deflate data"
    return blocks

def ref_code(length,distance):
    li=next(i for i,base in enumerate(LB) if i==len(LB)-1 or LB[i+1]>length)
    di=next(i for i,base in enumerate(DB) if i==len(DB)-1 or DB[i+1]>distance)
    assert 0<=length-LB[li] < (1<<LE[li])
    assert 0<=distance-DB[di] < (1<<DE[di])
    return (257+li,length-LB[li],LE[li],di,distance-DB[di],DE[di])

def emit_fixed(out, final, tokens):
    out.put(final | 2,3)
    for token in tokens:
        if isinstance(token,int):
            out.symbol(token,FIXED_LL)
        else:
            sym,lex,leb,dsym,dex,deb=ref_code(*token)
            out.symbol(sym,FIXED_LL)
            out.put(lex,leb)
            out.symbol(dsym,FIXED_DS)
            out.put(dex,deb)
    out.symbol(256,FIXED_LL)

def huffman_lengths(freq,max_len):
    heap=[]
    seq=0
    for symbol,count in enumerate(freq):
        if count:
            heap.append((count,seq,symbol))
            seq+=1
    if not heap:
        return None
    if len(heap)==1:
        result=[0]*len(freq)
        result[heap[0][2]]=1
        return result
    heapq.heapify(heap)
    while len(heap)>1:
        f1,_,a=heapq.heappop(heap)
        f2,_,b=heapq.heappop(heap)
        heapq.heappush(heap,(f1+f2,seq,(a,b)))
        seq+=1
    lengths=[0]*len(freq)
    stack=[(heap[0][2],0)]
    while stack:
        node, depth=stack.pop()
        if isinstance(node,int):
            if depth>max_len:
                return None
            lengths[node]=depth
        else:
            stack.append((node[0],depth+1))
            stack.append((node[1],depth+1))
    return lengths

def rle_lengths(lengths):
    result=[]
    i=0
    while i<len(lengths):
        value=lengths[i]
        end=i+1
        while end<len(lengths) and lengths[end]==value:
            end+=1
        run=end-i
        if value==0:
            while run>=11:
                take=min(138,run)
                result.append((18,take-11,7))
                run-=take
            while run>=3:
                take=min(10,run)
                result.append((17,take-3,3))
                run-=take
            result.extend([(0,0,0)]*run)
        else:
            result.append((value,0,0))
            run-=1
            while run>=3:
                take=min(6,run)
                result.append((16,take-3,2))
                run-=take
            result.extend([(value,0,0)]*run)
        i=end
    return result

def dynamic_params(tokens):
    ll=[0]*286
    ds=[0]*30
    for token in tokens:
        if isinstance(token,int):
            ll[token]+=1
        else:
            sym,_,_,dist,_,_=ref_code(*token)
            ll[sym]+=1
            ds[dist]+=1
    ll[256]+=1
    if not any(ds):
        ds[0]=1
    ll_lengths=huffman_lengths(ll,15)
    dist_lengths=huffman_lengths(ds,15)
    if ll_lengths is None or dist_lengths is None:
        return None
    ll_count=max(257, max(i+1 for i,x in enumerate(ll_lengths) if x))
    ds_count=max(1, max(i+1 for i,x in enumerate(dist_lengths) if x))
    encoded=rle_lengths(ll_lengths[:ll_count]+dist_lengths[:ds_count])
    cl_freq=[0]*19
    for sym,_,_ in encoded:
        cl_freq[sym]+=1
    if sum(bool(x) for x in cl_freq)==1:
        cl_freq[next(i for i in range(19) if not cl_freq[i])]=1
    cl_lengths=huffman_lengths(cl_freq,7)
    if cl_lengths is None:
        return None
    cl_count=max(4,max(i+1 for i,sym in enumerate(ORDER) if cl_lengths[sym]))
    return ll_lengths,dist_lengths,cl_lengths,ll_count,ds_count,cl_count,encoded

def emit_dynamic(out,final,tokens,params):
    ll_lengths,dist_lengths,cl_lengths,ll_count,ds_count,cl_count,encoded=params
    lc=canonical(ll_lengths)
    dc=canonical(dist_lengths)
    cc=canonical(cl_lengths)
    out.put(final | 4,3) # BTYPE=10 (dynamic), LSB first
    out.put(ll_count-257,5)
    out.put(ds_count-1,5)
    out.put(cl_count-4,4)
    for symbol in ORDER[:cl_count]:
        out.put(cl_lengths[symbol],3)
    for symbol,extra,nbits in encoded:
        out.symbol(symbol,cc)
        out.put(extra,nbits)
    for token in tokens:
        if isinstance(token,int):
            out.symbol(token,lc)
        else:
            sym,lex,leb,dist,dex,deb=ref_code(*token)
            out.symbol(sym,lc)
            out.put(lex,leb)
            out.symbol(dist,dc)
            out.put(dex,deb)
    out.symbol(256,lc)

def recode(compressed):
    blocks=parse_blocks(compressed)
    original=Writer()
    changed=Writer()
    dynamic_blocks=0
    fallback_depth=0
    saved_bits=0
    for final,kind,payload in blocks:
        if kind=="stored":
            for writer in (original,changed):
                writer.put(final,3)
                writer.align()
                length=len(payload)
                writer.raw(length.to_bytes(2,"little") + (length^65535).to_bytes(2,"little")+payload)
            continue
        emit_fixed(original,final,payload)
        fixed=Writer()
        emit_fixed(fixed,final,payload)
        params=dynamic_params(payload)
        if params is None:
            fallback_depth+=1
            emit_fixed(changed,final,payload)
            continue
        dyn=Writer()
        emit_dynamic(dyn,final,payload,params)
        if dyn.written < fixed.written:
            emit_dynamic(changed,final,payload,params)
            dynamic_blocks+=1
            saved_bits+=fixed.written-dyn.written
        else:
            emit_fixed(changed,final,payload)
    reference=original.bytes()
    assert reference==compressed, "fixed parser/serializer does not reconstruct original DEFLATE bytes"
    modified=changed.bytes()
    return modified,dict(total_blocks=len(blocks),fixed_blocks=sum(k=="fixed" for _,k,_ in blocks),
                         dynamic_blocks=dynamic_blocks,skipped_max_code_length=fallback_depth,
                         theoretical_bits_saved=saved_bits)

def png_chunks(png):
    assert png[:8]==b"\x89PNG\r\n\x1a\n"
    pos=8
    idat=bytearray()
    ihdr=None
    while pos<len(png):
        n=int.from_bytes(png[pos:pos+4],"big")
        kind=png[pos+4:pos+8]
        data=png[pos+8:pos+8+n]
        crc=int.from_bytes(png[pos+8+n:pos+12+n],"big")
        assert crc == binascii.crc32(kind+data)
        if kind==b"IHDR":
            ihdr=data
        if kind==b"IDAT":
            idat.extend(data)
        pos+=n+12
    assert pos==len(png) and ihdr is not None
    return ihdr,bytes(idat)

def chunk(kind,payload):
    return len(payload).to_bytes(4,"big")+kind+payload+binascii.crc32(kind+payload).to_bytes(4,"big")

def png_with_zlib(ihdr,stream):
    return b"\x89PNG\r\n\x1a\n"+chunk(b"IHDR",ihdr)+chunk(b"IDAT",stream)+chunk(b"IEND",b"")

DRIVER=r'''
#![forbid(unsafe_code)]
use raipixel::{png,Limits,Work};
fn main()->Result<(),Box<dyn std::error::Error>>{
    let args:Vec<String>=std::env::args().collect();
    let original=png::decode(&std::fs::read(&args[2])?,
        Limits::default(),&mut Work::default())?.image;
    if args[1]=="prepare"{
        let mut encoded=Vec::new();
        png::encode_compact(&original,&mut encoded,&mut Work::default())?;
        std::fs::write(&args[3],encoded)?;
    }else{
        let decoded=png::decode(&std::fs::read(&args[3])?,
            Limits::default(),&mut Work::default())?.image;
        assert_eq!(original,decoded,"full RGBA changed, including hidden RGB");
    }
    Ok(())
}
'''

with tempfile.TemporaryDirectory(prefix="raipixel-dynamic-tokens-") as location:
    directory=Path(location)
    (directory/"src").mkdir()
    (directory/"Cargo.toml").write_text(
        '[package]\nname="raipixel-dynamic-token-screen"\nversion="0.0.0"\nedition="2024"\n'
        '[dependencies]\nraipixel={path='+json.dumps(str(ROOT))+'}\n',encoding="utf-8")
    (directory/"src/main.rs").write_text(DRIVER,encoding="utf-8")
    subprocess.run(["cargo","build","--release","--offline","--manifest-path",str(directory/"Cargo.toml")],check=True)
    exe=directory/"target/release"/("raipixel-dynamic-token-screen.exe" if os.name=="nt" else "raipixel-dynamic-token-screen")
    results=[]
    for case,path in enumerate(map(Path,sys.argv[1:])):
        compact=directory/f"accepted-{case}.png"
        subprocess.run([str(exe),"prepare",str(path),str(compact)],check=True)
        content=compact.read_bytes()
        ihdr,stream=png_chunks(content)
        assert zlib.decompress(stream)
        start=time.perf_counter()
        changed,info=recode(stream[2:-4])
        recoded=stream[:2]+changed+stream[-4:]
        assert zlib.decompress(recoded)==zlib.decompress(stream)
        candidate=png_with_zlib(ihdr,recoded)
        out=directory/f"dynamic-{case}.png"
        out.write_bytes(candidate)
        subprocess.run([str(exe),"verify",str(path),str(out)],check=True)
        results.append(dict(input=str(path),input_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                            current_png_bytes=len(content),candidate_png_bytes=len(candidate),
                            current_zlib_bytes=len(stream),candidate_zlib_bytes=len(recoded),
                            ratio_zlib=len(recoded)/len(stream),ratio_png=len(candidate)/len(content),
                            python_recode_ms=(time.perf_counter()-start)*1000,
                            sha256=hashlib.sha256(candidate).hexdigest(),**info))
    print(json.dumps(dict(experiment="own_match_stream_fixed_vs_dynamic_huffman_exact_tokens",
                          reference="production encode_compact, no match changes, no external crate",
                          compiler=subprocess.check_output(["rustc","--version","--verbose"],text=True),
                          platform=platform.platform(),python_zlib=zlib.ZLIB_RUNTIME_VERSION,
                          source_sha256=hashlib.sha256((ROOT/"src/deflate/encode.rs").read_bytes()).hexdigest(),
                          driver_sha256=hashlib.sha256(DRIVER.encode()).hexdigest(),
                          all_outputs_verified_by_zlib_and_production_rust_decoder=True,
                          cases=results)))
