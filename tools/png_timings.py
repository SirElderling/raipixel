"""Paired full-codec measurements using only Python/Rust standard libraries.
Usage: python tools/png_timings.py --baseline SHA --output timings.json
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
parser.add_argument('--baseline', required=True)
parser.add_argument('--output', required=True, type=Path)
parser.add_argument('--batch-ms', default=25, type=int)
parser.add_argument('--corpus', type=Path, help='Recorded PNG hex and independently verified RGBA hashes')
args = parser.parse_args()

def run(*command, cwd=ROOT, capture=True):
    return subprocess.run(command, cwd=cwd, check=True, text=True,
                          stdout=subprocess.PIPE if capture else None).stdout

def revision(ref):
    return run('git', 'rev-parse', '--verify', ref + '^{commit}').strip()

baseline = revision(args.baseline)
current = revision('HEAD')
metadata = dict(baseline=baseline, current=current,
                working_tree=run('git', 'status', '--porcelain'),
                compiler=run('rustc', '--version', '--verbose'),
                platform=platform.platform(), architecture=platform.machine(),
                processor=platform.processor(), python=platform.python_version(),
                rustflags=os.environ.get('RUSTFLAGS', ''),
                github_run=os.environ.get('GITHUB_RUN_ID'),
                minimum_batch_ms=args.batch_ms,
                operation_order=['decode_input', 'decode_stored', 'encode_stored', 'encode_compressed'],
                process_order=['baseline', 'current', 'current', 'baseline'],
                process_scope='case')
metadata['driver_sha256'] = hashlib.sha256((ROOT / 'examples/png_bench.rs').read_bytes()).hexdigest()
metadata['library_sources_sha256'] = {
    str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
    for p in sorted([ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', *ROOT.glob('src/**/*.rs')])
}
records = []
with tempfile.TemporaryDirectory(prefix='raipixel-timings-') as temporary:
    temporary = Path(temporary)
    checkout = temporary / 'baseline'
    run('git', 'worktree', 'add', '--detach', str(checkout), baseline)
    try:
        # Identical driver, compiler and profile for both versions. Building is
        # outside all measurements; never edit the baseline library sources.
        shutil.copyfile(ROOT / 'examples/png_bench.rs', checkout / 'examples/png_bench.rs')
        executables = {}
        for label, directory in [('baseline', checkout), ('current', ROOT)]:
            target = temporary / ('target-' + label)
            run('cargo', 'build', '--release', '--locked', '--offline', '--example',
                'png_bench', '--target-dir', str(target), cwd=directory, capture=False)
            executables[label] = target / 'release/examples' / ('png_bench.exe' if os.name == 'nt' else 'png_bench')
        metadata['benchmark_executable_bytes'] = {label: exe.stat().st_size for label, exe in executables.items()}
        corpus = None
        if args.corpus:
            corpus_bytes = args.corpus.read_bytes()
            corpus = json.loads(corpus_bytes)['cases']
            metadata['corpus_sha256'] = hashlib.sha256(corpus_bytes).hexdigest()
            metadata['inputs_sha256'] = {}
            for item in corpus:
                data = bytes.fromhex(item['png_hex'])
                assert hashlib.sha256(data).hexdigest() == item['png_sha256']
                source = temporary / (item['name'] + '.png')
                source.write_bytes(data)
                item['temporary_path'] = str(source)
                metadata['inputs_sha256'][item['name']] = item['png_sha256']
        hashes = {}
        for label, exe in executables.items():
            folder = temporary / ('outputs-' + label)
            if corpus is None:
                run(str(exe), '--export', str(folder))
            else:
                for item in corpus:
                    target = folder / item['name']
                    run(str(exe), '--export-file', str(target), item['temporary_path'])
                    assert hashlib.sha256((target / 'rgba.bin').read_bytes()).hexdigest() == item['rgba_sha256'], item['name']
            hashes[label] = {p.relative_to(folder).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
                             for p in sorted(folder.rglob('*')) if p.is_file()}
        assert hashes['baseline'] == hashes['current'], 'samples, flags, work or encoded PNG bytes changed'
        cases = (sorted(name.removesuffix('-compressed.png') for name in hashes['current'] if name.endswith('-compressed.png'))
                 if corpus is None else [item['name'] for item in corpus])
        for case in cases:
            for label in metadata['process_order']:
                if corpus is None:
                    arguments = ['--suite', str(args.batch_ms), case]
                else:
                    item = next(item for item in corpus if item['name'] == case)
                    arguments = [item['temporary_path'], str(args.batch_ms)]
                rows = [json.loads(line) for line in run(str(executables[label]), *arguments).splitlines()]
                assert len(rows) == 1
                assert rows[0]['case'] == (case if corpus is None else item['temporary_path'])
                rows[0]['case'] = case
                records.append(dict(version=label, cases=rows))
    finally:
        run('git', 'worktree', 'remove', '--force', str(checkout))

summary = []
cases = {item['case']: item for record in records for item in record['cases']}
for case in cases.values():
    row = dict(case=case['case'], operations={})
    for mode, name in enumerate(metadata['operation_order']):
        values = {}
        for label in ['baseline', 'current']:
            samples = [sample for record in records if record['version'] == label
                       for item in record['cases'] if item['case'] == case['case']
                       for sample in item['samples_us'][mode]]
            values[label] = dict(median_us=statistics.median(samples), min_us=min(samples), max_us=max(samples))
        values['current_over_baseline'] = values['current']['median_us'] / values['baseline']['median_us']
        row['operations'][name] = values
    summary.append(row)
report = dict(metadata=metadata, sha256=hashes['current'], summary=summary, runs=records)
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report))
