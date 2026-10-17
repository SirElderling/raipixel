#!/usr/bin/env python3
"""Native ABBA comparison of old snapshot stamp and new overlap-safe copy."""
import argparse
import json
import os
from pathlib import Path
import shutil
import statistics
import subprocess
import tempfile

BASE = "9ee5a3b3977f4be50afb5fc2023a475318564a9f"
CASES = {"icon": 120, "sprite": 40, "sheet": 8,
         "large": 2, "clipped": 8, "upward": 8}


def call(argv, directory):
    return subprocess.run(argv, cwd=directory, check=True,
                          capture_output=True, text=True).stdout


def build(root):
    call(["cargo", "+1.99.1", "build", "--release", "--locked",
          "--offline", "--example", "bench_stamp_copy"], root)
    exe = "bench_stamp_copy.exe" if os.name == "nt" else "bench_stamp_copy"
    return root / "target" / "release" / "examples" / exe


def measure(exe, case, mode, iterations, dump=None):
    argv = [str(exe), case, mode, str(iterations)]
    if dump is not None:
        argv.append(str(dump))
    return dict(token.split("=", 1) for token in
                call(argv, exe.parent).split() if "=" in token)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", default="stamp-compare.json")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    records = []
    with tempfile.TemporaryDirectory(prefix="raipixel-copy-") as tmp:
        old = Path(tmp) / "baseline"
        call(["git", "worktree", "add", "--detach", str(old), BASE], root)
        try:
            shutil.copy2(root / "examples" / "bench_stamp_copy.rs",
                         old / "examples" / "bench_stamp_copy.rs")
            builds = {"snapshot": build(old), "row-copy": build(root)}
            for case, iterations in CASES.items():
                old_png = Path(tmp) / "a.png"
                new_png = Path(tmp) / "b.png"
                reference = measure(builds["snapshot"], case, "edit", 1, old_png)
                candidate = measure(builds["row-copy"], case, "edit", 1, new_png)
                assert old_png.read_bytes() == new_png.read_bytes(), case
                assert all(reference[key] == candidate[key] for key in
                           ("rgba_hash", "remaining", "discarded", "png_bytes")), case
                for mode in ("edit", "export"):
                    samples = {"snapshot": [], "row-copy": []}
                    repeats = max(1, iterations // (4 if mode == "export" else 1))
                    for n in range(5):
                        order = (("snapshot", "row-copy", "row-copy", "snapshot")
                                 if n % 2 == 0 else
                                 ("row-copy", "snapshot", "snapshot", "row-copy"))
                        for key in order:
                            result = measure(builds[key], case, mode, repeats)
                            assert result["rgba_hash"] == reference["rgba_hash"]
                            samples[key].append(int(result["ns"]))
                    before = statistics.median(samples["snapshot"])
                    after = statistics.median(samples["row-copy"])
                    entry = {"case": case, "mode": mode,
                             "snapshot_ns": before, "row_copy_ns": after,
                             "row_copy_over_snapshot": round(after / before, 4),
                             "png_bytes": int(reference["png_bytes"]),
                             "byte_identical": True, "work_identical": True,
                             "samples_ns": samples}
                    records.append(entry)
                    print(json.dumps({k: v for k, v in entry.items()
                                      if k != "samples_ns"}), flush=True)
        finally:
            call(["git", "worktree", "remove", "--force", str(old)], root)
    Path(args.output).write_text(json.dumps({
        "baseline": BASE, "toolchain": "1.99.1", "profile": "release",
        "rounds": 5, "memory": "estimated, not RSS", "records": records
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
