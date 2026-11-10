# QK — G2 gösterim sahnesinde quinn kolu (2026-11-09)

Dal `yol3/quinn-kol` @ **d93aef10** (origin'de) = yol2/o1 5fdad83b + 354584d3 (bench) + d93aef10 (düzenek). Worktree /home/ryuzaki/kama-wt/yol3-quinn-kol. Ürün crate'leri, tel ve altın vektörler değişmedi; Cargo.lock değişmedi.

## Kollar [kod]
`bench f0-quinn --role alice|bob <hücrenin F0A_ARGS'ı aynen> --q-kol <kol>` (crates/bench/src/f0q.rs). f0_flows'un akış takvimi, gövde/boyları, sync işaretleri, tamamlanma kuralı ve çıktı biçimi aynen; f0a_analyze/f0a_tam/oku.py değişmeden okur.
- `Q_bbr`: BBR, IW 240 000 B (quinn BBR varsayılanı, mevcut bench tanımı)
- `Q_bbr_rfc`: BBR, IW 14 520 B = RFC 9002 §7.2 min(10·MSS, max(2·MSS, 14720)), MSS = initial_mtu 1452
- `Q_cubic`: Cubic, IW 12 000 B (quinn varsayılanı)
- Ortak: quinn 0.11.11 / quinn-proto 0.11.18, TLS 1.3, idle 300 s, initial_rtt 333 ms, MTU 1452, PMTUD/GSO kapalı, uni-stream kredisi 512, mesaj başına uni-stream (her PTT demeti ayrı), öncelik yok. bob TEXT başına alındı ('R') ve dosya alındısı ('D') döner.

## Koşu (KOSU-KAYDI biçimi; orkestratör)
```
ağaç: yol3/quinn-kol d93aef104b01f7363935e859de0f89c389f909ab = yol2/o1 5fdad83b + QK
derleme (D-34): KAMA_AGENT=ORK CARGO_TARGET_DIR=/home/ryuzaki/kama-wt/target-quinn-kol scripts/ci/cargo-lock.sh build --release --locked -p bench --bins
export BIN_QK=/home/ryuzaki/kama-wt/target-quinn-kol/release/bench
cd /home/ryuzaki/kama-wt/yol3-quinn-kol
Q=~/kama-k4g-work/yol3/quinn/2026-11-10-q; mkdir -p "$Q"
for c in ~/kama-k4g-work/yol3/tesis/2026-11-09-g2/a_r12k_d150_l20_foto150k__K*; do [ -d "$c" ] && ln -s "$c" "$Q/"; done
systemd-inhibit --what=sleep:idle --why="YOL3 quinn kolu" setsid scripts/ktp2/f0/f0_grid.sh -j 3 "$Q" $(cat scripts/yol3/tesis/SIRA-Q.txt)
python3 -I scripts/yol3/tesis/oku.py --kok "$Q" --manifest docs/design/E1-TESHIS-G2.md   # → $Q/okuma/OKUMA.md "Gösterim sahnesi" tablosu
```
3 hücre × 10 tekrar = 30 iş; G2 köküne yazılmaz (Kama hücreleri sembolik bağla). Süre tahmini [çıkarım]: Kama koşuları 104–127 s → hücre başına ≈ 20 dk, -j 3 ile ≈ 20–25 dk.

## Doğrulama
- [ölçüldü] build -p bench --bins, clippy -p bench --all-targets -D warnings, fmt --check yeşil; f0q birim testleri 9/9.
- [ölçüldü] Loopback duman (debug, netem yok, kısaltılmış takvim, ≈ 4 s/kol): üç kolda alice complete, alındı 7/7, dosya 150 480 B doğru; f0a_analyze üçünü de geçerli okudu.
- [ölçüldü] oku.py, G2 kökünde (160 koşu): OKUMA.md + okuma.json 5fdad83b ile bayt bayt aynı (Q yoksa sahne bölümü yazılmaz). Sahte kökte K0/K2 + Q satırları aynı tabloda; K0 23.9 s / 15.29 s, K2 17.5 / 9.24 (OKUMA.md ile aynı).
- [ölçüldü] oku.py ↔ yol3/o-goster@502fce94 (KK-ORK-03) üç yollu birleşme çakışmasız, öz-test 31/31 + 3/3. Dalımın oku.py'si KK-ORK-03'süz (taban 5fdad83b): S tablosundaki (iii) için 502fce94'ü birleştirin; sahne tablosu (iii) kullanmaz.

## Adillik farkları
1. El sıkışma [kod]: quinn'inki netem İÇİNDE (kayıpta saniyeler; 30 s'te bağlanamazsa koşu hata). Kama zarfları Unix soketinden, netem DIŞINDA. İkisinde de t0 = bağlantı sonrası → kurulum pencere dışında. `t0` işaretinde `setup_ms` var; tabloda "kurulum" sütunu. quinn t0'a el sıkışmasından gelen RTT örneğiyle giriyor, Kama soğuk giriyor.
2. Öncelik [kod]: eşitlendi. Bütün stream'ler 0, quinn send_fairness ile sırayla dolduruyor. mixed.rs'teki PTT > TEXT > dosya önceliği yalnız `--q-oncelik on` ile açılıyor; hücresi yok.
3. Sıralama [kod]: eşitlenmedi. quinn stream'leri bağımsız (akışlar arası HOL yok, demet başına stream); Kama ürün yolunda oturum geneli S1 Reorder var. Bu kol quinn için üst sınır.
4. Karşı örnek tetiği [çıkarım]: vekil. Yeni veri tükendi ≈ udp_tx − lost − datagram × 27 B ≥ verilen uygulama baytı; teyitsiz ≥ 3 birim ≈ dosyanın bob'a ulaşmamış kısmı ≥ 3 × 1013 B. Diğer şartlar aynen. BBR-240 kB'de tetiğin erken (≈ file_at + 1 s) gelmesi beklenir.
5. Uygulama katmanı [kod]: Kama'da kama-app motorları çalışıyor (1013 B dosya parçaları, FILE_RECEIPT_TTL 60 s → alice ReceiptTimeout). quinn'de çerçeve en az (1 B tür + mid), dosya tek stream, TTL yok. Tamamlanma ve dosya ölçüsü iki kolda da bob'un alımı.
6. Kripto [kod]: ikisi de gerçek (quinn TLS 1.3, Kama Noise). Paket ek yükü quinn ≈ 27 B + çerçeve, Kama 22 + 11 B. FEC iki kolda da yok (KK-ORK-01).
7. Teşhis [kod]: quinn'de motor sondası ts/tr yok, yerine qs yazılıyor. (iii) emisyon ve R4 analog sütunları Q için boş; sahne tablosu yalnız (i), (ii), teslim ve kurulum.
8. Durma senaryoları quinn'de öykünülmüyor (tipli hata); G2'de kullanılmıyor.

