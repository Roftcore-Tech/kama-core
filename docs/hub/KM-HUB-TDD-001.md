# KM-HUB-TDD-001 — `hub-server`: tasarım ve testler (v0.6.1)

Sahip: Abkarada · Uygulayan: Yaaz (Claude Code ile) · Sürüm: v0.6.1 (2026-10-15; Yaaz'ın soruları: adres ailesi, adres çakışması, yansıtma, sayaç adları) · Kod: `crates/hub-server` (workspace'te boş iskelet olarak var) · Sözleşme: **`docs/seams/KM-SEAM-TH-001.md` (S4 v1.3.1)** — baytlar, mesaj adları, hata kodları, sabitler ve hub kuralları **oradadır**; bu belge onları **kodlar ve sınar**, yeniden tanımlamaz. Çelişkide S4 kazanır.

**Önce oku:** S4 §1 (on cümle), §2 (tel; özellikle §2.3 mesaj tablosu ve §2.4(c) geçerli birleşimler), §3 (H1–H12), §6 (sabitler). Toplam 5 sayfa. Başka belge gerekmez.

---

## 1. Hub ne yapar (S4 §1'in kod gözüyle özeti)

Tek UDP soketi, tek iş parçacığı, bellekte birkaç tablo. Gelen her datagram için: ilk 4 bayt sıfırsa **kontrol** (bind / zarf rölesi / heartbeat), değilse **veri** (bir hattın paketi; yükü okumadan karşı uca at). Hub hiçbir zarfı açmaz, hiçbir mesajı saklamaz, hiçbir "koptu" kararı vermez; tabloları yalnız gelen mesajlar değiştirir.

## 2. Durum (hepsi bellekte; `hub.json` salt-okunur)

```
Directory   cert_ref → { dest_hash, device_ed25519_pub }                 # hub.json'dan, değişmez (H2)
Endpoints   dest_hash → { addr: Option<Addr>,  # None = adressiz (hiç bind olmadı ya da adresi başka kimlik aldı — H3); hatlar/anahtarlar kalır
                          cert_ref, last_activity,
                          bind_epoch: u32,                                               # H3: her kabulde +1; cookie girdisi
                          last_accepted: Option<{client_nonce, hub_nonce, addr, ack_bytes}>,  # H3: yalnız kayıp ACK tekrarı
                          link_count }                                                   # her hat iki uçta sayılır
            by_addr: addr → dest_hash                                     # H4 kaynak denetimi, heartbeat; BİREBİR: bir adres ↔ en çok bir uç
Links       handle → { a: dest_hash, b: dest_hash, last_activity, key: AllocationKey }  # key = hattı açan istek (S4 §2.4(c))
            by_key: AllocationKey → handle          # AKTİF hatların TAMAMI; hat yaşadıkça girdi silinmez (KR-19)
Tombstones  handle → { retired_at, key }            # kapasite MAX_TOMBSTONES; erken silme YOK
            tomb_by_key: AllocationKey → handle     # emekli-anahtar sorgusu; tombstone ile aynı kapasite ve 7 gün
            (indekslerin toplam boyutu = len(links) + len(tombstones); ayrı istek geçmişi YOK)
Rate        verify_budget (küresel) · bind_fail: Lru<addr → {fails, since}, BIND_FAIL_ENTRIES> (yalnız geçerli cookie sonrası yazılır)
            · store_rate[dest_hash] · alloc_rate[dest_hash] · route_miss_rate[dest_hash]
Secrets     cookie_secret (+ öncekisi), handle_secret, alloc_counter: u64, hub_ed25519_sk
```
Nonce listesi/halkası **yoktur**: eski kanıtlar `bind_epoch` ile düşer (S4 §2.4(b)).

## 3. İşleyiciler (sözde kod; her dal S4'teki bir kurala bağlı)

