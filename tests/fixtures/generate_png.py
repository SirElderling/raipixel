"""Original fixture generator. Python stdlib/zlib only; not a Rust test dependency.
Standards: W3C PNG Third Edition sections 8-11; RFC 1950/1951.
Fixtures store input samples, not expected output from another decoder.
Run from the repository root: python tests/fixtures/generate_png.py
"""
import binascii
import json
import struct
import zlib
from pathlib import Path

SIGNATURE = b'\x89PNG\r\n\x1a\n'
PASSES = [(0,0,8,8),(4,0,8,8),(0,4,4,8),(2,0,4,4),(0,2,2,4),(1,0,2,2),(0,1,1,2)]


def chunk(name, data=b''):
    return struct.pack('>I',len(data))+name+data+struct.pack('>I',binascii.crc32(name+data))


def ihdr(w,h,depth,color,interlace=0):
    return chunk(b'IHDR',struct.pack('>IIBBBBB',w,h,depth,color,0,0,interlace))


def png(header, data, extra=b''):
    return SIGNATURE+header+extra+chunk(b'IDAT',data)+chunk(b'IEND')


def paeth(a,b,c):
    distances=[abs(b-c),abs(a-c),abs(a+b-2*c)]
    return [a,b,c][distances.index(min(distances))]


