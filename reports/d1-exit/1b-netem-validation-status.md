# Z4 — Dar-Netem-Doğrulaması: HAZIRLIK + EXACT-BLOCKER

**BLOCKER (exact):** Bu-oturum/CI-sandbox'ında `sudo`-parolasız-yetki-YOK (`sudo -n` → "password
required"); `ip netns add`/`tc qdisc`-root-gerektirir → GERÇEK-netem-koşusu-bu-ortamda-YAPILAMAZ.
Repo-politikası-uyumlu-koşu-yeri: self-hosted-nightly-runner (§4.4-netem-altyapısı; CLAUDE.md-§7).

**Hazırlık (runnable):** `scripts/netem/run_policy_validation.sh` — veth-namespace-çifti (host-izole;
cleanup-trap); 6-profil (rtt1s+burst / rtt2.5s+GE / rev-blackout / offered≈C / 30kbps / gap80-benzeri);
10-tekrar (p6: 30); arm'lar = Leftover + F15 (shortlist-S1; kararsızlıkta+P15). Eksik-tek-parça:
bench-koşucu-ikilisinin-policy-arg'lı-UDP-modu (script-içinde-beklenen-komut-satırı-hazır; çıktı-şeması
pos-block-ledger-ile-AYNI → `pos_analyze.py`-doğrudan-çalışır). Beklenen-çıktılar/cleanup/güvenlik-
sınırları-script-başlığında; policy-params-canonical-dump = PolicyParams-alanları (commit-hash'li).

**Sim↔netem-yön-tablosu (koşu-sonrası-doldurulacak-iskelet):** F15/P15-recovery-sırası · DATA/ARQ-cost-
yönü · tail-yönü · bimodallik · queue-baskısı · floor-miss · parity-efficiency — her-satır:
deterministic-direction / netem-direction / magnitude / confidence / discrepancy.
**Kural-uygulanır:** netem-YOKKEN-production-default-SEÇİLMEZ; netem-ters-yön → politika-seçimi-DUR.
