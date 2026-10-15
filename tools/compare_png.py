"""Optional independent verification/benchmark runner; never a production dependency.
Requires cargo/rustc and Pillow, downloads the locked reference crate if uncached.
Usage: python tools/compare_png.py --output benchmarks/png/optimized.json
"""
import argparse
import hashlib
import io
import json
import platform
import random
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--allow-encoded-changes', action='store_true')
parser.add_argument('--memory', action='store_true', help='Measure fresh-process peak RSS on Linux')
args = parser.parse_args()
baseline = json.loads((ROOT / 'benchmarks/png/baseline.json').read_text())
expected_hashes = {row['name']: row['sha256']['ours'] for row in baseline['measurements']}
compressed_baseline = json.loads((ROOT / 'benchmarks/png/compressed.json').read_text())
expected_compressed_hashes = {row['name']: row['sha256']['compressed'] for row in compressed_baseline['measurements']}
with tempfile.TemporaryDirectory(prefix='raipixel-png-compare-') as temporary:
    work = Path(temporary)
    (work / 'src').mkdir()
    manifest = '[package]\nname="png-comparison"\nversion="0.0.0"\nedition="2024"\npublish=false\n[dependencies]\npng="=0.18.1"\nraipixel={path=' + json.dumps(str(ROOT)) + '}\n'
    (work / 'Cargo.toml').write_text(manifest)
    shutil.copyfile(ROOT / 'tools/png_compare.rs', work / 'src/main.rs')
    shutil.copyfile(ROOT / 'benchmarks/png/reference.lock', work / 'Cargo.lock')
    subprocess.run(['cargo', 'build', '--release', '--locked', '--manifest-path', str(work / 'Cargo.toml')], check=True)
    exe = work / 'target/release' / ('png-comparison.exe' if platform.system() == 'Windows' else 'png-comparison')
    def run(*arguments):
        return subprocess.run([str(exe), *map(str, arguments)], capture_output=True, check=True)
    run('features')
    profile = []
    for index, case in enumerate(json.loads((ROOT / 'tests/fixtures/png.json').read_text())['valid']):
        source = work / f'profile-{index}.png'
        source.write_bytes(bytes.fromhex(case['png']))
        expected = bytes.fromhex(case['rgba'])
        ours = run('verify-ours', source).stdout
        assert ours[8:] == expected, case['name']
        reference = run('verify', source).stdout
        profile.append({'name': case['name'], 'reference': 'exact' if reference[8:] == expected else 'pixels differ'})
        for writer in ['reencode-ours', 'reencode-compressed']:
            encoded = run(writer, source).stdout
            with Image.open(io.BytesIO(encoded)) as image:
                image.load()
                assert image.convert('RGBA').tobytes() == expected, (case['name'], writer)
    icon = Image.new('RGBA', (16, 16), (17, 23, 31, 0))
    draw = ImageDraw.Draw(icon)
    draw.polygon([(8, 0), (10, 3), (8, 9), (6, 9), (6, 3)], fill=(173, 184, 198, 255))
    draw.line([(8, 1), (7, 8)], fill=(240, 244, 250, 255))
    draw.rectangle((4, 9, 10, 10), fill=(197, 145, 42, 255))
    draw.rectangle((7, 11, 8, 14), fill=(98, 51, 26, 255))
    draw.rectangle((6, 15, 9, 15), fill=(197, 145, 42, 255))
    cases = [(f'icon-{n}', icon.resize((n, n), Image.Resampling.NEAREST), count) for n, count in [(16, 2000), (32, 1000), (64, 500)]]
    for n, count in [(256, 50), (1024, 10)]:
        sheet = Image.new('RGBA', (n, n))
        tile = icon.resize((64, 64), Image.Resampling.NEAREST)
        for y in range(0, n, 64):
            for x in range(0, n, 64):
                sheet.paste(tile, (x, y))
        cases.append((f'sheet-{n}', sheet, count))
    for n, count in [(256, 50), (1024, 10)]:
        pixels = bytes(channel for y in range(n) for x in range(n) for channel in ((x * 19 + y * 7) % 256, (x * 37 + y * 11) % 256, (x * 71 + y * 13) % 256, 0 if (x + y) % 7 == 0 else 255))
        cases.append((f'texture-{n}', Image.frombytes('RGBA', (n, n), pixels), count))
    measurements = []
    for name, image, count in cases:
        source = work / f'{name}.png'
        image.save(source, format='PNG', compress_level=6)
        folder = work / name
        result = json.loads(run('bench', source, count, folder).stdout)
        result['name'] = name
        result['sha256'] = {}
        for mode in ['ours', 'none', 'fast', 'default', 'compressed']:
            encoded = (folder / (mode + '.png')).read_bytes()
            result['sha256'][mode] = hashlib.sha256(encoded).hexdigest()
            with Image.open(io.BytesIO(encoded)) as decoded:
                decoded.load()
                assert decoded.convert('RGBA').tobytes() == image.tobytes(), (name, mode)
        result['ours_byte_identical_to_baseline'] = result['sha256']['ours'] == expected_hashes[name]
        result['compressed_byte_identical_to_baseline'] = result['sha256']['compressed'] == expected_compressed_hashes[name]
        if not args.allow_encoded_changes:
            assert result['ours_byte_identical_to_baseline'], name
            assert result['compressed_byte_identical_to_baseline'], name
        measurements.append(result)
        print(json.dumps(result), flush=True)
    noise = Image.frombytes('RGBA', (257, 257), random.Random(17).randbytes(257 * 257 * 4))
    source = work / 'noise.png'
    noise.save(source, format='PNG', compress_level=0)
    encoded = run('reencode-compressed', source).stdout
    compressed_source = work / 'noise-compressed.png'
    compressed_source.write_bytes(encoded)
    assert run('verify', compressed_source).stdout[8:] == noise.tobytes()
    with Image.open(io.BytesIO(encoded)) as image:
        image.load()
        assert image.convert('RGBA').tobytes() == noise.tobytes()
    memory = None
    if args.memory:
        if platform.system() != 'Linux':
            raise SystemExit('--memory currently requires Linux (wait4 peak RSS in KiB)')
        source = work / 'memory-sheet4096.png'
        sheet = Image.new('RGBA', (4096, 4096))
        tile = icon.resize((64, 64), Image.Resampling.NEAREST)
        for y in range(0, 4096, 64):
            for x in range(0, 4096, 64):
                sheet.paste(tile, (x, y))
        sheet.save(source, format='PNG', compress_level=6)
        # A thin parent prevents the image generator's RSS being inherited by
        # freshly spawned measurement processes before exec.
        memory = json.loads(subprocess.check_output([
            sys.executable, str(ROOT / 'tools/png_memory.py'), str(exe), str(source)
        ], text=True))
    report = {'rust': subprocess.check_output(['rustc', '--version'], text=True).strip(),
              'compiler': subprocess.check_output(['rustc', '--version', '--verbose'], text=True),
              'driver_sha256': hashlib.sha256((ROOT / 'tools/png_compare.rs').read_bytes()).hexdigest(),
              'library_sources_sha256': {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(ROOT.glob('src/**/*.rs'))},
              'host': platform.platform(), 'reference': 'png 0.18.1, locked default features',
              'raipixel_commit': subprocess.check_output(['git', '-C', str(ROOT), 'rev-parse', 'HEAD'], text=True).strip(),
              'working_tree': subprocess.check_output(['git', '-C', str(ROOT), 'status', '--porcelain'], text=True),
              'timing_order': baseline['timing_order'] + ['ours_compressed_encode'], 'profile': profile, 'measurements': measurements}
    if memory is not None:
        report['memory'] = memory
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n')
