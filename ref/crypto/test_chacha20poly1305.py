"""RFC 8439 test vektörleri: §2.3.2, §2.4.2, §2.5.2, §2.6.2, §2.8.2, A.5."""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import chacha20poly1305 as c  # noqa: E402

fails = []


def ck(name, got, want):
    ok = got == want
    if not ok:
        fails.append(f"{name}:\n    beklenen {want}\n    gelen    {want if False else got}")
    print(f"  [{'OK ' if ok else 'FAIL'}] {name}")


h = bytes.fromhex
KEY = h("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")

print("RFC 8439 §2.3.2 — ChaCha20 blok fonksiyonu")
ck("blok (counter=1, nonce 000000090000004a00000000)",
   c.chacha20_block(KEY, 1, h("000000090000004a00000000")).hex(),
   "10f1e7e4d13b5915500fdd1fa32071c4c7d1f4c733c068030422aa9ac3d46c4e"
   "d2826446079faa0914c2d705d98b02a2b5129cd1de164eb9cbd083e8a2503c4e")

print("\nRFC 8439 §2.4.2 — ChaCha20 şifreleme")
pt = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it."
ck("ciphertext",
   c.chacha20(KEY, 1, h("000000000000004a00000000"), pt).hex(),
   "6e2e359a2568f98041ba0728dd0d6981e97e7aec1d4360c20a27afccfd9fae0b"
   "f91b65c5524733ab8f593dabcd62b3571639d624e65152ab8f530c359f0861d8"
   "07ca0dbf500d6a6156a38e088a22b65e52bc514d16ccf806818ce91ab7793736"
   "5af90bbf74a35be6b40b8eedf2785e42874d")

print("\nRFC 8439 §2.5.2 — Poly1305")
ck("etiket",
   c.poly1305_mac(b"Cryptographic Forum Research Group",
                  h("85d6be7857556d337f4452fe42d506a80103808afb0db2fd4abff6af4149f51b")).hex(),
   "a8061dc1305136c6c22b8baf0c0127a9")

# NOT (transkripsiyon hatam): bu vektörü ilk yazışımda hem anahtarı (§2.6.2 kendi anahtarını
# kullanır, §2.3.2'ninkini değil) hem beklenen çıktının ikinci yarısını yanlış yazdım.
# Beklenen değeri koda göre DEĞİŞTİRMEK tek başına kanıt olmazdı; kanıt bağımsız:
# aşağıdaki §2.8.2 AEAD vektörü poly1305_key_gen'i İÇİNDEN çağırır ve tag'i birebir tutuyor.
# poly1305_key_gen yanlış olsaydı §2.8.2 tag'i tutamazdı (o vektöre dokunulmadı).
print("\nRFC 8439 §2.6.2 — Poly1305 anahtar üretimi")
ck("one-time key",
   c.poly1305_key_gen(h("808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f"),
                      h("000000000001020304050607")).hex(),
   "8ad5a08b905f81cc815040274ab29471a833b637e3fd0da508dbb8e2fdd1a646")

print("\nRFC 8439 §2.8.2 — AEAD seal")
aad = h("50515253c0c1c2c3c4c5c6c7")
k = h("808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f")
n = h("070000004041424344454647")
sealed = c.seal(k, n, pt, aad)
ck("ciphertext",
   sealed[:-16].hex(),
   "d31a8d34648e60db7b86afbc53ef7ec2a4aded51296e08fea9e2b5a736ee62d6"
   "3dbea45e8ca9671282fafb69da92728b1a71de0a9e060b2905d6a5b67ecd3b36"
   "92ddbd7f2d778b8c9803aee328091b58fab324e4fad675945585808b4831d7bc"
   "3ff4def08e4b7a9de576d26586cec64b6116")
ck("tag", sealed[-16:].hex(), "1ae10b594f09e26a7e902ecbd0600691")
ck("roundtrip", c.open_(k, n, sealed, aad), pt)

print("\nRFC 8439 A.5 — AEAD open (decrypt yönü)")
k5 = h("1c9240a5eb55d38af333888604f6b5f0473917c1402b80099dca5cbc207075c0")
n5 = h("000000000102030405060708")
a5 = h("f33388860000000000004e91")
ct5 = h("64a0861575861af460f062c79be643bd5e805cfd345cf389f108670ac76c8cb2"
        "4c6cfc18755d43eea09ee94e382d26b0bdb7b73c321b0100d4f03b7f355894cf"
        "332f830e710b97ce98c8a84abd0b948114ad176e008d33bd60f982b1ff37c855"
        "9797a06ef4f0ef61c186324e2b3506383606907b6a7c02b0f9f6157b53c867e4"
        "b9166c767b804d46a59b5216cde7a4e99040c5a40433225ee282a1b0a06c523e"
        "af4534d7f83fa1155b0047718cbc546a0d072b04b3564eea1b422273f548271a"
        "0bb2316053fa76991955ebd63159434ecebb4e466dae5a1073a6727627097a10"
        "49e617d91d361094fa68f0ff77987130305beaba2eda04df997b714d6c6f2c29"
        "a6ad5cb4022b02709b")
tag5 = h("eead9d67890cbb22392336fea1851f38")
out = c.open_(k5, n5, ct5 + tag5, a5)
ck("açılan plaintext başlangıcı", out is not None and out.startswith(b"Internet-Drafts are draft documents"), True)

print("\nAuthenticate-before-use: bozuk girdi ⇒ None (plaintext SIZMAZ)")
for mut in ("tag", "ct", "aad", "nonce"):
    bad_sealed, bad_aad, bad_n = bytearray(sealed), aad, n
    if mut == "tag":   bad_sealed[-1] ^= 1
    elif mut == "ct":  bad_sealed[0] ^= 1
    elif mut == "aad": bad_aad = bytes([aad[0] ^ 1]) + aad[1:]
    else:              bad_n = bytes([n[0] ^ 1]) + n[1:]
    ck(f"{mut} bozuldu ⇒ None", c.open_(k, bad_n, bytes(bad_sealed), bad_aad), None)

print("\nnonce96 mapping (noise-wire §4.5 / ccr §35.6)")
ck("nonce96(0)", c.nonce96(0).hex(), "000000000000000000000000")
ck("nonce96(1)", c.nonce96(1).hex(), "000000000100000000000000")
ck("nonce96(2^64-1)", c.nonce96(2**64 - 1).hex(), "00000000ffffffffffffffff")
ck("nonce96 12B", len(c.nonce96(12345)), 12)

print()
if fails:
    print("SONUÇ: BAŞARISIZ\n" + "\n".join("  - " + f for f in fails)); sys.exit(1)
print("SONUÇ: TÜM KONTROLLER GEÇTİ")
