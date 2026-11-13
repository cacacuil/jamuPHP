"""Uji berhenti dengan rapi di Windows: Ctrl-Break ke jamu saat ada permintaan yang berjalan.

Permintaan itu harus selesai dengan 200, jamu keluar dengan kode 0, dan tidak ada
caddy.exe atau php-cgi.exe yang tertinggal.

Pemakaian: python tests/windows_stop.py dist/jamu.exe
"""

import pathlib
import signal
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request

# Konsol Windows bawaannya cp1252; log bisa memuat karakter di luar itu.
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
PORT = 8112
jamu = sys.argv[1]
site = pathlib.Path(tempfile.mkdtemp()) / "situs"
site.mkdir()
(site / "index.php").write_text("<?php sleep(3); echo 'selesai';")


def count(image):
    out = subprocess.run(["tasklist", "/FI", f"IMAGENAME eq {image}", "/NH"], capture_output=True, text=True).stdout
    return out.lower().count(image)


before = count("caddy.exe"), count("php-cgi.exe")
proc = subprocess.Popen(
    [jamu, "serve", str(site), "--port", str(PORT)],
    stderr=subprocess.PIPE,
    text=True,
    encoding="utf-8",
    errors="replace",
    creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
)
lines = []
threading.Thread(target=lambda: lines.extend(iter(proc.stderr.readline, "")), daemon=True).start()
deadline = time.time() + 40
while not any("jamu: siap" in line for line in lines):
    if time.time() > deadline or proc.poll() is not None:
        sys.exit("GAGAL  jamu tidak siap:\n" + "".join(lines))
    time.sleep(0.1)

reply = {}


def slow():
    try:
        with urllib.request.urlopen(f"http://127.0.0.1:{PORT}/", timeout=30) as r:
            reply["got"] = f"{r.status} {r.read().decode()}"
    except Exception as e:  # noqa: BLE001
        reply["got"] = repr(e)


request = threading.Thread(target=slow)
request.start()
time.sleep(1)
start = time.time()
proc.send_signal(signal.CTRL_BREAK_EVENT)
try:
    code = proc.wait(timeout=20)
except subprocess.TimeoutExpired:
    proc.kill()
    code = "tidak keluar dalam 20 detik"
took = time.time() - start
request.join()
time.sleep(1)
after = count("caddy.exe"), count("php-cgi.exe")

checks = {
    "permintaan yang sedang berjalan selesai": (reply.get("got"), "200 selesai"),
    "jamu keluar dengan kode 0": (code, 0),
    "tidak ada caddy.exe atau php-cgi.exe tertinggal": (after, before),
}
failed = 0
for name, (got, want) in checks.items():
    ok = got == want
    failed += not ok
    print(f"{'ok    ' if ok else 'GAGAL '} {name}" + ("" if ok else f": dapat {got!r}, harap {want!r}"))
print(f"info   berhenti dalam {took:.1f} detik")
print("--- log jamu ---\n" + "".join(lines[-12:]))
sys.exit(1 if failed else 0)