```
recv(d, src):
  if len(d) > MAX_DATAGRAM or len(d) < 5: count(drop_size); return
  if u32_be(d[0..4]) == 0: on_control(d, src) else on_data(u32_be(d[0..4]), d, src)

on_control(d, src):                                   # S4 §2.1–2.3
  if len(d) < 21 or d[len-16..] != [0;16]: count(drop_malformed); return
  frame_type = d[4]; body = d[5..len-16]
  0x1B → on_heartbeat(body, src)
  0x1D → f = parse AppRpcFrame(body) or {count(drop_malformed); return}; on_rpc(f, src)
  else → count(drop_unknown_frame)

on_heartbeat(body, src):                              # H8
  if len(body) ∉ {0,4}: count(drop_malformed); return
  me = by_addr.get(src) or {count(hb_unbound); return}          # slot yaratmaz, cevap yok
  endpoints[me].last_activity = now
  send(src, [0;4] ‖ 0x1C ‖ body ‖ [0;16])

on_rpc(f, src):
  if f.flags != 0 or (f.method_id & 0x80): count(drop_malformed); return   # tel→hub yalnız istek
  0x01 → on_bind1(f, src)  ·  0x0B → on_bind2(f, src)  ·  0x05 → on_store(f, src)
  else → count(drop_unknown_method)                   # yanıt yok

cookie(src, cert_ref, client_nonce, minute, secret, epoch):            # S4 §2.4(a)
  src = normalize(src)                                  # ::ffff:a.b.c.d → IPv4
  fam = 0x04 if ipv4 else 0x06; ip = 4 B | 16 B
  blake2s_256(secret ‖ fam ‖ ip ‖ port_be16 ‖ cert_ref ‖ client_nonce ‖ minute_be32 ‖ epoch_be32)[0..16]

on_bind1(f, src):                                     # H2, H3, S4 §2.4(a) — hiçbir şey YAZMAZ
  req = parse(f.payload) else return
  dev = Directory.get(req.cert_ref) or {count(bind_unknown); return}       # SESSİZ
  epoch = endpoints.get(dev.dest_hash)?.bind_epoch or 0                    # yalnız okur
  reply(f, src, BindResp{bind_ok=0, 0, 0, CHALLENGE, hub_nonce = cookie(src, req.cert_ref, req.client_nonce, minute_now, cur, epoch)})

on_bind2(f, src):                                     # S4 §2.4(b) işlem sırası 1–6 — SIRA ÖNEMLİ
  req = parse(f.payload) else return
  dev = Directory.get(req.cert_ref) or {count(bind_unknown); return}       # (1) sessiz
  ep = endpoints.get(dev.dest_hash)
  la = ep?.last_accepted
  if la and la.client_nonce == req.client_nonce and la.hub_nonce == req.hub_nonce   # (2) kayıp ACK tekrarı
         and la.addr == src and ep.addr == src:
      send(src, la.ack_bytes); count(bind_ack_repeat); return               #     bayt-özdeş yanıt, DURUM DEĞİŞMEZ
  epoch = ep?.bind_epoch or 0
  ok = any(req.hub_nonce == cookie(src, req.cert_ref, req.client_nonce, m, s, epoch)
           for m in {now, now-1} for s in {cur, prev})
  if not ok: reply_err(f, src, BAD_COOKIE); count(bind_bad_cookie); return  # (3) ucuz; HİÇBİR durum/sayaç yazılmaz
  if bind_fail.get(src)?.held(): count(bind_held); return                   # (4) adres kilidi (yalnız geçerli cookie sonrası yazılmış), sessiz
  if not verify_budget.take(): reply_err(f, src, RATE_LIMITED); return      #     bütçe cookie'den sonra
  tbs = domain_sep("KAMA-HUB-BIND-V1") ‖ hub_pub ‖ req.cert_ref ‖ req.client_nonce ‖ req.hub_nonce
  if verify_strict(dev.pub, tbs, req.sig) fails:                            # (5)
      bind_fail.hit(src)  # LRU; dolunca en eski atılır
      reply_err(f, src, POSSESSION_PROOF_FAIL); return
  if ep is None:                                                            # (6) kabul
      ep = endpoints[dev.dest_hash] = Endpoint{addr: src, cert_ref, bind_epoch: 0, last_accepted: None, link_count: 0}
  elif ep.addr is not None and ep.addr != src: del by_addr[ep.addr]        # adres TAŞINIR, hatlar kalır
  if (old = by_addr.get(src)) and old != dev.dest_hash:                      # H3: X adresini başka kimlik tutuyordu
      endpoints[old].addr = None; count(bind_displaced)                      #   eski uç adressiz kalır; hatları/anahtarları korunur
  ep.addr = src; by_addr[src] = dev.dest_hash; ep.last_activity = now
  ep.bind_epoch += 1                                                        # bundan önce verilmiş HER cookie artık geçersiz
  ack = BindResp{bind_ok=1, 0, 0, OK}; ack.hub_sig = sign(hub_sk, domain_sep("KAMA-HUB-ACK-V1") ‖ cert_ref ‖ client_nonce ‖ hub_nonce ‖ 1 ‖ OK)
  bytes = reply(f, src, ack); ep.last_accepted = {req.client_nonce, req.hub_nonce, src, bytes}
  log("bind ok dh=<ilk 8 hex> epoch=…")

on_store(f, src):                                     # H4, H5, H6, H11, S4 §2.4(c)
  me = by_addr.get(src) or {count(src_unbound); return}                      # SESSİZ (H4: adres hiçbir uca bağlı değil)
  if not store_rate[me].take(): reply_err(f, src, RATE_LIMITED); return
  req = parse(f.payload) else {reply_err(f, src, BAD_REQUEST); return}
  if (req.hub_handle, req.envelope_type) ∉ VALID_COMBOS (S4 §2.4(c) tablo): reply_err(f, src, BAD_REQUEST); return
  if req.envelope_type == 0x05 and len(req.envelope) > RELAY_OPEN_MAX_ENVELOPE: reply_err(f, src, BAD_REQUEST); return
  if req.recipient ∉ Directory.dest_hashes or req.recipient == me: reply_err(f, src, BAD_REQUEST); return
  if req.hub_handle == 0:                                                   # YENİ HAT (H5; S4 §2.6 planlayıcısı, 8 adım)
      key = (me, f.request_id, req.recipient, req.envelope_type, blake2s(req.envelope)[0..8])
      h = by_key.get(key)                                                   # 1a aktif anahtar → aynı hat, yeniden DELIVER
      if h is not None:
          if not bound(req.recipient): reply_store(f, src, PEER_UNBOUND, 0); return     # bound(x) = x ∈ endpoints and endpoints[x].addr is not None
          reply_store(f, src, OK, h); send(endpoints[req.recipient].addr, DELIVER{h, sender=me, req.envelope_type, req.envelope}); return
      if key ∈ tomb_by_key: reply_store(f, src, UNKNOWN_HANDLE, 0); count(store_key_retired); return   # 1b emekli anahtar: tahsis YOK, DELIVER YOK
      if not bound(req.recipient): reply_store(f, src, PEER_UNBOUND, 0); return                       # 1c
      if not alloc_rate[me].take(): reply_err(f, src, RATE_LIMITED); return                            # 2 (bütçe harcanır — garanti dışı)
      plan = set()                                                                                     # 3
      if endpoints[req.recipient].link_count ≥ MAX_LINKS_PER_ENDPOINT:                                 # 4 alıcı kotası
          shared = oldest_idle(L for L in links if {L.a, L.b} == {me, req.recipient})                  #   YALNIZ ortak hatlar
          if shared is None: reply_err(f, src, HUB_CAPACITY); return                                   #   ortak hat yok → dokunma
          plan.add(shared)
      after(x) = endpoints[x].link_count - |{L ∈ plan : x ∈ {L.a, L.b}}|                               # 5 plan sonrası sayı
      if after(me) ≥ MAX_LINKS_PER_ENDPOINT: plan.add(oldest_idle(L for L in links if me ∈ {L.a, L.b} and L ∉ plan))
      if len(links) - len(plan) ≥ MAX_LINKS: plan.add(oldest_idle(L for L in links if L ∉ plan))     # 6 küresel; küme
      ok = after(me) + 1 ≤ MAX_LINKS_PER_ENDPOINT and after(req.recipient) + 1 ≤ MAX_LINKS_PER_ENDPOINT   # 7 denetimler
           and len(links) - len(plan) + 1 ≤ MAX_LINKS
           and len(tombstones) + len(plan) ≤ MAX_TOMBSTONES
      h = pick_handle() if ok else None                                     #   aday; yan etkisi yalnız alloc_counter
      if not ok or h is None: reply_err(f, src, HUB_CAPACITY); return       #   HİÇBİR ŞEY DEĞİŞMEDİ (hatlar, sayaçlar, tombstone)
      for L in plan: retire(L)                                               # 8 ancak şimdi
      links[h] = {a: me, b: req.recipient, last_activity: now, key}; by_key[key] = h
      endpoints[me].link_count++; endpoints[req.recipient].link_count++; log("link open h=…")
      reply_store(f, src, OK, h)
      send(endpoints[req.recipient].addr, DELIVER{h, sender=me, req.envelope_type, req.envelope})
  else:                                                                     # MEVCUT HAT (H6)
      L = links.get(req.hub_handle) or {reply_store(f, src, UNKNOWN_HANDLE, 0); return}
      if me ∉ {L.a, L.b}: reply_err(f, src, BAD_REQUEST); return
      other = L.b if me == L.a else L.a
      if req.recipient != other: reply_err(f, src, BAD_REQUEST); return     # karşı uç denetimi
      if not bound(other): reply_store(f, src, PEER_UNBOUND, 0); return
      L.last_activity = now
      reply_store(f, src, OK, req.hub_handle)
      send(endpoints[other].addr, DELIVER{req.hub_handle, sender=me, req.envelope_type, req.envelope})

on_data(h, d, src):                                   # H4, H7
  L = links.get(h)
  if L is None:
      if (me = by_addr.get(src)) and route_miss_rate[me].take(): send(src, ROUTE_MISS{h, UNKNOWN_HANDLE})
      else count(drop_unknown_link)
      return
  me = by_addr.get(src) or {count(src_unbound); return}                      # H4: adres hiçbir uca bağlı değil (eski adres dahil)
  if me ∉ {L.a, L.b}: count(src_mismatch); return                            # H4: bağlı uç ama bu hattın ucu değil
  other = L.b if me == L.a else L.a
  if not bound(other): count(drop_peer_unbound); return                       # karşı uç adressiz (hiç bind olmadı / adresi alındı)
  L.last_activity = now; endpoints[me].last_activity = now
  send(endpoints[other].addr, d)                        # bayt bayt, değiştirmeden

pick_handle():                                        # S4 §2.6, H9 — yan etkisi YALNIZ alloc_counter
  for _ in 0..8:
      h = blake2s(handle_secret ‖ alloc_counter)[0..4]; alloc_counter += 1   # her deneme ilerletir (başarısız da)
      if h != 0 and h ∉ links and h ∉ tombstones: return h
  return None                                           # üstüne yazma YOK; sonraki istek yeni adaylar dener

retire(h): L = links.pop(h); del by_key[L.key]; endpoints[L.a].link_count--; endpoints[L.b].link_count--
           tombstones[h] = {retired_at: now, key: L.key}; tomb_by_key[L.key] = h; log("link retire h=…")
tick(every 60 s): for h in tombstones where age ≥ HANDLE_TOMBSTONE: del tomb_by_key[tombstones[h].key]; del tombstones[h]   # YALNIZ süresi dolanlar
                  bind_fail.retain(age < BIND_FAIL_HOLD)       # kilit girdileri süresi dolunca
                  rotate cookie_secret hourly (prev saklanır)
```

