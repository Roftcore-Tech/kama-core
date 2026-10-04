"""RFC 8032 §7.1 (Ed25519 TEST 1/2/3/1024/SHA(abc)) + katılık negatifleri."""
import hashlib
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ed25519 as e  # noqa: E402

fails = []


def ck(name, got, want):
    ok = got == want
    if not ok:
        fails.append(f"{name}: beklenen {want}, gelen {got}")
    print(f"  [{'OK ' if ok else 'FAIL'}] {name}")


h = bytes.fromhex

# (ad, sk, pk, msg, sig) — RFC 8032 §7.1
VEC = [
 ("TEST 1 (boş mesaj)",
  "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
  "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a", "",
  "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e3970"
  "1cf9b46bd25bf5f0595bbe24655141438e7a100b"),
 ("TEST 2 (1 byte)",
  "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
  "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c", "72",
  "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613"
  "d0f11d8c387b2eaeb4302aeeb00d291612bb0c00"),
 ("TEST 3 (2 byte)",
  "c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7",
  "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025", "af82",
  "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac18ff9b538d16f290ae67f760"
  "984dc6594a7c15e9716ed28dc027beceea1ec40a"),
 ("TEST SHA(abc)",
  "833fe62409237b9d62ec77587520911e9a759cec1d19755b7da901b96dca3d42",
  "ec172b93ad5e563bf4932c70e1245034c35467ef2efd4d64ebf819683467e2bf",
  hashlib.sha512(b"abc").hexdigest(),
  "dc2a4459e7369633a52b1bf277839a00201009a3efbf3ecb69bea2186c26b58909351fc9ac90b3ecfdfbc7c6"
  "6431e0303dca179c138ac17ad9bef1177331a704"),
]

print("RFC 8032 §7.1 — Ed25519 test vektörleri")
for name, sk, pk, msg, sig in VEC:
    ck(f"{name} · pubkey", e.public_key(h(sk)).hex(), pk)
    ck(f"{name} · sign", e.sign(h(sk), h(msg)).hex(), sig)
    ck(f"{name} · verify", e.verify(h(pk), h(msg), h(sig)), True)

print("\nRFC 8032 §7.1 — TEST 1024 (1023-byte mesaj)")
sk = h("f5e5767cf153319517630f226876b86c8160cc583bc013744c6bf255f5cc0ee5")
pk = h("278117fc144c72340f67d0f2316e8386ceffbf2b2428c9c51fef7c597f1d426e")
msg = h("08b8b2b733424243760fe426a4b54908632110a66c2f6591eabd3345e3e4eb98fa6e264bf09efe12ee50f8f5"
        "4e9f77b1e355f6c50544e23fb1433ddf73be84d879de7c0046dc4996d9e773f4bc9efe5738829adb26c81b37"
        "c93a1b270b20329d658675fc6ea534e0810a4432826bf58c941efb65d57a338bbd2e26640f89ffbc1a858efc"
        "b8550ee3a5e1998bd177e93a7363c344fe6b199ee5d02e82d522c4feba15452f80288a821a579116ec6dad2b"
        "3b310da903401aa62100ab5d1a36553e06203b33890cc9b832f79ef80560ccb9a39ce767967ed628c6ad573c"
        "b116dbefefd75499da96bd68a8a97b928a8bbc103b6621fcde2beca1231d206be6cd9ec7aff6f6c94fcd7204"
        "ed3455c68c83f4a41da4af2b74ef5c53f1d8ac70bdcb7ed185ce81bd84359d44254d95629e9855a94a7c1958"
        "d1f8ada5d0532ed8a5aa3fb2d17ba70eb6248e594e1a2297acbbb39d502f1a8c6eb6f1ce22b3de1a1f40cc24"
        "554119a831a9aad6079cad88425de6bde1a9187ebb6092cf67bf2b13fd65f27088d78b7e883c8759d2c4f5c6"
        "5adb7553878ad575f9fad878e80a0c9ba63bcbcc2732e69485bbc9c90bfbd62481d9089beccf80cfe2df16a2"
        "cf65bd92dd597b0707e0917af48bbb75fed413d238f5555a7a569d80c3414a8d0859dc65a46128bab27af87a"
        "71314f318c782b23ebfe808b82b0ce26401d2e22f04d83d1255dc51addd3b75a2b1ae0784504df543af8969b"
        "e3ea7082ff7fc9888c144da2af58429ec96031dbcad3dad9af0dcbaaaf268cb8fcffead94f3c7ca495e056a9"
        "b47acdb751fb73e666c6c655ade8297297d07ad1ba5e43f1bca32301651339e22904cc8c42f58c30c04aafdb"
        "038dda0847dd988dcda6f3bfd15c4b4c4525004aa06eeff8ca61783aacec57fb3d1f92b0fe2fd1a85f672451"
        "7b65e614ad6808d6f6ee34dff7310fdc82aebfd904b01e1dc54b2927094b2db68d6f903b68401adebf5a7e08"
        "d78ff4ef5d63653a65040cf9bfd4aca7984a74d37145986780fc0b16ac451649de6188a7dbdf191f64b5fc5e"
        "2ab47b57f7f7276cd419c17a3ca8e1b939ae49e488acba6b965610b5480109c8b17b80e1b7b750dfc7598d5d"
        "5011fd2dcc5600a32ef5b52a1ecc820e308aa342721aac0943bf6686b64b2579376504ccc493d97e6aed3fb0"
        "f9cd71a43dd497f01f17c0e2cb3797aa2a2f256656168e6c496afc5fb93246f6b1116398a346f1a641f3b041"
        "e989f7914f90cc2c7fff357876e506b50d334ba77c225bc307ba537152f3f1610e4eafe595f6d9d90d11faa9"
        "33a15ef1369546868a7f3a45a96768d40fd9d03412c091c6315cf4fde7cb68606937380db2eaaa707b4c4185"
        "c32eddcdd306705e4dc1ffc872eeee475a64dfac86aba41c0618983f8741c5ef68d3a101e8a3b8cac60c905c"
        "15fc910840b94c00a0b9d0")
