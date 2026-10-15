"""Linux-only fresh-process RSS measurement; thin parent avoids generator RSS.
Usage: python tools/png_memory.py COMPARISON_EXECUTABLE INPUT.png
"""
import json
import os
import subprocess
import sys

if sys.platform != 'linux':
    raise SystemExit('wait4 peak RSS units require Linux')
executable, source = sys.argv[1:]
report = {'input': '4096x4096 repeated-icon sheet',
          'method': 'three fresh processes per decoder; thin parent; wait4 ru_maxrss in KiB'}
for mode in ['memory-ours', 'memory-crate']:
    values = []
    for _ in range(3):
        process = subprocess.Popen([executable, mode, source])
        _, status, usage = os.wait4(process.pid, 0)
        process.returncode = os.waitstatus_to_exitcode(status)
        if process.returncode:
            raise SystemExit(f'{mode} exited {process.returncode}')
        values.append(usage.ru_maxrss)
    report[mode] = {'peak_rss_kib': values, 'median_kib': sorted(values)[1]}
print(json.dumps(report))
