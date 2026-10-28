# DUR-05g-3 (A) — eş başına onaysız provisional tavanı — TASLAK

## 0. Künye

**TASLAK — kod yok.** Kaynak: CEVAP PAKETİ 16 §2, DUR-05g-3 (A). Ayrı kart; "INT-02 GEÇTİ"den ÖNCE kapanır. **Kart adı ve numarası sahip kararı.**
- Dal `k4g/05g-a-provisional-tavan`, taban main `a858f3fc`. `crates/kama-noise-wire` ve ilgili spec'ler bu tabanda `1b1425d2` ile aynı.
- Satır atıfları bu tabandandır.
- Tel değişmez. Hub etkilenmez: oturumlar uçlardadır, hub yalnız zarf taşır.
- Donmuş metne dokunan kısımlar §5'te DUR olarak yazıldı.

## 1. Sorun

Eşle etkin oturum varken gelen her yeni sayaçlı INIT, `cmd_accept_init` içinde yeni bir provisional `Meta` doğurur:
- etkin oturum araması `supersedes = cp.active_session_with(&peer)`, `session_control.rs:769`;
- ekleme `cp.sessions.insert`, `:801`.

`sessions` haritasının (`:205`) tavanı yok. Yalnız yanıt önbelleği eş başına `HALF_OPEN_CAP` = 64 ile budanıyor (`:80`, `:818-828`), o da oturum kaydını silmiyor.

Onaysız provisional üç yoldan biter:
- onay: `after_session_open`, `:418`;
- süre: `PROVISIONAL_TTL` = 7 gün (`:85`), sessiz düşüş `tick_meta` `:450-453` → `end_session(h, None)` (`:518`, `:543`);
- kapanış: `Close`.

Bu yüzden doğrulanmış bir eş — ya da yerel uç hatası altında tekrar tekrar bağlanan biz (K4G-05g) — karşı uçta 7 gün boyunca sınırsız sayıda provisional bırakabilir. DUR-05g-3 (B) yalnız bizim tarafımızın başarısız girişimlerini azaltır; bu sınırı koymaz.

## 2. Öneri (kama-noise-wire, `session_control.rs`)

- **Sabit.** Önerilen ad `PROVISIONAL_CAP`: eş kimliği başına onaysız provisional tavanı. Değer ⟨KARAR⟩.
  - Statüsü `HALF_OPEN_CAP` gibi "yerel politika, tel sabiti değil"; KS-DOS-1 ailesi.
  - Aday değer 1: dürüst bir eşin aynı anda tek canlı girişimi olur — `connect` idempotenttir, yenileme etkin oturum başına tektir. Yeni sayaçlı INIT öncekini zaten terk etmiştir.
  - Aday değer 2–4: geç gelen INIT'e tolerans payı.
- **Kabul kuralı.** `AcceptInit` provisional yoluna girerken eşin onaysız provisional sayısı tavana ulaşmışsa:
  - Gelen INIT'in `init_counter`'ı tutulanların hepsinden büyükse, en küçük sayaçlı provisional düşer. Bu `HALF_OPEN_CAP`'in "dolunca en eski (en küçük sayaç) düşer" kalıbıdır.
  - Aksi hâlde gelen INIT reddedilir: opak `Failed`, RESPONSE yok, kayıt yok. Hub kuyruğunda yeri değişip geç gelen INIT canlı girişimi böylece düşüremez.
  - Bunun için `Meta`'ya provisional'ın `init_counter`'ı eklenir (iç yapı; tel değil).
- **Temizleme.** Düşürme `end_session(h, None)` yolundan yapılır; `PROVISIONAL_TTL` düşüşüyle aynı: protector kaydı ve önbellekli yanıt silinir. `Active` ya da `Draining` oturum ASLA düşürülmez. Onay ve `PROVISIONAL_TTL` yolları değişmez.
- **Olay.** Bugün sessiz düşüş olay üretmiyor (KM-SEAM-TC-001 :137 "olay yok"). Ama adaptörün `provisional` haritası (`client-core/src/transport.rs:752` ekleme; `:857`, `:918` silme; main `a858f3fc` satırları) olay gelmeyince hiç temizlenmiyor; bu da aynı sınırsızlığın istemci tarafı. Yollar §5 DUR-A2'de.

## 3. Zorunlu testler (kama-noise-wire, sahte saatle)

1. **Tavan aşılmaz:** etkin oturum varken N > tavan başarısız tekrar INIT (onay gelmez). Eşin onaysız provisional sayısı hiçbir anda tavanı aşmaz; düşen kayıtların protector ve önbellek izi kalmaz.
2. **Eski etkin oturum korunur:** aynı akışta etkin oturum seal/open için tam çalışır, `Closed` üretmez. Düşürülen provisional olay üretmez (DUR-A2 (a) seçilirse yalnız provisional'ın kendi `Closed`'ı).
3. **Sonraki başarılı reconnect mümkün:** tavan doluyken gelen en yüksek sayaçlı INIT kabul edilir. Onay gelince `Confirmed` olur, eski oturum drain'e düşer.