`reply(f, src, …)`: aynı `method_id`, `flags = 0x01`, aynı `request_id`, sabit yerleşim (S4 §2.4). `reply_err(f, src, code)`: aynı `method_id`, `flags = 0x03`, aynı `request_id`, `ErrorPayload{code, 0}`. `reply_store`: `StoreResp{status, handle}`, `flags = 0x01`. Hub→tel sunucu-başlatımlı mesajlar (`DELIVER`, `ROUTE_MISS`): `flags = 0`, `request_id = 0`.

## 4. Sınırlar ve sayılar

Adıyla S4 §6'dan: `MAX_DATAGRAM`, `MAX_LINKS`, `MAX_LINKS_PER_ENDPOINT`, `MAX_TOMBSTONES`, `HANDLE_TOMBSTONE`, `COOKIE_ROTATE`, `COOKIE_WINDOW`, `VERIFY_BUDGET`, `BIND_FAIL_MAX`, `BIND_FAIL_HOLD`, `BIND_FAIL_ENTRIES`, `STORE_RATE`, `LINK_ALLOC_RATE`, `ROUTE_MISS_RATE`, `STATS_PERIOD`, `RELAY_OPEN_MAX_ENVELOPE`. Hız sınırları token bucket (`rate.rs`). Hiçbir sayı bu belgede yeniden yazılmaz.

