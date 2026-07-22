# DR-1b ÖN-KOŞUL-CHECKLİSTİ [DR-DEĞİL — gelecek-1b-DR'ının-zorunlu-girdi-listesi]

> 1b = parity-token/budget-politikası (budget_cap-kablolama, token-artığı-zamanlaması, BtlBw-offered-
> kilitlenmesi-kararı). Bu-dosya 1b-DR-yazılmadan-önce-karşılanması/temsil-edilmesi-gereken-kayıtları-
> toplar. 1b-İZNİ-VERİLMEDİ; buradaki-hiçbir-madde-implementasyon-değildir.

- [ ] **Token-denial-attribution (Commit-K/L-kaydı):** `token_denied_r_gauge` block/event-attribution
  taşımaz → **bu-değer attribution-olmadan calibration-veya-budget-policy-verisi-olarak-KULLANILMAZ.**
  1b-tasarımı-önce-asgari-bağlamı-tanımlamalı: denial-event-index + (mümkünse)-block-id/generation +
  mode + pending-count. (Gauge'un-mevcut-§8.1-exact-tanımı: SON-plan_fec_parity-token-reddi-anında-
  kuyrukta-bekleyen-parity-adedi; kümülatif-değil.)
- [ ] **BtlBw-offered-kilitlenmesi:** DEĞİŞİKLİK-YASAĞI-sürüyor; 1b-tasarım-girdisi-olarak-bulgu-
  dosyada (kesit-1-audit: underutilized-linkte delivered-max-offered'ı-ölçer; gap80-r2-bimodal).
- [ ] **Requested→emitted-ayrışması:** diag-zinciri (enqueued/emitted/canceled/token_denied) 1a'da-
  ayrıştırıldı; 1b-politika-değerlendirmesi-collapse-hücrelerini (kesit-1 d1000-FULL-COLL) yeniden-
  örneklemeli.
- [ ] **Emergency-arbitrasyon:** emergency-interlock ("Temporary safety interlock pending 1b/1d
  arbitration") 1b/1d-tasarımında-çözülmeli; emergency/no-rule-örtüşme-bulgusu (kesit-2-plan §G)
  politika-kararından-önce-veriyle-ele-alınmalı.
- [ ] **DATA-ÖNCE-sıralaması:** mühürlü; 1b token-politikası-sıralamayı-değiştirmez (değiştirecekse-
  ayrı-açık-karar).