def filtered(row,previous,bpp,kind):
    out=bytearray([kind])
    for i,value in enumerate(row):
        a=row[i-bpp] if i>=bpp else 0
        b=previous[i] if previous else 0
        c=previous[i-bpp] if previous and i>=bpp else 0
        prediction=[0,a,b,(a+b)//2,paeth(a,b,c)][kind]
        out.append((value-prediction)&255)
    return bytes(out)


def pack(samples,depth):
    if depth==8:
        return bytes(samples)
    out=bytearray((len(samples)*depth+7)//8)
    for index,sample in enumerate(samples):
        out[index*depth//8]|=sample<<(8-depth-(index*depth%8))
    return bytes(out)


def profile(color,depth,interlace):
    w,h=9,9
    maximum=(1<<depth)-1
    palette=[[(i*41)&255,(i*83)&255,(i*127)&255] for i in range(min(1<<depth,16))]
    transparency=bytes([0,71,255])[:len(palette)]
    raw_pixels=[]
    expected=[]
    for y in range(h):
        row=[]
        for x in range(w):
            if color==0:
                g=(x+3*y)% (maximum+1)
                row.append([g]); expected.extend([g*255//maximum]*3+[0 if g==1 else 255])
            elif color==3:
                index=(x+3*y)%len(palette)
                row.append([index]); expected.extend(palette[index]+[transparency[index] if index<len(transparency) else 255])
            else:
                r,g,b=(x*19+y*7)&255,(x*37+y*11)&255,(x*71+y*13)&255
                a=(x*29+y*31)&255
                if color==2:
                    row.append([r,g,b]);expected.extend([r,g,b,0 if (r,g,b)==(0,0,0) else 255])
                elif color==4:
                    row.append([g,a]);expected.extend([g,g,g,a])
                else:
                    row.append([r,g,b,a]);expected.extend([r,g,b,a])
        raw_pixels.append(row)
    extra=b''
    if color==3:
        extra=chunk(b'PLTE',bytes(sum(palette,[])))+chunk(b'tRNS',transparency)
    elif color==0:
        extra=chunk(b'tRNS',b'\0\1')
    elif color==2:
        extra=chunk(b'tRNS',b'\0'*6)
    channels={0:1,2:3,3:1,4:2,6:4}[color]
    raw=bytearray()
    for pi,(sx,sy,dx,dy) in enumerate(PASSES if interlace else [(0,0,1,1)]):
        previous=b''
        for ri,y in enumerate(range(sy,h,dy)):
            samples=[sample for x in range(sx,w,dx) for sample in raw_pixels[y][x]]
            if not samples:
                continue
            row=pack(samples,depth)
            raw.extend(filtered(row,previous,max(1,(channels*depth+7)//8),(pi+ri)%5))
            previous=row
    return {'name':f'color{color}-depth{depth}-adam{interlace}',
            'png':png(ihdr(w,h,depth,color,interlace),zlib.compress(raw),extra).hex(),
            'width':w,'height':h,'rgba':bytes(expected).hex(),'color_warning':False,'ancillary_warning':False,'trailing':0}


valid=[profile(color,depth,interlace) for color,depths in [(0,[1,2,4,8]),(2,[8]),(3,[1,2,4,8]),(4,[8]),(6,[8])] for depth in depths for interlace in [0,1]]
base=ihdr(1,1,8,6)
raw=b'\0\x01\x02\x03\0'
compressed=zlib.compress(raw)

def good(name,data,**warnings):
    fixture={'name':name,'png':data.hex(),'width':1,'height':1,'rgba':'01020300','color_warning':False,'ancillary_warning':False,'trailing':0}
    fixture.update(warnings);valid.append(fixture)


good('empty-and-split-IDAT',SIGNATURE+base+chunk(b'IDAT')+chunk(b'IDAT',compressed[:3])+chunk(b'IDAT',compressed[3:])+chunk(b'IEND'))
good('uninterpreted-color',png(base,compressed,chunk(b'gAMA',struct.pack('>I',45455))),color_warning=True)
good('skipped-ancillary',png(base,compressed,chunk(b'tEXt',b'Note\0inert')),ancillary_warning=True)
good('trailing-zlib-bytes',png(base,compressed+b'xyz'),trailing=3)
for interlace in [0,1]:
    # Empty Adam7 passes in a one-pixel image have no filter byte.
    good('one-pixel-adam'+str(interlace),png(ihdr(1,1,8,6,interlace),compressed))

good('masked-tRNS-gray',png(ihdr(1,1,2,0),zlib.compress(b'\0\x40'),chunk(b'tRNS',b'\xff\xfd')))
valid[-1]['rgba']='55555500'
good('masked-tRNS-rgb',png(ihdr(1,1,8,2),zlib.compress(b'\0\1\2\3'),chunk(b'tRNS',b'\xff\1\xff\2\xff\3')))

good('reserved-ancillary',png(base,compressed,chunk(b'abcd',b'inert')),ancillary_warning=True)

invalid=[]
def bad(name,data,code):
    invalid.append({'name':name,'png':data.hex(),'code':code})

original=png(base,compressed)
bad('signature',b'bad'+original[3:],'E_PNG_SIGNATURE')
bad('CRC',original[:-1]+bytes([original[-1]^1]),'E_PNG_CRC')
bad('missing-IEND',original[:-12],'E_PNG_CHUNK')
bad('trailing-after-IEND',original+b'x','E_PNG_CHUNK')
bad('duplicate-IHDR',png(base,compressed,base),'E_PNG_CHUNK')
bad('IHDR-not-first',SIGNATURE+chunk(b'tEXt',b'n\0x')+original[8:],'E_PNG_CHUNK')
bad('zero-width',png(ihdr(0,1,8,6),compressed),'E_PNG_CHUNK')
bad('illegal-depth',png(ihdr(1,1,4,6),compressed),'E_PNG_CHUNK')
bad('16-bit',png(ihdr(1,1,16,6),compressed),'E_UNSUPPORTED_DEPTH')
bad('reserved-critical',png(base,compressed,chunk(b'ABcD')),'E_UNSUPPORTED_PNG')
bad('unknown-critical',png(base,compressed,chunk(b'ABCD')),'E_UNSUPPORTED_PNG')
for name in [b'acTL',b'fcTL',b'fdAT']:
    bad(name.decode(),png(base,compressed,chunk(name)),'E_UNSUPPORTED_ANIMATION')
bad('nonconsecutive-IDAT',SIGNATURE+base+chunk(b'IDAT',compressed[:2])+chunk(b'tEXt',b'n\0x')+chunk(b'IDAT',compressed[2:])+chunk(b'IEND'),'E_PNG_CHUNK')
bad('missing-palette',png(ihdr(1,1,1,3),zlib.compress(b'\0\0')),'E_PNG_CHUNK')
bad('palette-index',png(ihdr(1,1,8,3),zlib.compress(b'\0\1'),chunk(b'PLTE',b'\0\0\0')),'E_PNG_CHUNK')
bad('PLTE-for-gray',png(ihdr(1,1,8,0),zlib.compress(b'\0\0'),chunk(b'PLTE',b'\0\0\0')),'E_PNG_CHUNK')
bad('tRNS-for-RGBA',png(base,compressed,chunk(b'tRNS',b'\0')),'E_PNG_CHUNK')
bad('invalid-filter',png(base,zlib.compress(b'\5\1\2\3\0')),'E_PNG_FILTER')
bad('short-inflate',png(base,zlib.compress(raw[:-1])),'E_PNG_DEFLATE')
bad('long-inflate',png(base,zlib.compress(raw+b'\0')),'E_PNG_DEFLATE')
bad('bad-Adler',png(base,compressed[:-1]+bytes([compressed[-1]^1])),'E_PNG_ADLER')
bad('truncated-zlib',png(base,compressed[:3]),'E_PNG_DEFLATE')

# Independent compression fixtures exercise all required DEFLATE block kinds.
deflate=[]
for name,payload,level,strategy,kind in [
    ('stored',b'raipixel\0'*100,0,zlib.Z_DEFAULT_STRATEGY,0),
    ('fixed-overlap',b'A'*2000,6,zlib.Z_FIXED,1),
    ('dynamic',(b'raipixel makes editable pixel artwork and animations. '*100),6,zlib.Z_DEFAULT_STRATEGY,2),
]:
    compressor=zlib.compressobj(level,zlib.DEFLATED,15,8,strategy)
    stream=compressor.compress(payload)+compressor.flush()
    assert (stream[2]>>1)&3==kind,(name,stream[:8].hex())
    deflate.append({'name':name,'zlib':stream.hex(),'data':payload.hex()})

# Explicit format cases from RFC 1950/1951, authored as bit fields.
class Bits:
    def __init__(self): self.bits=[]
    def put(self,value,count): self.bits.extend((value>>i)&1 for i in range(count))
    def symbol(self,symbol,lengths):
        counts=[0]*16
        for length in lengths:
            if length: counts[length]+=1
        next_code=[0]*16
        for length in range(1,16): next_code[length]=(next_code[length-1]+counts[length-1])*2
        for index,length in enumerate(lengths):
            if not length: continue
            code=next_code[length];next_code[length]+=1
            if index==symbol:
                # Huffman code order is MSB-first within the LSB-packed stream.
                self.bits.extend((code>>i)&1 for i in reversed(range(length)))
                return
        raise AssertionError(symbol)
    def finish(self):
        out=bytearray((len(self.bits)+7)//8)
        for index,bit in enumerate(self.bits): out[index//8]|=bit<<(index%8)
        return bytes(out)


def wrapped(bits,payload,cmf=0x78):
    flag=next(flag for flag in range(32) if (cmf*256+flag)%31==0)
    return bytes([cmf,flag])+bits.finish()+struct.pack('>I',zlib.adler32(payload))


fixed_lengths=[8]*144+[9]*112+[7]*24+[8]*8
# Each invalid stream is wrapped with valid PNG framing and CRC so the error
# must be detected by the inflater, not the outer container.
def bad_stream(name,stream,header=base):
    bad(name,png(header,stream),'E_PNG_DEFLATE')

for name,cmf,flag in [('method',0x79,0),('CINFO',0x88,0),('FDICT',0x78,32)]:
    flag=next(f for f in range(flag,flag+32) if (cmf*256+f)%31==0)
    bad_stream(name,bytes([cmf,flag])+compressed[2:])
bad_stream('FCHECK',bytes([compressed[0],compressed[1]^1])+compressed[2:])
bad_stream('stored-LEN-NLEN',b'\x78\x01\x01\x05\x00\x00\x00'+raw+struct.pack('>I',zlib.adler32(raw)))
b=Bits();b.put(1,1);b.put(3,2);bad_stream('reserved-block',wrapped(b,raw))
b=Bits();b.put(1,1);b.put(1,2);b.symbol(286,fixed_lengths);bad_stream('reserved-literal',wrapped(b,raw))
b=Bits();b.put(1,1);b.put(1,2);b.symbol(257,fixed_lengths);b.symbol(0,[5]*32);bad_stream('distance-before-history',wrapped(b,raw))
b=Bits();b.put(1,1);b.put(1,2);b.symbol(65,fixed_lengths);b.symbol(257,fixed_lengths);b.symbol(30,[5]*32);bad_stream('reserved-distance',wrapped(b,raw))
b=Bits();b.put(1,1);b.put(1,2)
for _ in range(257): b.symbol(65,fixed_lengths)
b.symbol(257,fixed_lengths);b.symbol(16,[5]*32);b.put(0,7)
bad_stream('advertised-window',wrapped(b,b'A'*260,cmf=0x08),ihdr(259,1,8,0))


def dynamic_header(lengths,hlit=0):
    order=[16,17,18,0,8,7,9,6,10,5,11,4,12,3,13,2,14,1,15]
    count=max(4,max(i+1 for i,symbol in enumerate(order) if lengths[symbol]))
    b=Bits();b.put(1,1);b.put(2,2);b.put(hlit,5);b.put(0,5);b.put(count-4,4)
    for symbol in order[:count]: b.put(lengths[symbol],3)
    return b

lengths=[0]*19;lengths[0]=lengths[16]=1
b=dynamic_header(lengths);b.symbol(16,lengths);b.put(0,2)
bad_stream('repeat-without-previous',wrapped(b,raw))
lengths=[0]*19;lengths[0]=lengths[16]=lengths[17]=1
bad_stream('oversubscribed-code-tree',wrapped(dynamic_header(lengths),raw))
lengths=[0]*19;lengths[0]=1
bad_stream('incomplete-code-tree',wrapped(dynamic_header(lengths),raw))
lengths=[0]*19;lengths[0]=lengths[18]=1
b=dynamic_header(lengths)
for _ in range(2): b.symbol(18,lengths);b.put(127,7)
bad_stream('repeat-overrun',wrapped(b,raw))
b=dynamic_header(lengths);b.symbol(18,lengths);b.put(127,7);b.symbol(18,lengths);b.put(109,7)
bad_stream('missing-end-code',wrapped(b,raw))
b=Bits();b.put(1,1);b.put(2,2);b.put(30,5);b.put(0,5);b.put(0,4)
bad_stream('reserved-HLIT',wrapped(b,raw))

# Valid degenerate trees: one EOB or two literal codes, no distance alphabet
# (or the permitted single one-bit distance code, unused by these blocks).
for name,literal,distance_length in [('empty-single-EOB',False,0),('literal-only-empty-distance',True,0),('literal-only-single-distance',True,1)]:
    lengths=[0]*19;lengths[0]=1;lengths[1]=lengths[18]=2
    b=dynamic_header(lengths)
    def zeros(count):
        while count>=11:
            amount=min(138,count);b.symbol(18,lengths);b.put(amount-11,7);count-=amount
        for _ in range(count): b.symbol(0,lengths)
    if literal:
        zeros(65);b.symbol(1,lengths);zeros(190)
    else: zeros(256)
    b.symbol(1,lengths);b.symbol(distance_length,lengths)
    if literal: b.put(0,1);b.put(1,1)
    else: b.put(0,1)
    payload=b'A' if literal else b''
    stream=wrapped(b,payload)
    assert zlib.decompress(stream)==payload,name
    deflate.append({'name':name,'zlib':stream.hex(),'data':payload.hex()})
compressor=zlib.compressobj()
payload=b'A'*1000+b'B'*1000
stream=compressor.compress(payload[:1000])+compressor.flush(zlib.Z_SYNC_FLUSH)+compressor.compress(payload[1000:])+compressor.flush()
assert zlib.decompress(stream)==payload
deflate.append({'name':'mixed-blocks','zlib':stream.hex(),'data':payload.hex()})

folder=Path('tests/fixtures')
writer_raw=b'\0\1\2\3\0\4\5\6\xff'
writer_zlib=b'\x78\x01'
writer_block=b'\x01'+struct.pack('<HH',len(writer_raw),len(writer_raw)^0xffff)+writer_raw
writer=(SIGNATURE+ihdr(2,1,8,6)+chunk(b'IDAT',writer_zlib)+chunk(b'IDAT',writer_block)+chunk(b'IDAT',struct.pack('>I',zlib.adler32(writer_raw)))+chunk(b'IEND')).hex()
compressed_raw=b'\1\1\2\3\0'
compressor=zlib.compressobj(level=1,strategy=zlib.Z_FIXED)
compressed_stream=compressor.compress(compressed_raw)+compressor.flush()
compressed_writer=(SIGNATURE+ihdr(1,1,8,6)+chunk(b'IDAT',compressed_stream[:2])+chunk(b'IDAT',compressed_stream[2:-4])+chunk(b'IDAT',compressed_stream[-4:])+chunk(b'IEND')).hex()
for name,value in [('png',{'valid':valid,'invalid':invalid,'writer':writer,'compressed_writer':compressed_writer}),('deflate',deflate)]:
    (folder/(name+'.json')).write_text(json.dumps(value,separators=(',',':'))+'\n')
print(f'Wrote {len(valid)} valid PNGs, {len(invalid)} rejected PNGs and {len(deflate)} zlib streams')