## 5. Kod düzeni ve bağımlılıklar

- Tek ikili, **tek iş parçacığı**: `recv_from` (okuma zaman aşımı `STATS_PERIOD`) döngüsü; async/tokio yok.
- Modüller: `main.rs` · `config.rs` · `wire.rs` (**yalnız** `kama_contracts::s4` codec'ini çağırır) · `bind.rs` (cookie[epoch], imza, kayıp-ACK tekrarı, adres kilidi LRU) · `table.rs` (Directory/Endpoints/Links/Tombstones; `by_key` ve `tomb_by_key` indeksleri; planlayıcı `plan_new_link()` saf fonksiyon — durum değiştirmez, plan döndürür) · `relay.rs` (store/data/route-miss) · `rate.rs` · `stats.rs` · `probe.rs` (`hub-server probe`: bind + 3 heartbeat; cihaz dosyasından okuduğu alanlar: cihaz Ed25519 gizli anahtarı, `cert` (→ `cert_ref = SHA-256(cert)`), `hub.addr`, `hub.ed25519_pub` — v1 dosya biçimi = bugünkü v0 anahtar adları + `format_version: 1` + `hub.ed25519_pub`; normatif ad listesi A2.5'in `PROVISIONING-FORMAT-v1.md`'sinde, o gelene kadar alan adları v0 çıktısından alınır, tahmin edilmez).
- Bağımlılık: `kama-contracts` · `kama-noise-wire` (`ed25519_strict::verify_strict`, `entropy::OsEntropy`, `handshake::cert_ref_of`) · `ed25519-dalek` (imzalama) · `blake2` · `sha2` · `serde`/`serde_json`. Hepsi Cargo.lock'ta; **yeni crate yok**.
- Saat (`trait Clock`) ve rastgelelik (`EntropySource`) dışarıdan verilir.
- Log: `STATS_PERIOD`'da bir satır `hub stats t= endpoints= links= tombstones= bind_fail_entries= relay_pps= relay_bps= bind_ok= bind_ack_repeat= bind_bad_cookie= bind_fail= bind_unknown= bind_held= store_ok= store_repeat= store_key_retired= peer_unbound= hub_capacity= route_miss= drop_size= drop_malformed= drop_unknown_frame= drop_unknown_link= src_mismatch= src_unbound= rate_store= rate_alloc= hb_unbound=`; olaylar `bind ok`, `link open`, `link retire`, `cookie rotate`. **Yük, tam kimlik, anahtar, zarf asla loglanmaz.**

## 6. `hub.json` (kama-provision v1 üretir)

```json
{ "format_version": 1,
  "listen": "0.0.0.0:47000",
  "hub_ed25519": { "priv": "<64 hex>", "pub": "<64 hex>" },
  "devices": [ { "name": "alice", "dest_hash": "<64 hex>", "cert": "<204 base64>" },
               { "name": "bob",   "dest_hash": "<64 hex>", "cert": "<204 base64>" },
               { "name": "probe", "dest_hash": "<64 hex>", "cert": "<204 base64>" } ] }
```
Bilinmeyen alan → hub **başlamaz**. Tek gizli alan `hub_ed25519.priv`. Dosya konteyner kullanıcısına (65532) okunabilir olmalı (Deploy TDD §6.2); okunamazsa hub tek satır `config unreadable: <yol>` yazar ve çıkış kodu 1 ile biter (Deploy T-DEP-6b buna dayanır). `cert_ref = SHA-256(cert)`, `device_ed25519_pub = cert[0..32]` açılışta türetilir.

## 7. Testler — önce yazılır (her satır §3'teki bir dala karşılık gelir)

Araçlar: bellek içi soket çifti, sahte saat, sabit `cookie_secret`/`handle_secret`, `vectors/hub/` altın vektörleri.

**Bind** · T-B1 iki tur doğru → `bind_ok=1`, hub imzası doğrulanır, `endpoints` dolu, `bind_epoch = 1` · T-B2 dizinde olmayan `cert_ref` → iki turda da **cevap yok** · T-B3 yanlış anahtarla / başka hub pub'ıyla imza → `POSSESSION_PROOF_FAIL` (ErrorPayload, imzasız), kayıt yok, `bind_fail[src]` = 1 · T-B4 2 dakika eski `hub_nonce` → `BAD_COOKIE`; **imza doğrulaması hiç çağrılmaz** (sayaçla); `bind_fail` **boş kalır** · T-B5 aynı kimlik yeni nonce'la yeni adresten → adres taşınır, hatlar korunur, eski adresten veri **`src_unbound`** (eski adres artık hiçbir uca bağlı değil) · **T-B12 aynı adresten ikinci kimlik:** alice X'ten bağlı; bob X'ten geçerli kanıt → `by_addr[X] = bob`, alice adressiz (`addr = None`), alice'in hatları ve anahtarları **yerinde**; carol → alice STORE → `PEER_UNBOUND`; alice'e veri → `drop_peer_unbound`; alice Y'den yeniden bind → her şey devam · **T-B13 cookie adres ailesi:** IPv4 kaynağı `fam = 0x04` + 4 B, IPv6 `0x06` + 16 B; `::ffff:a.b.c.d` ile `a.b.c.d` **aynı** cookie'yi üretir · **T-B6 epoch tekrarı (halka yok, N bağımsız):** A'da bind (kanıt₀ saklanır), B'den **20** ayrı bind, sonra kanıt₀ A kaynağıyla tekrar (cookie penceresi içinde, sahte saat) → `BAD_COOKIE`, adres **B'de kalır**, `bind_epoch = 21` · **T-B6b gecikmiş kanıt:** A ve B için iki 1. tur (aynı epoch), B'nin kanıtı önce kabul, sonra A'nınki → `BAD_COOKIE`, adres B'de · **T-B6c tek bind:** A'dan 1. tur, kanıt 50 s gecikip gelir (pencere içinde), arada başka kabul yok → **kabul** (en yeni bind odur) · T-B7 kayıp ACK: aynı kanıt aynı adresten tekrar → **bayt-özdeş** yanıt, `bind_epoch` değişmez, `bind_ack_repeat` +1 · T-B7b kayıp ACK sonrası taşınma: A'da kabul (ACK kayıp), B'de kabul, A-kanıtı A'dan tekrar → `BAD_COOKIE` · **T-B8 kimlik kilitlenemez:** saldırgan C adresinden Alice'in `cert_ref`'iyle 50 geçersiz **imza** (gerçek cookie'lerle) → C adresi `BIND_FAIL_HOLD` boyunca sessiz, **gerçek Alice kendi adresinden bağlanır** · **T-B8b adres de kilitlenemez:** Alice'in **adresini taklit eden** 50 yanlış cookie → `bind_fail` girdisi **yok**, Alice'in sonraki geçerli bind'i **kabul** · T-B9 geçersiz cookie seli `verify_budget`'ı **tüketmez** · T-B10 1. tur 10 000 kez (dizindeki kimlikle, rastgele adreslerden) → tablo boş, bellek sabit, cevap boyu ≤ istek · **T-B11 kilit tablosu sınırı:** 10 000 farklı erişilebilir adresten geçersiz imza → `bind_fail` ≤ `BIND_FAIL_ENTRIES`, en eskiler atılmış; 60 s sonra boş.

**Zarf** · T-S1 `STORE{0, INIT, bob}` → `{OK, h}` ve bob'a `DELIVER{h, alice, INIT, aynı zarf}` · T-S2 aynı istek tekrar (aynı request_id + alıcı + tip + zarf) → **aynı h**, bob'a **yine DELIVER**, `alloc_rate` harcanmaz · T-S3 aynı request_id, **farklı alıcı** → **yeni** h, doğru alıcıya DELIVER · T-S4 bob bağlı değil → yanıt `PEER_UNBOUND`, hat yok · T-S5 `STORE{h, RESPONSE, alice}` bob'dan → alice'e `DELIVER{h, bob, RESPONSE, zarf}` · T-S6 `STORE{h, RESPONSE, carol}` (alıcı karşı uç değil) → `BAD_REQUEST` · T-S7 `STORE{h,…}` hattın ucu olmayan carol'dan → `BAD_REQUEST` · T-S8 `{0, RESPONSE}`, `{h, INIT}`, tip 0x03/0x04 → `BAD_REQUEST` · T-S9 bağsız adresten STORE → sessiz · T-S10 `RELAY_OPEN` 16 B → INIT ile aynı yol; 65 B → `BAD_REQUEST`; `{h, RELAY_OPEN}` → `BAD_REQUEST` · T-S11 `STORE_RATE` aşımı → `RATE_LIMITED`; `LINK_ALLOC_RATE` aşımı → `RATE_LIMITED` (tekrarlar tahsis sayılmaz) · **T-S12 aktif anahtar unutulmaz (K7):** alice bir INIT yollar (anahtar K, hat h); arada **başka uç çiftleri** arasında 200 hat açılır ve kapanır (sahte saat; eski LRU sınırının çok üstünde; alice'in kotasına dokunulmaz, h emekli edilmemiştir); K yinelenir → **aynı h**, bob'a yeniden DELIVER; `[h][veri]` iki yönde hâlâ geçer (hat gerçekten aktif) · **T-S13 emekli anahtar:** h emekli edildikten sonra K yinelenir → yanıt `{UNKNOWN_HANDLE, 0}`, **tahsis yok, DELIVER yok**, `links` değişmez · **T-S14 tombstone süresi dolunca:** 7 gün sonra K yinelenir → anahtar hiçbir yerde → normal yeni tahsis (yeni h) · **T-S15 indeks boyutu:** her adımda `len(by_key) == len(links)` ve `len(tomb_by_key) == len(tombstones)` (değişmez denetimi).

**Veri** · T-D1 `[h][1276 rastgele bayt]` alice'ten → bob'a **bayt bayt aynı** · T-D2 1281 B → `drop_size` · T-D3 doğru h, alice'in bağlı adresinden değil: kaynak **bağlı başka uç** (carol) ise `src_mismatch`; kaynak **bağsız adres** ise `src_unbound` — iki ayrı vaka · T-D4 bilinmeyen h bağlı alice'ten → `ROUTE_MISS{h, UNKNOWN_HANDLE}`, en çok `ROUTE_MISS_RATE`; bağsız adresten → sessiz · T-D5 karşı uç bağlı değil → `drop_peer_unbound`.

**Heartbeat** · T-H1 bağlı alice, token'lı → ACK aynı uzunluk, token aynı · T-H2 bağsız adres → cevap yok · T-H3 gövde 2 B → `drop_malformed`.

**Bellek (sahte saat)** · T-M1 **gönderen kotası:** alice'in 65. hattı → alice'in en-uzun-boşta hattı emekli (tombstone), diğer uçların hatları etkilenmez; hem alice'in hem karşı ucun sayacı −1 · **T-M1b alıcı kotası, ortak hat var (K4):** alice–bob 1 kurulu oturum + 63 kapalı medya hattı (ikisi de 64'te); alice yeni RELAY_OPEN → plan = alice–bob **ortak** en-uzun-boşta medya hattı → yeni hat açılır, iki uç da ≤ 64, kurulu oturum dokunulmamış · **T-M1c alıcı kotası, ortak hat yok (K5):** bob 64 hatta, carol'ın bob'la ortak hattı yok → carol→bob `HUB_CAPACITY`; alice–bob dahil **diğer çiftlerin hatları ve sayaçlar aynen** · **T-M1d üçüncü taraf seçilemez:** bob 64 hatta, carol'ın bob'la 1 ortak hattı var → yalnız o ortak hat emekli; alice–bob hatlarına dokunulmaz · T-M2 `MAX_LINKS` dolu → küresel en-uzun-boşta emekli · T-M3 tombstone'daki h 7 gün yeniden verilmez; 7 gün sonra serbest · **T-M4 kapasite planı (K6):** alice 64 hatta ve tombstone tablosu dolu → yeni hat `HUB_CAPACITY`, **alice'in 64 hattı yerinde**, uç sayaçları ve tombstone içeriği değişmemiş (erken silme yok); yalnız `alloc_rate` bütçesi ve `alloc_counter` ilerlemiş olabilir · **T-M5 aday tükenmesi:** 8 deneme çakışması (sabit `handle_secret`; adaylar önceden hat/tombstone olarak kurgulanır) → `HUB_CAPACITY`, mevcut hat **üstüne yazılmaz**, plan uygulanmamış, `alloc_counter` **8 ilerlemiş**; bir sonraki istek **farklı** adaylarla başarılı · T-M6 6 saat sessiz uç ve hat → **hiçbir şey silinmez** · T-M7 cookie rotasyonu sınırında önceki anahtarla nonce kabul · T-M8 tavanlar hiçbir anda aşılmaz: rastgele 100 000 STORE/retire dizisi boyunca `links ≤ MAX_LINKS`, her uç `≤ MAX_LINKS_PER_ENDPOINT`, `tombstones ≤ MAX_TOMBSTONES`, ve her `HUB_CAPACITY` yanıtında `(links, link_count'lar, tombstones)` üçlüsü istek öncesiyle **özdeş** (değişmez denetimi her adımda) · T-M9 plan kümedir: gönderen kotası + küresel tavan aynı hattı işaret ettiğinde hat **bir kez** emekli edilir, tombstone sayısı +1.

