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
                process_order=['baseline', 'current', 'current', 'baseline'])
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
        hashes = {}
        for label, exe in executables.items():
            folder = temporary / ('outputs-' + label)
            run(str(exe), '--export', str(folder))
            hashes[label] = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(folder.iterdir())}
        assert hashes['baseline'] == hashes['current'], 'encoded PNG bytes changed'
        for label in metadata['process_order']:
            rows = [json.loads(line) for line in run(str(executables[label]), '--suite', str(args.batch_ms)).splitlines()]
            records.append(dict(version=label, cases=rows))
    finally:
        run('git', 'worktree', 'remove', '--force', str(checkout))

summary = []
for case in records[0]['cases']:
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
