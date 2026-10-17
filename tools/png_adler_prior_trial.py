"""Screen a smaller prior-sum reduction in OUR two-stream Adler implementation.
No production edits. Reuses the stream probe's scalar checks and timing driver.
Run from a revision containing the two-stream production candidate.
"""
import hashlib
import json
import platform
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
source = (ROOT / 'src/checksum.rs').read_text(encoding='utf-8')
assert 'Two interleaved streams each accumulate eight eight-byte words.' in source
old = '''                    let pairs =
                        (prior & 0x0000_ffff_0000_ffff) + ((prior >> 16) & 0x0000_ffff_0000_ffff);
                    let prior_total = (pairs & 0xffff_ffff) + (pairs >> 32);'''
assert source.count(old) == 1
candidate = source.replace(old, '''                    // Eight words: all eight prior byte fields total <= 57120.
                    let prior_total = prior.wrapping_mul(0x0001_0001_0001_0001) >> 48;''')
driver = (ROOT / 'tools/png_adler_streams_trial.rs').read_text(encoding='utf-8')
driver = driver.replace('#[path = "../src/checksum.rs"]', '#[path = "baseline.rs"]')
driver = driver.replace('mod current;', 'mod current;\n#[path = "candidate.rs"]\nmod candidate;')
old_modes = '[|d|{let mut a=current::Adler::new();a.update(d);a.value()},packed::<1>,packed::<2>,packed::<4>]'
assert driver.count(old_modes) == 1
baseline_call = '|d|{let mut a=current::Adler::new();a.update(d);a.value()}'
candidate_call = '|d|{let mut a=candidate::Adler::new();a.update(d);a.value()}'
driver = driver.replace(old_modes, '[' + ','.join([baseline_call, candidate_call] * 2) + ']')
with tempfile.TemporaryDirectory(prefix='raipixel-adler-prior-') as temporary:
    work = Path(temporary)
    sources = {'baseline.rs': source, 'candidate.rs': candidate, 'driver.rs': driver}
    for name, text in sources.items():
        (work / name).write_text(text, encoding='utf-8')
    exe = work / 'probe.exe'
    subprocess.run(['rustc', '--edition=2024', '-O', str(work / 'driver.rs'), '-o', str(exe)], check=True)
    measurements = json.loads(subprocess.check_output([str(exe)], text=True))
    print(json.dumps(dict(experiment='two_stream_adler_prior_reduction',
        operations=['current', 'narrow_prior', 'current_duplicate', 'narrow_prior_duplicate'],
        compiler=subprocess.check_output(['rustc', '--version', '--verbose'], text=True),
        platform=platform.platform(), compiler_args=['--edition=2024', '-O'],
        source_sha256={name: hashlib.sha256(text.encode()).hexdigest() for name, text in sources.items()},
        script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        derivation='RFC 1950: each stream has 8 words; sum of all prior byte fields <= 8*28*255=57120; no 16-bit prefix carry',
        **measurements), indent=2))