**Dayanıklılık** · T-R1 rastgele adreslerden 10 000 anlamsız paket → 0 cevap · T-R2 1 000 000 rastgele/kesik datagram → panik yok · T-R3 hub'ın her çıkışı bir girişe cevap ya da bağlı uca iletim (sayaçla); **doğrulanmamış kaynağa** giden her cevap ≤ istek (CLIENT_BIND meydan okuması, BIND_PROOF hataları; veri/STORE/heartbeat → cevapsız); **bağlı** kaynağa giden `ROUTE_MISS`/hata yanıtları istekten büyük olabilir ama uç başına hızla sınırlı (5 B veri seli → en çok `ROUTE_MISS_RATE` cevap) · T-R4 `flags ≠ 0` ya da bit7'li method_id ile gelen istek → `drop_malformed` · T-L1 loglarda yük/anahtar/tam kimlik yok · T-C1 bozuk/bilinmeyen alanlı `hub.json` → başlamaz; T-C2 tek gizli alan `hub_ed25519.priv`; T-C3 okunamayan dosya → `config unreadable: <yol>`, çıkış 1, başka çıktı yok.

**Uyum** · `vectors/hub/` altın vektörleri (S4 §2.3 tablosunun her satırı dahil): encode/decode Python baytlarıyla özdeş · S4 harness: gerçek hub ↔ sahte istemci yeşil.

