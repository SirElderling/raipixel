"""Matched native public-API indexed PNG encoder comparison.

Both raipixel and png 0.18.1 receive exact RGBA samples and construct a
lossless palette per iteration; neither receives a precompressed stream or a
precomputed index buffer. Compare output sizes and complete encoding time for
raipixel indexed, reference Fast indexed and reference default indexed. All
outputs must pass the reference decoder and raipixel decoder with exact RGBA
(including fully transparent RGB). The reference crate is a disposable
benchmark-only dependency, never inspected or added to the product.

Usage: python tools/png_indexed_reference.py INPUT.png [INPUT.png ...]
"""
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix="raipixel-indexed-public-parity-") as temp:
    folder=Path(temp)
    (folder/"src").mkdir()
    (folder/"Cargo.toml").write_text(
        '[package]\nname="png-comparison"\nversion="0.0.0"\nedition="2024"\npublish=false\n'
        '[dependencies]\npng="=0.18.1"\nraipixel={path='
        +json.dumps(str(ROOT))+'}\n',encoding="utf-8")
    shutil.copyfile(ROOT/"benchmarks/png/reference.lock",folder/"Cargo.lock")
    shutil.copyfile(ROOT/"tools/png_indexed_reference.rs",folder/"src/main.rs")
    subprocess.run(["cargo","build","--release","--locked",
                    "--manifest-path",str(folder/"Cargo.toml")],check=True)
    exe=folder/"target/release"/("png-comparison.exe" if sys.platform=="win32" else "png-comparison")
    lines=subprocess.check_output([str(exe),*sys.argv[1:]],text=True).splitlines()
    if len(lines)!=len(sys.argv[1:]):
        raise ValueError("missing indexed reference cases")
    cases=[]
    for path,line in zip(sys.argv[1:],lines):
        result=json.loads(line)
        result["input"]=path
        result["input_sha256"]=hashlib.sha256(Path(path).read_bytes()).hexdigest()
        if result["supported"]:
            medians=[sorted(x)[len(x)//2] for x in result["samples_us"]]
            result["median_us"]=medians
            result["bytes_over_raipixel"]=[x/result["bytes"][0] for x in result["bytes"]]
            result["time_over_raipixel"]=[x/medians[0] for x in medians]
        cases.append(result)
    print(json.dumps(dict(experiment="matched_indexed_png_public_reference_workflow",
                          reference="png 0.18.1 locked default features public API",
                          modes=["raipixel_indexed","png_fast_indexed","png_default_indexed"],
                          calibration_ms=25,rounds=9,
                          all_palette_outputs_exact_rgba_verified=True,
                          compiler=subprocess.check_output(["rustc","--version","--verbose"],text=True),
                          platform=platform.platform(),toolchain=os.environ.get("RUSTUP_TOOLCHAIN",""),
                          driver_sha256=hashlib.sha256((ROOT/"tools/png_indexed_reference.rs").read_bytes()).hexdigest(),
                          cases=cases)))
