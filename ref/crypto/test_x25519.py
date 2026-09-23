"""RFC 7748 §5.2 (tek adım) + §6.1 (Diffie-Hellman) + iterasyon vektörleri."""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from x25519 import A24, BASE_U, is_all_zero, public_key, x25519  # noqa: E402

fails = []


def ck(name, got, want):
    ok = got == want
    if not ok:
        fails.append(f"{name}: beklenen {want}, gelen {got}")
    print(f"  [{'OK ' if ok else 'FAIL'}] {name}")


h = bytes.fromhex

print("RFC 7748 §5 — ladder sabiti")
# Kod incelemesinde "A24 486662 olmalı" itirazı geldi. A (486662) eğri parametresi,
# a24 ise ladder sabiti: a24 = (A-2)/4 = 121665. Bu satır ikisinin karışmasını kilitler.
ck("a24 == (A-2)/4 == 121665", (A24, A24 == (486662 - 2) // 4), (121665, True))

print("\nRFC 7748 §5.2 — tek adım X25519")
ck("vektör 1",
   x25519(h("a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4"),
          h("e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c")).hex(),
   "c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552")
ck("vektör 2",
   x25519(h("4b66e9d4d1b4673c5ad22691957d6af5c11b6421e0ea01d42ca4169e7918ba0d"),
          h("e5210f12786811d3f4b7959d0538ae2c31dbe7106fc03c3efc4cd549c715a493")).hex(),
   "95cbde9476e8907d7aade45cb4b873f88b595a68799fa152e6f8f7647aac7957")

print("\nRFC 7748 §5.2 — u'nun yüksek biti MASKELENİR (aynı sonucu vermeli)")
k2 = h("4b66e9d4d1b4673c5ad22691957d6af5c11b6421e0ea01d42ca4169e7918ba0d")
u2 = bytearray(h("e5210f12786811d3f4b7959d0538ae2c31dbe7106fc03c3efc4cd549c715a493"))
u2m = bytearray(u2); u2m[31] |= 0x80
ck("yüksek bit set ⇒ aynı çıktı", x25519(k2, bytes(u2m)), x25519(k2, bytes(u2)))

print("\nRFC 7748 §5.2 — iterasyon vektörleri")
k = u = bytes(BASE_U)
for i in range(1, 1001):
    k, u = x25519(k, u), k
    if i == 1:
        ck("iter 1", k.hex(), "422c8e7a6227d7bca1350b3e2bb7279f7897b87bb6854b783c60e80311ae3079")
    elif i == 1000:
        ck("iter 1000", k.hex(), "684cf59ba83309552800ef566f2f4d3c1c3887c49360e3875f2eb94d99532c51")

print("\nRFC 7748 §6.1 — Diffie-Hellman")
a = h("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a")
b = h("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb")
ck("Alice pub", public_key(a).hex(), "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a")
ck("Bob pub", public_key(b).hex(), "de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f")
ss = "4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742"
ck("K(a, B)", x25519(a, public_key(b)).hex(), ss)
ck("K(b, A)", x25519(b, public_key(a)).hex(), ss)

print("\nDüşük-mertebeli nokta reddi (RFC 7748 §6.1)")
ck("u=0 ⇒ tüm-sıfır paylaşılan sır", is_all_zero(x25519(a, bytes(32))), True)
ck("u=1 ⇒ tüm-sıfır paylaşılan sır", is_all_zero(x25519(a, (1).to_bytes(32, "little"))), True)
ck("geçerli anahtar ⇒ tüm-sıfır DEĞİL", is_all_zero(x25519(a, public_key(b))), False)

print()
if fails:
    print("SONUÇ: BAŞARISIZ\n" + "\n".join("  - " + f for f in fails)); sys.exit(1)
print("SONUÇ: TÜM KONTROLLER GEÇTİ")