**Kapanış vakaları (S4 §7 K1–K11):** hub tarafı K4, K5, K6, K7 bu belgede (T-M1b, T-M1c, T-M4, T-S12); K1–K3, K8–K11 iki ucu da gerektirir → A2.4 harness'ı ve A3 testleri; hub bu vakalarda yalnız S4 H5/H6/H7'yi uygular.

**Kabul (Y1 DONE):** hepsi yeşil; vektörler Python'dan; harness yeşil; `layer_check` yeşil; PR gövdesinde "S4 kuralı → test" tablosu (H1–H12 her biri en az bir testle). "Geçti" yalnız gerçekten koşmuş testler için yazılır.

## 8. Sözlük

**uç (endpoint):** bind olmuş bir cihaz = kimlik + dönüş adresi · **hat (link) / handle:** bir oturumun hub'daki yolu ve her veri datagramının ilk 4 baytı; iki uçta aynı · **zarf:** oturum kurma mesajı (INIT/RESPONSE) ya da medya hattı açılışı; hub için opak · **cookie:** hub'ın hatırlamak yerine hesapladığı meydan okuma sayısı; adrese ve ucun `bind_epoch`'una bağlı · **bind_epoch:** ucun kabul edilmiş bind sayısı; her kabul, ondan önce verilmiş bütün cookie'leri geçersiz kılar (eski kanıt adresi geriye taşıyamaz) · **last_accepted:** son kabul edilen kanıt + yanıtı; yalnız kayıp ACK'te aynı yanıtı yinelemek için · **emeklilik planı:** yeni hat için gerekecek emekliliklerin kümesi (alıcı kotası → yalnız gönderen–alıcı ortak hattı; gönderen kotası → kendi kalan hattı; küresel tavan); bütün denetimler geçmeden uygulanmaz · **tahsis anahtarı (`allocation_key`):** hattı açan isteğin kimliği; hattın alanıdır, emekli olunca tombstone'a taşınır; emekli anahtarla tekrar `{UNKNOWN_HANDLE, 0}` alır · **tombstone:** emekli handle'ın 7 gün yeniden verilmemesi.