sig = h("0aab4c900501b3e24d7cdf4663326a3a87df5e4843b2cbdb67cbf6e460fec350aa5371b1508f9f4528ecea23"
        "c436d94b5e8fcd4f681e30a6ac00a9704a188a03")
ck("pubkey", e.public_key(sk).hex(), pk.hex())
ck("sign", e.sign(sk, msg).hex(), sig.hex())
ck("verify", e.verify(pk, msg, sig), True)

print("\nTaban noktasının mertebesi (sign() içindeki 'r mod L'nin gerekçesi)")
# İnceleme itirazı: "r mod L kofaktör bileşenini siler". B'nin mertebesi TAM OLARAK L'dir;
# kofaktör bileşeni B'de yoktur, dolayısıyla [r]B == [r mod L]B. RFC 8032 §5.1.6 adım 3 de
# indirgemeye açıkça izin verir ("For efficiency, do this by first reducing r modulo L").
ck("[L]B == nötr (B'nin mertebesi tam olarak L)", e._eq(e._mul(e.B, e.L), (0, 1, 1, 0)), True)
ck("[L+1]B == B", e._eq(e._mul(e.B, e.L + 1), e.B), True)
ck("64 farklı 512-bit r için [r]B == [r mod L]B",
   all(e._compress(e._mul(e.B, v)) == e._compress(e._mul(e.B, v % e.L))
       for v in [int.from_bytes(hashlib.sha512(bytes([i])).digest(), "little") for i in range(64)]),
   True)

print("\nKanonik olmayan nokta kodlamaları — [P, 2^255) aralığının TAMAMI")
# İnceleme itirazı: "bit-255 maskelemesi y >= P kontrolünü atlatıyor". Aralıktaki 19 y'nin
# her ikisi sign bitiyle = 38 kodlamanın TAMAMI reddedilmeli. Tek örnek yeterli kanıt değil.
_enc = [(y | (sb << 255)).to_bytes(32, "little") for y in range(e.P, 1 << 255) for sb in (0, 1)]
ck("38 kodlamanın tamamı _decompress tarafından RED",
   sum(1 for b in _enc if e._decompress(b) is None), 38)
ck("38 kodlamanın tamamı verify() tarafından pubkey olarak RED",
   any(e.verify(b, b"m", bytes(64)) for b in _enc), False)

print("\nSmall-order noktalar (ccr §5.2) — burulma alt grubunun 8 elemanı")
# Sekiz nokta HESAPLANIR (ezberden yazılmaz): y adayları taranmaz, burulma üreteci bulunur.
# Mertebesi 8 olan bir T için [i]T, i = 0..7 tüm alt grubu verir.
def _find_order8():
    y = 2
    while True:
        x = e._recover_x(y % e.P, 0)
        if x is not None:
            pt = (x, y % e.P, 1, x * y % e.P)
            t = e._mul(pt, e.L)            # [L]P burulma bileşenidir
            if e._eq(e._mul(t, 8), e.NEUTRAL) and not e._eq(e._mul(t, 4), e.NEUTRAL):
                return t
        y += 1
