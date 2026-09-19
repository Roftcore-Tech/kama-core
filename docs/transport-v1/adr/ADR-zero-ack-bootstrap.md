# ADR — Sıfır-geri-besleme başlangıcı: v1'de BELGELENMİŞ SINIR

**Status:** ACCEPTED (Faz-6, 2026-09-11) — DOCUMENTED LIMIT / FUTURE EVOLUTION CANDIDATE.

## Context

Faz-5 T1b (çift-yön %80 rastgele kayıp, N=30): 18/30 Kama koşusunda dönüş-yolu tanısı
`n=0/0 conf=0‰ Unknown` — **hiç ACK dönmedi**. Zincir: ACK yok → RTT örneği yok → yeniden-iletim
zamanlayıcısı kurulamıyor → `retx=0`, `tail_probe=0` → akış ilk pencerede (≈1 698 B / 25 600 B,
`initial_window_bytes = 2 400 B` ile tutarlı) 20 s boyunca duruyor. ACK alabilen 8/30 koşu tam yükü
gönderdi (0.900'e kadar). Aynı hücrede quinn 17/30, TCP 13/30 `connection-failure`.

Bu rejim için tasarlanmış mekanizmalar mevcut ama ürün profilinde kapalı:
`bootstrap_rtt = false`, `bootstrap_fast_ladder = false` — "kalibre-değil"
(`crates/client-core/src/runtime.rs`, KamaV1Profile belgesi; DR-CS-3 §9 kilitli panel).

## Decision

**v1 bunu garanti ETMEZ ve Faz-6'da hiçbir bootstrap mekanizması açılmaz.** Normatif ifade
(HOSTILE-ENVELOPE / KNOWN-LIMITATIONS LIM-001):

> Kama Transport v1 bozuk ve ağır şekilde zayıflamış geri-besleme yollarını destekler; ancak ilk
> kuruluş sırasında **hiç kullanılabilir geri-besleme elde edilemezse** ileri ilerlemeyi garanti
> etmez.

İki durum AÇIKÇA ayrılır:

| durum | v1 | kanıt |
|---|---|---|
| **BOZUK DÖNÜŞ YOLU** (ACK'lerin çoğu kayıp, bazıları dönüyor) | DESTEKLENİR, güçlü doğrulanmış | T6: dönüş %80 kayıp, Kama 1.000 CI[1.000,1.000] |
| **SIFIR-GERİ-BESLEME KURULUŞU** (ilk uçuşta hiç ACK yok) | GARANTİ EDİLMEZ | T1b: 18/30 kalıcı durma |

## Neden v1'de değişmiyor

* Mekanizma **yanlışlıkla** değil, kalibre edilmediği için kapalı — kayıtlı bir kapanış kararı.
* Faz-5 sonucuna bakarak bir ürün default'unu açmak, kampanyanın yasakladığı ayarlamadır.
* Doğrulanmamış bootstrap mantığını açmak, 990 koşuluk doğrulamayı geçersiz kılar: profil
  değişir → `kama-v1-full` kimliği artık ölçülen sistemi tanımlamaz (INV-008).
* Mevcut hiçbir ürün gereksinimi sıfır-geri-besleme kuruluş ilerlemesini VAAT ETMİYOR
  (§ HOSTILE-ENVELOPE "OUTSIDE"); dolayısıyla v1 blocker'ı değildir.

## Alternatives considered

* `bootstrap_rtt` açmak → kalibre değil, T1/T6 eşdeğerlerinde yeniden doğrulama gerektirir; reddedildi.
* İlk pencereyi büyütmek → yalnız `p_loss^N` olasılığını kaydırır, durmayı çözmez; reddedildi.
* ACK'siz kör yeniden iletim → yeni mekanizma, Faz-6 yasağı; reddedildi.

## Consequences

Zarf dışı bir rejim vardır ve belgelidir. Rakipler de aynı hücrede kuruluş kaybeder (davranış türü
kıyaslanabilir); fark, Kama'nın "zorunlu el sıkışma yok" iddiasından beklenen zarif bozulmanın
burada gerçekleşmemesidir — bu iddia zarfın içine alınmaz.

## Evidence

`reports/transport-v1/phase5/RESULTS-RUNNING.md` §T1b BULGU T1b-1;
`reports/transport-v1/PHASE5-T0-T7-REPORT.md` §22 madde 1.

## Revisit conditions (v1.1 / v2)

Saha/ürün kanıtı "ilk uçuşta sıfır ACK" senaryosunun operasyonel hedef olduğunu gösterirse.
Asgari yeniden doğrulama: temiz yol no-harm · rastgele-kayıp kuruluş · bozuk-dönüş (T1/T6
eşdeğerleri) · N≥30 + CI · yeni profil kimliği. Bkz. EVOLUTION §9.1.