## Ek — v0.4 → v0.5
Nonce halkası kaldırıldı → `bind_epoch` + `last_accepted` (ikinci inceleme 1) · yanlış cookie durum yaratmaz; `bind_fail` yalnız geçerli cookie sonrası, LRU `BIND_FAIL_ENTRIES` (2) · `on_store` yeni hat: plan → aday → emeklilik → yazma; alıcı kotası `HUB_CAPACITY`; `pick_handle` sayacı her denemede ilerletir (3) · idempotency LRU isabette tazelenir (4) · testler T-B6/6b/6c/7b/8b/11, T-S12, T-M1b/M4/M5/M8.

## Ek — v0.5 → v0.6
Idempotency LRU kaldırıldı → `links[h].key` + `by_key` (aktif) + tombstone `{retired_at, key}` + `tomb_by_key` (KR-19); emekli anahtar → `{UNKNOWN_HANDLE, 0}` · `on_store` yeni hat: sekiz adımlı planlayıcı (alıcı kotası yalnız ortak hat; plan küme; denetimler geçmeden hiçbir şey değişmez; garanti dışı yalnız `alloc_rate` ve `alloc_counter`) · testler T-S12 (aktif anahtar unutulmaz, veri geçer), T-S13/14/15, T-M1b/1c/1d, T-M4, T-M8 (başarısız tahsiste durum özdeş), T-M9 · kapanış vakaları S4 §7 K1–K11 ile eşlendi.

## Ek — v0.6 → v0.6.1 (Yaaz'ın soruları, 2026-10-15)
`cookie()` adres ailesi ve normalizasyon · `on_bind2` adres çakışması: son kabul adresi alır, eski uç adressiz (`addr = None`), `bind_displaced` sayacı; `bound(x)` yardımcısı · `src_unbound` (adres bağlı değil) ↔ `src_mismatch` (bağlı uç, hattın ucu değil) her yerde aynı anlamda; T-B5 ve T-D3 düzeltildi · T-R3 yansıtma kuralı S4 H10 ile · probe'un okuduğu alanlar ve v1 biçimi notu · yeni T-B12, T-B13.