## İkincil — quinn'i ürünün taşıyıcısı yapmak [kod/çıkarım]
1. `carrier::product_envelope_link` yalnız el sıkışma zarflarını (INIT/RESPONSE) taşır. Veri yolu `ClientTransport` (client-core transport.rs): düğüm başına `ServerEndpoint::bind_expecting` (≈ :574), oturum başına `ClientEndpoint::connect_session_counted` (≈ :1104). Mühür `PacketProtection` ile paket düzeyinde, runtime'ın içinde.
2. Spec yolu ADR-14 / transport-layer-tdd §440 Option A: yalnız güvenilir taşıma (substrate) değişir; Session V3 AEAD, replay, hub_handle ve S1 anlamı aynı kalır; fork yok.
3. En kısa gösterim yolu: client-core'a quinn'li bir runtime ucu (örn. `runtime_quinn.rs`) yazmak; ClientTransport'un kullandığı alt kümeyi karşılar (send_with_deadline, acked, outstanding, next_delivery, ctx dispatch).
4. Mühür aynı katmanda kalır: quinn soketi `AsyncUdpSocket` ile sarılır (quic.rs GovernedUdpSocket deseni), her QUIC datagramı düğümün PacketProtection'ıyla mühürlenir; initial_mtu − 22 B.
5. Seçim `ClientTransportConfig.runtime`'a bir `carrier` alanıyla yapılır; f0_flows'a `--carrier quinn` eklenir (mevcut `start_with_link_cfg_for_test` kancası yeter). EnvelopeLink ve Reorder dokunulmadan kalır.
6. Dokunulacak dosyalar: client-core/{runtime_quinn.rs (yeni), transport.rs, runtime.rs (config alanı), Cargo.toml}, kama-integration/examples/f0_flows.rs. kama-node'a ancak ürün seçimi istenirse dokunulur.
7. İş miktarı: ≈ 600–900 satır + testler, 3–5 iş günü; netem doğrulaması +1 gün.
8. Risk 1: pki §13 kripto kaydı ve ADR-03. quinn ürüne rustls/ring getirir. ADR-14 metni QUIC'i "hostile ana transport DEĞİL, iyi-ağ yolu" diye sınırlar; bu yüzden sahibin D-kararı gerekir.
9. Risk 2: S1 oturum sırası korunursa quinn'in akış bağımsızlığı uygulama noktasında kaybolur, bu kolun ölçtüğü üst sınıra ulaşılmaz. Akış başına sıra S1 sözleşme değişikliği ister.
10. Risk 3: iki el sıkışma olur (Noise EnvelopeLink + QUIC netem içinden), %20 kayıpta kurulum uzar.
11. Risk 4: live_diag, ts ve birim izi Kama motoruna özgü; f0 enstrümantasyonu kısmen boş kalır. Çift AEAD ek yükü 12,5 kB/s'de küçük ama ölçülmeli.
