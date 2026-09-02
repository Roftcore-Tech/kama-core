# KAMA — ÜRÜN BİTİRME PLANI (anti-drift, sprint'lere bölünmüş)

**Tarih:** 2026-08-26
**Amaç:** Kama'yı **ürün** olarak en hızlı şekilde bitirmek — transport bir araç, ürün onun üstüne kurulur.
**Bu belge neden var:** Önceki turlar bağlamdan koptu (docs:code 1.3×, ölçüm-döngüleri, mekanizma-inşa
ederken entegrasyon ertelendi). Bu plan her sprint'i tek bir soru-setine ("anti-drift sözleşmesi")
tabi tutar; geçemeyen sprint başlamaz. Kaynak: 3-ajan full-scale analiz (crate-haritası + uykuda-denetim
+ değer-önerisi matrisi, 2026-08-26).

---

## 0. ANTI-DRIFT SÖZLEŞMESİ (her sprint başında sorulur — geçmeyen iş yapılmaz)

```
Q1  Bağlamdan uzaklaşıyor muyum? Bu iş "ürünü bitirmeye" en kısa yolda mı, yoksa
    enstrüman-mükemmelleştirme / meta-doküman / yeniden-ölçüm mü?
Q2  AMAÇ: ürünü EN HIZLI bitirmek. Bu sprint ürünü sevk-edilebilirliğe yaklaştırıyor mu,
    yoksa yan-kanıt/iddia-gücü mü? (yan-kanıt = BACKLOG, ürün-grade değil)
Q3  DEĞER-ÖNERİSİ TESTİ: kama-transport'un amacı quinn'in çalışamayacağı sanılan yerde
    çalışmak. Bu sağlanamıyorsa OVER-ENGINEERING yapılmıştır. Bu sprint bu tezi
    kanıtlıyor/kuruyor mu, yoksa tezi varsayıp üstüne mi inşa ediyor?
Q4  UYKUDA KOD: (a) neden bağlanmadı? (b) ne zaman bağlanmalı? (c) ürüne etkisi ne?
    Bu sprint uykuda bir mekanizmayı ürüne bağlıyorsa a/b/c cevaplı olmalı.
```

Bir sprint DEĞER üretmiyorsa (Q2/Q3) veya bağlamdan koparsa (Q1) → **yapılmaz veya BACKLOG.**

---

## 1. FULL-SCALE DURUM (kanıtlı, sıkıştırılmış)

**Proje = federe egemen-IP güvenli haberleşme (taktik dayanıklı comms).** Bitmiş ürün için gereken
uçtan-uca: **transport + gerçek-kripto/kimlik + hub/rendezvous + discovery/PKI + client.**

