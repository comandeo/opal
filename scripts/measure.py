#!/usr/bin/env python3
"""Startup time (median of 10, to the first frame) and resident memory of opal.

Usage: scripts/measure.py target/release/opal
Linux-only (reads /proc); a development tool, not part of the editor.
"""
import os
import statistics
import subprocess
import sys
import tempfile
import time

binary = sys.argv[1]
block = ("Это **жирный** текст и *курсив*, а ещё `код` и [ссылка](http://x). "
         "Обычные слова, много слов.\n\n> цитата\n> ещё\n\n- пункт\n- [ ] задача\n\n")
with tempfile.NamedTemporaryFile("w", suffix=".md", delete=False) as f:
    text = "# Заголовок\n\n"
    while len(text.encode()) < 100_000:
        text += block
    f.write(text)
    doc = f.name

runs = []
for _ in range(10):
    started = time.perf_counter()
    subprocess.run([binary, doc], env=dict(os.environ, OPAL_BENCH="startup"), check=True)
    runs.append((time.perf_counter() - started) * 1000)
print(f"startup: median {statistics.median(runs):.0f} ms, min {min(runs):.0f} ms (100 KB document)")

proc = subprocess.Popen([binary, doc])
time.sleep(1.5)
with open(f"/proc/{proc.pid}/status") as status:
    rss = next(line for line in status if line.startswith("VmRSS"))
proc.terminate()
proc.wait()
print(f"memory: {rss.split(':')[1].strip()}")
os.unlink(doc)