Ek iki test:
- daha küçük sayaçlı geç INIT reddedilir ve canlı provisional'ı düşürmez;
- client-core ucunda adaptörün `provisional` haritası da sınırlı kalır (DUR-A2'ye bağlı).

## 4. Tel / hub / sınır

- Zarf biçimi, codec ve `tv_*` değişmez.
- S3 enum'ları değişmez (`SessionResult::Failed` ve gerekirse mevcut `SessionEvent::Closed` kullanılır).
- Hub kodu etkilenmez.
- Bu kart K4G-05g'nin Ready düzeltmesinden bağımsızdır; onunla birlikte merge şartı yoktur.

## 5. DUR'lar (sahip kararı)

- **DUR-A1 — donmuş spec metni.** Kural şu üç belgeye yazılmalı: ccr §35.15 KS-DOS-1 (yeni sabit), identity §16 madde 4 ve KM-SEAM-TC-001 Provisional durum paragrafı (yeni düşüş koşulu). Üçü de donmuş; biçim üstü çiz + tarihli YAMA KAYDI + `word_coverage`. Metin taslağı Ek A'da.
- **DUR-A2 — adaptör temizliği.**
  - (a) Düşüşte Crypto `Closed(p, KeyHygiene)` yayımlar (enum aynı); adaptör `on_crypto_closed` adım (3) haritayı temizler, S1'e olay gitmez. KM-SEAM-TC-001'deki "olay yok" cümlesi bu düşüş için değişir.
  - (b) client-core haritayı kendi tavanıyla sınırlar. Crypto'nun sırasını bilmez: yanlış kaydı düşürürse sonra gelen `Confirmed` yok sayılır ve onay kaybolur.
  - Öneri: (a). Aynı olay `PROVISIONAL_TTL` düşüşüne de verilirse bugünkü sınırsız harita da kapanır.
- **DUR-A3 — değer ve sıra politikası.**
  - Tavan değeri ⟨KARAR⟩.
  - Sayaç sırası ile varış sırası arasında seçim: sayaç sırası geç INIT'e dayanıklıdır, ama kalıcı durumunu yitirmiş eşte (D4: sayaç geri gider) yeni INIT'i `PROVISIONAL_TTL` boyunca reddeder. Varış sırasında bunun tersi geçerlidir.

## Ek A — taslak yama metinleri (DUR-A1; ⟨KARAR⟩ değerleri sahipten)

Biçim: üstü çiz, silme + tarihli YAMA KAYDI + `scripts/ci/word_coverage.py`. `<ONAY-TARİHİ>` sahip kararının tarihidir.

### A.1 — `specs/ccr-format-spec.md` §35.15 (`PROVISIONAL_TTL` satırının, `:3156-3159`, ardına)

```
> PROVISIONAL_CAP  [<ONAY-TARİHİ> — DUR-05g-3 (A)] İSTEMCİ tarafı — eş kimliği başına onaysız provisional
>           oturum tavanı = ⟨KARAR⟩. Dolunca: gelen INIT'in init_counter'ı tutulanlardan büyükse en küçük
>           sayaçlı provisional düşer (HALF_OPEN_CAP kalıbı); değilse gelen INIT opak Failed. Etkin ya da
>           drain'deki oturum düşürülmez. Yerel politika, tel sabiti DEĞİL (KS-DOS-1 statüsü).
```

### A.2 — `specs/identity-and-trust-model.md` §16 madde 4 (`:2204`)

Satırın sonuna eklenir: **[<ONAY-TARİHİ> — DUR-05g-3 (A)]** ya da eş başına `PROVISIONAL_CAP` dolunca en küçük `init_counter`'lı onaysız provisional düşer (ccr §35.15); eski oturuma yine DOKUNULMAZ.

### A.3 — `docs/seams/KM-SEAM-TC-001.md` Provisional durum (`:137`)

"Onaylanmayan provisional oturum `PROVISIONAL_TTL` ile düşer (ccr §35.15); olay yok, …" cümlesinin ardına eklenir: **[<ONAY-TARİHİ> — DUR-05g-3 (A)]** Eş başına `PROVISIONAL_CAP` (ccr §35.15) dolunca en küçük `init_counter`'lı onaysız provisional da düşer; tavan doluyken daha küçük sayaçlı INIT opak `Failed`. DUR-A2 (a) seçilirse: ~~olay yok~~ bu iki düşüşte Crypto `Closed(p, KeyHygiene)` yayımlar (Transport onu Application'a İLETMEZ; provisional dönem görünmez), `h`'ye dokunulmaz.