_T = _find_order8()
_torsion = [e._compress(e._mul(_T, i)) for i in range(8)]
ck("8 farklı kodlama", len(set(_torsion)), 8)
ck("hepsi small-order", all(e.is_small_order(e._decompress(t)) for t in _torsion), True)
ck("taban noktası small-order DEĞİL", e.is_small_order(e.B), False)
ck("nötr eleman kodlaması 01 00..00", _torsion[0].hex(), "01" + "00" * 31)

print("\nKatılık negatifleri (RFC 8032 §5.1.7 / §8.4)")
sk0, pk0, m0 = h(VEC[1][1]), h(VEC[1][2]), h(VEC[1][3])
s0 = e.sign(sk0, m0)
ck("doğru imza geçerli", e.verify(pk0, m0, s0), True)
ck("mesaj değişti ⇒ RED", e.verify(pk0, m0 + b"x", s0), False)
ck("R bozuldu ⇒ RED", e.verify(pk0, m0, bytes([s0[0] ^ 1]) + s0[1:]), False)
ck("S bozuldu ⇒ RED", e.verify(pk0, m0, s0[:32] + bytes([s0[32] ^ 1]) + s0[33:]), False)
ck("yanlış pubkey ⇒ RED", e.verify(h(VEC[0][2]), m0, s0), False)
# S + L ⇒ aynı denklemi sağlar ama S kanonik DEĞİL ⇒ malleability reddi
s_mal = s0[:32] + ((int.from_bytes(s0[32:], "little") + e.L) % (1 << 256)).to_bytes(32, "little")
ck("S+L (malleable) ⇒ RED", e.verify(pk0, m0, s_mal), False)
ck("kanonik olmayan y (y=p) ⇒ RED", e.verify((e.P).to_bytes(32, "little"), m0, s0), False)
ck("64B olmayan imza ⇒ RED", e.verify(pk0, m0, s0[:63]), False)

print()
if fails:
    print("SONUÇ: BAŞARISIZ\n" + "\n".join("  - " + f for f in fails)); sys.exit(1)
print("SONUÇ: TÜM KONTROLLER GEÇTİ")

print("\nccr §5.2 — dört ret nedeni AYRI raporlanır (verify_reason)")
_sk, _pk, _m = h(VEC[1][1]), h(VEC[1][2]), h(VEC[1][3])
_sig = e.sign(_sk, _m)
ck("geçerli imza", e.verify_reason(_pk, _m, _sig), e.OK)
ck("S >= L", e.verify_reason(_pk, _m, _sig[:32] + ((int.from_bytes(_sig[32:], "little") + e.L)).to_bytes(32, "little")), e.R_S_NOT_CANONICAL)
ck("kanonik olmayan A (y = p)", e.verify_reason(e.P.to_bytes(32, "little"), _m, _sig), e.R_NON_CANONICAL_POINT)
# small-order A = nötr eleman: [k]A = O ⇒ R = [r]B, S = r HER mesaj için kofaktörsüz denklemi sağlar.
_r = 12345
_forged = e._compress(e._mul(e.B, _r)) + _r.to_bytes(32, "little")
ck("small-order A ile sahte imza katı kipte RED", e.verify_reason(_torsion[0], b"any message", _forged), e.R_SMALL_ORDER_A)
ck("…ama gevşek doğrulayıcı KABUL ederdi (vektör anlamlı)", e.verify_cofactored_no_small_order_check(_torsion[0], b"any message", _forged), True)
# small-order R: R = T (mertebe 8), S = k·a ⇒ kofaktörlü denklem sağlanır, kofaktörsüz sağlanmaz.
_a, _ = e._secret_expand(_sk)
_Rt = _torsion[1]
_k = int.from_bytes(hashlib.sha512(_Rt + _pk + _m).digest(), "little") % e.L
_sig_r = _Rt + ((_k * _a) % e.L).to_bytes(32, "little")
ck("small-order R katı kipte RED", e.verify_reason(_pk, _m, _sig_r), e.R_SMALL_ORDER_R)
ck("…ama gevşek doğrulayıcı KABUL ederdi", e.verify_cofactored_no_small_order_check(_pk, _m, _sig_r), True)

print()
if fails:
    print("SONUÇ: BAŞARISIZ\n" + "\n".join("  - " + f for f in fails)); sys.exit(1)
print("SONUÇ: TÜM KONTROLLER GEÇTİ (ccr §5.2 dahil)")
