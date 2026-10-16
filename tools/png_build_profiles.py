"""Compare complete PNG calls under matched release profiles via public APIs.

Only this temporary comparison package depends on the locked reference crate.
Usage: python tools/png_build_profiles.py --output report.json INPUT.png [...]
"""
import argparse
import hashlib
import json
import os
import platform
import shutil
import statistics
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--profiles', nargs='+', choices=['default', 'single', 'thin'],
                    default=['default', 'single', 'thin'])
parser.add_argument('--offline', action='store_true')
parser.add_argument('inputs', nargs='+', type=Path)
args = parser.parse_args()
profiles = dict(default=dict(codegen_units=16, lto='false'),
                single=dict(codegen_units=1, lto='false'),
                thin=dict(codegen_units=1, lto='thin'))
order = args.profiles + list(reversed(args.profiles))
operations = ['ours_decode', 'reference_decode', 'ours_stored', 'reference_none',
              'reference_fast', 'reference_balanced', 'ours_compressed', 'ours_compact']


def run(command, **kwargs):
    return subprocess.check_output(command, text=True, **kwargs)


with tempfile.TemporaryDirectory(prefix='raipixel-build-profiles-') as folder:
    folder = Path(folder)
    (folder / 'src').mkdir()
    manifest = '[package]\nname="png-comparison"\nversion="0.0.0"\nedition="2024"\npublish=false\n[dependencies]\npng="=0.18.1"\nraipixel={path=' + json.dumps(str(ROOT)) + '}\n'
    (folder / 'Cargo.toml').write_text(manifest, encoding='utf-8')
    shutil.copyfile(ROOT / 'tools/png_compare.rs', folder / 'src/main.rs')
    shutil.copyfile(ROOT / 'benchmarks/png/reference.lock', folder / 'Cargo.lock')
    executables, builds = {}, {}
    for label in args.profiles:
        settings = profiles[label]
        env = os.environ.copy()
        env.update(CARGO_PROFILE_RELEASE_CODEGEN_UNITS=str(settings['codegen_units']),
                   CARGO_PROFILE_RELEASE_LTO=settings['lto'],
                   CARGO_PROFILE_RELEASE_OPT_LEVEL='3', CARGO_INCREMENTAL='0')
        target = folder / ('target-' + label)
        command = ['cargo', 'build', '--release', '--locked', '--manifest-path',
                   str(folder / 'Cargo.toml'), '--target-dir', str(target)]
        if args.offline:
            command.append('--offline')
        subprocess.run(command, env=env, check=True)
        exe = target / 'release' / ('png-comparison.exe' if os.name == 'nt' else 'png-comparison')
        executables[label] = exe
        builds[label] = dict(**settings, opt_level=3, incremental=False,
                             executable_bytes=exe.stat().st_size,
                             executable_sha256=hashlib.sha256(exe.read_bytes()).hexdigest())
    records, hashes, inputs = [], {}, []
    for index, path in enumerate(args.inputs):
        path = path.resolve()
        inputs.append(dict(input=str(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
        for sequence, label in enumerate(order):
            output = folder / f'outputs-{index}-{sequence}'
            row = json.loads(run([str(executables[label]), 'bench', str(path), '1', str(output)]))
            current = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(output.iterdir())}
            if index in hashes:
                assert hashes[index] == current, 'writer bytes differ across build profiles'
            else:
                hashes[index] = current
            records.append(dict(input_index=index, profile=label, **row))
    summary = []
    for index, path in enumerate(args.inputs):
        results = {}
        for label in args.profiles:
            rows = [r for r in records if r['input_index'] == index and r['profile'] == label]
            values = [statistics.median([v for r in rows for v in r['samples_us'][i]])
                      for i in range(len(operations))]
            results[label] = dict(median_us=dict(zip(operations, values)),
                                  compressed_over_fast=values[6] / values[4],
                                  compact_over_fast=values[7] / values[4],
                                  decode_over_reference=values[0] / values[1])
        summary.append(dict(input=str(path), profiles=results))
    report = dict(experiment='complete_codec_build_profiles',
                  reference='png 0.18.1, locked default features, public APIs only',
                  compiler=run(['rustc', '--version', '--verbose']),
                  platform=platform.platform(), architecture=platform.machine(),
                  rustflags=os.environ.get('RUSTFLAGS', ''),
                  encoded_rustflags=os.environ.get('CARGO_ENCODED_RUSTFLAGS', ''),
                  commit=run(['git', 'rev-parse', 'HEAD'], cwd=ROOT).strip(),
                  driver_sha256=hashlib.sha256((ROOT / 'tools/png_compare.rs').read_bytes()).hexdigest(),
                  source_sha256={str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
                                 for p in sorted(ROOT.glob('src/**/*.rs'))},
                  operations=operations, process_order=order, builds=builds,
                  inputs=inputs, output_sha256=hashes, summary=summary, runs=records)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report))
