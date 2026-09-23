#!/usr/bin/env python3
"""ref/crypto BAĞIMSIZ referansının tek kapısı — A2.1.

Koşum: python3 ref/crypto/selftest.py
Çıkış kodu 0 = tüm RFC vektörleri ve negatifler geçti. CI: core-ci / vectors job'ı.
"""
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SUITES = [
    ("HKDF-BLAKE2s (RFC 5869 + INV-KDF-3)", "test_kdf.py"),
    ("X25519 (RFC 7748)", "test_x25519.py"),
    ("Ed25519 (RFC 8032)", "test_ed25519.py"),
    ("ChaCha20-Poly1305 (RFC 8439)", "test_chacha20poly1305.py"),
]

failed = []
for title, script in SUITES:
    print(f"\n{'=' * 78}\n== {title}\n{'=' * 78}")
    r = subprocess.run([sys.executable, os.path.join(HERE, script)])
    if r.returncode != 0:
        failed.append(title)

print(f"\n{'=' * 78}")
if failed:
    print("ref/crypto: BAŞARISIZ — " + ", ".join(failed))
    sys.exit(1)
print(f"ref/crypto: {len(SUITES)}/{len(SUITES)} süit GEÇTİ")