| katman | crate(ler) | durum | ürün-kritikliği |
|---|---|---|---|
| Transport motoru | adaptive-transport (17k), kama-fec-codec, kama-transport-schema | **REAL** ama ayırt-edici (FEC-fusion) **kablolu değil** | tez-taşıyıcı |
| Kayıt-koruma (AEAD wire) | kama-noise-wire (3.2k), kama-engine, kama-integration | **REAL** ama handshake **enjekte-anahtar** | gerçek-oturum-öncesi |
| Client kompozisyon | client-core (879, P1 runtime kabuğu) | **PARTIAL** — opak-birim runtime var, gerçek-kripto/lifecycle yok | ürün-yüzeyi |
| **Gerçek kripto/handshake** | **crypto** | **3-SATIR İSKELET** — X25519/Ed25519/Noise/DH **HİÇ YOK** | **BLOKÖR: gerçek oturum + §4.4-G2** |
| Wire envelope (CCR) | **ccr-codec** | **3-SATIR İSKELET** | SESSION_* zarfları (noise-wire ile örtüşme sprint-0'da netleşir) |
| Rendezvous/relay | **hub-server** | **3-SATIR İSKELET** | IP'siz iki-client buluşması |
| PKI/kimlik/enrollment | **ra-ca** | **3-SATIR İSKELET** | egemen-kimlik, cert-bound trust |
| Contact discovery | **discovery** | **3-SATIR İSKELET** | VOPRF ile kişi bulma |
| Admin kontrol | **admin-server** | **3-SATIR İSKELET** | operasyonel |

**Tek cümle:** transport olgun ~%40; güvenlik/kimlik omurgası + 5 alt-sistem iskelet; **değer-önerisi
KANITLANMADI** (tek savunulabilir hücre p2 %80-IID: Kama 0.505 vs quinn 0.040 = asimetrik-üstünlük ama
%50<%90 kategorik-bar, üstelik FEC-KAPALIYKEN; burst GERİDE; kullanıcının merkez-rejimi test-edilmedi;
G2 crypto-stub yüzünden iddia-edilemez → §4.4 kapısı kapalı).

---

## 2. OVER-ENGINEERING TESTİ + UYKUDA-KOD HÜKMÜ (Q3/Q4 cevabı)

**Neden %50 kablolu değil (tek cümle):** client-core var olana kadar adaptive-transport'un
ürün-tüketicisi YOKTU → FEC/fusion/bootstrap "inşa-gereği sevk-edilemez"di. Boşa değil, **entegre
edilmedi.** Ama entegre edilmezse Q3 gereği over-engineering'dir.

| uykuda kod | (a) neden bağlanmadı | (b) ne zaman | (c) ÜRÜNE ETKİSİ |
|---|---|---|---|
| **FEC-actuation + fusion_policy + fate_estimator** (yapı-farkında FEC gate) | tüketici yoktu; Q18 (rate-only gate %20'de parity=0) naif-wiring'i güvensiz yaptı | **S1 (şimdi)** | **YÜKSEK — tez-belirleyici.** Bağlanmazsa: burst'te kayıp, yüksek-kayıpta %50 tavan = değer-önerisi ÇÖKER. Q18-fix = kalibrasyon değil, fate_estimator'ı gate'e bağlamak |
| budget_kernel (FEC-bütçe) | experimental-gate; sadece FEC aktifken gerekli | S1 (FEC ile) | ORTA — FEC bant-disiplini |
| bootstrap-RTT ladder | panel-fail 2/3 (rep14 GE-L10) | S1-sonrası / tail-ON RTT-tahmini değeri artınca | DÜŞÜK-ORTA — cold-start PTO, tez-dışı |
| fs_harness/fs_trace/udp_runner (~3464 LOC) | test/bench tooling — ÜRÜN DEĞİL | asla (doğru şekilde ürün-dışı) | YOK (araç) |

**Sonuç:** gerçekten-uykuda ÜRÜN mühendisliği ~2049 LOC (fusion_policy+estimator+budget_kernel) +
sender-FEC-makinesi. Bunun ~%80'i **S1'de** bağlanıyor (değer-önerisi için). Kalanı düşük-etki/araç.

---

## 3. ÜRÜN-BİTİRME SPRINT SIRASI

> Sıralama ilkesi: **önce tezi de-risk et, sonra ürünü tezin üstüne kur.** Tez yanlışsa (S1 GATE
> başarısız) S2+'ya para harcamadan dururuz — bu, over-engineering'i erken yakalayan tek disiplin.

### S0 — Burst re-proof *(KOŞUYOR, ~3.5h)*
**İş:** V1 — ARQ-fix'in jammer-burst'te quinn'i yakalayıp yakalamadığını ölç (baseline).
**Q-kontrol:** Q3 ✓ (tezin burst-yarısını ölçüyor). **Çıkış:** NOT-WORSE / STILL-WORSE hükmü.
STILL-WORSE beklenir → S1'i zorunlu kılar. **Boyut:** bitmek üzere.

### S1 — TEZ DE-RISK: değer-önerisini kanıtla veya çürüt *(EN KRİTİK, ~2 sprint)* — **GATE**
**İş:** Uykuda FEC-fusion zincirini ürüne bağla (V2: yapı-farkında gate = Q18-fix; seam FEC-execute
byte-sadık; runtime FEC-pacing; ClientSession validated-fusion). SONRA hostile matrisi **FEC-AÇIK +
eksik-rejim** (burst + yüksek-bant/yüksek-kayıp/yüksek-gecikme + %80-95) ile ön-kayıtlı koş (~4h bounded).
**Q-kontrol:** Q3 ✓✓ (tezi doğrudan sınar), Q4 ✓ (uykuda-%50'yi bağlar — a/b/c yukarıda).
**GATE / çıkış-kriteri:**
- ✅ **TEZ GERÇEK** ise: FEC-gate'li Kama (a) burst'ü önde çevirir, (b) %80'i ≥%90'a yaklaştırır,
  (c) test-edilmemiş rejimde quinn≈0 iken çalışır → **değer-önerisi bankada → S2'ye geç.**
- ❌ **TEZ ÇÖKERSE** (FEC-açıkken de quinn'i geçemiyor): **DUR.** Bu, transport'un over-engineering
  olduğunu söyler; S2+ inşasına para harcamadan mimari yeniden-değerlendirme (Q3 kırmızı).
**Boyut:** V2 kodu ~1.5 sp (V1 biter bitmez cargo serbest) + V2.4 doğrulama ~0.5 sp.

### S2 — GERÇEK OTURUM: crypto handshake *(~2-2.5 sprint)* — S1-GATE=YEŞİL şartı
**İş:** crypto crate'i (X25519 + Ed25519, pki §13 registry) → Noise handshake (transport-tdd §0.1
IK/KK) → Split → yön-anahtarları → gerçek `SecuritySeam` (IdentitySeam stub'ı değiştirir, aynı trait).
Sonuç: iki BİLİNEN peer arası gerçek şifreli+kimlik-doğrulanmış oturum; §4.4-G2 açılır.
**Q-kontrol:** Q1 ✓ (ürün-kritik omurga, meta-iş değil), Q2 ✓ (sevk-edilebilirliğe en büyük adım).
**Çıkış:** iki-client gerçek-Noise handshake + FEC-transport uçtan-uca netem'de yeşil; cold-start
hücre-iii kapanır. **Boyut:** ~2-2.5 sp (workstream-B'nin çekirdeği; artık transport'u gate ETMEZ,
transport S1'de kanıtlanmış olur).

### S3 — BULUŞMA/RELAY: minimal hub rendezvous *(~2 sprint)*
**İş:** hub-server minimal rendezvous (redb arkası; hub-mesh DHT SONRA) — IP'siz iki-client'ın
buluşup relay olması. routing-rendezvous-architecture'ın MVP alt-kümesi.
**Q-kontrol:** Q2 ✓ (gerçek-dağıtım için gerekli). **Çıkış:** iki-client hub üzerinden buluşur+konuşur.
**Not:** demo doğrudan-bağlantıyla S3'ü ATLAYABİLİR; ama gerçek-ürün için gerekli. **Boyut:** ~2 sp.

### S4 — KİMLİK/TRUST: PKI + discovery *(~3 sprint)*
**İş:** ra-ca (enrollment CSR-PoP, revocation, HubEntryV3 cert-bound) + discovery (VOPRF).
egemen-kimlik + cert-bound trust + kişi-bulma. **Q-kontrol:** Q2 ✓ (federe-platform için).
**Çıkış:** cert-bound enrollment + VOPRF lookup çalışır. **Boyut:** ~3 sp (en büyük iskelet-blok).

### S5 — OPERASYONEL: admin + hardening + deploy *(~1.5 sprint)*
**İş:** admin-server (admin↔RA), musl static-binary + systemd (ADR-19), son güvenlik-denetimi.
**Çıkış:** sevk-edilebilir ürün. **Boyut:** ~1.5 sp.

---

## 4. KRİTİK KAPI ve ZAMAN

**S1 GATE tüm planın kaderini belirler.** Yeşilse ~9-10 sprint'lik net ürün-yolu (S2-S5) açılır.
Kırmızıysa transport tezi çürük demektir ve önce O çözülür (yeni transport-mimarisi ya da hedef-daraltma)
— S2+ inşası ancak tez gerçekse mantıklı. **Bu belge S1'i bir DENEY olarak konumlar, varsayım olarak
değil.** (Önceki hata: tezi varsayıp mekanizma üstüne mekanizma inşa etmek.)

**Kaba toplam (S1 yeşil senaryosu):** S1 ~2 sp + S2 ~2.5 + S3 ~2 + S4 ~3 + S5 ~1.5 ≈ **~11 sprint**
minimal-federe-ürüne. Demo (doğrudan-bağlantı, S3/S4 atlanmış): S1+S2 ≈ **~4.5 sprint**.

---

## 5. SÜREÇ KURALLARI (uzamayı kesmek — bağlayıcı)
1. Sprint başına TEK plan-belgesi (varsa) + TEK doğrulama-kapısı + anında commit. Addendum-zinciri YOK.
2. Sonuç-üreten araç sonuçla aynı commit'te. Yeni kampanya YALNIZ S1/S2.4 gibi tez-kritik noktada.
3. Result-bearing koşu = ön-kayıt + bounded (~4h, 90h-kampanya YOK). Koşarken cargo YASAK.
4. Her sprint sonunda anti-drift sözleşmesi (§0) yeniden sorulur; Q3 kırmızıysa DUR.
5. Bankadakini yeniden-ölçme; iskelet-crate'in spec'i o sprint'in İLK işi (just-in-time, ön-yükleme yok).
