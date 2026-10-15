#!/usr/bin/env python3
"""INT-02 analizi (plan K4G-05): `int02_run.sh` çıktılarından tohum × kol tabloları ve GATING hükmü.

    python3 scripts/netem/int02_analyze.py <çıktı-dizini> [--md DOSYA] [--json DOSYA]
    python3 scripts/netem/int02_analyze.py --selftest

Girdi: `<dizin>/seed-<S>-<kol>.json` (örnek `int02_node`'un satırı + sürücünün `driver` ve
`netem_check` kayıtları). Örnek yalnız GÖZLEM yazar; hüküm YALNIZ burada verilir (tek kaynak).
Çıktı: rapora (`docs/reports/INT-02-RAPORU.md`) girecek Markdown tablolar (stdout ya da `--md`).

HÜKÜM KURALLARI (her tohum ayrı; "temiz" = aşağıdaki ortak koşulların hepsi):

  ÖLÇÜM ARACI (her koşu, kol fark etmez) — netem ateşledi:
    ping: gönderilen > 0, alınan < gönderilen (kayıp görüldü), ort. RTT ≥ RTT_FLOOR_MS;
    qdisc: koşu boyunca düşürülen paket > 0. Sağlanmazsa koşu GEÇERSİZ (hüküm verilmez).
  ORTAK TEMİZLİK (iki düğüm): Application retransmit = 0 (`resent_on_new_session` +
    `resend_refused`), Crypto retransmit = 0 (bayt-özdeş el sıkışma zarfı tekrarı),
    `ctx_mismatch_drops` = `hub_frames_dropped` = `delivery_channel_drops` = 0, dinleyici olay
    düşürmedi (`listener_drops` = 0).
  GATING (1) `gating1`: koşu tamamlandı; alice→bob `count` TEXT tam bir kez teslim (teslim =
    gönderilen = `count`, tekrar / içerik uyuşmazlığı / bilinmeyen 0) ve `count` RECEIPT; + ortak.
  GATING (2) `gating2`: bob→alice için aynı teslim koşulu; + epoch 0→1 AKIŞ İÇİNDE: bob'un ilk
    gönderiminde örneklenen epoch'u 0, bob'un epoch-1 ilk görülüşü ilk gönderimden SONRA, iki
    düğümün son epoch'u 1; + TEK OTURUM: iki düğümde `Ready` = 1, `Closed` = 0, `RenewRequired` = 0,
    canlı oturum = 1, `Closed`/`ConnectAbandoned` düğüm olayı 0; + ortak.
  GATING (6) `gating6`: iki yönde teslim koşulu; iki düğümde tek `Connected`, `Closed` /
    `ConnectAbandoned` yok; + ortak. (Erken metin ve "≤ 2 RTT" el sıkışma ölçütü bu düzenekte
    ölçülmez — rapor beyanı.)
  NEGATİF `negatif` (kurtarma KAPALI): ölçümün kaybı GÖREBİLDİĞİ — netem geçerli, alice→bob
    teslim < gönderilen, iki düğümün bastırılan kurtarma kararı toplamı > 0 ve tele çıkan ARQ
    retransmit baytı 0.
  T4: her GATING kolu için tohum sayısı ≥ 5 ve her tohum GEÇTİ.

Yüzdelikler örnekte EN YAKIN SIRA (nearest-rank) yöntemiyle hesaplanır; burada yeniden hesaplanmaz.

YENİDEN KOŞU ALANLARI (CEVAP PAKETİ 4, şema `int02-node-v2`; hüküm kuralları DEĞİŞMEDİ):
  * `params.window_s` + `window.ended_by` — PENCERE: son gönderimden (ek-kesinti kolunda kesinti
    bitiminden) sonra tamamlanma üst sınırı (v1 dosyalarında `params.drain_s`).
  * GATING (2) özeti: geçen / koşulan tohum ve pencere(ler).
  * ek-kesinti: iki ayrı satır grubu — METİN (`outage.texts_complete`: %100 metin teslimi) ile
    `outage.same_session` (kesinti başından koşu sonuna yeni `Closed` yok, ikinci `Connected` yok,
    epoch değişmemiş) ve ayrı PTT tablosu. "Sahip ölçütü (CEVAP PAKETİ 4)" satırı = metin %100 ∧
    same_session (ölçüm özeti; GATING hükmü değildir).
  * ek-dosya-ptt: `file.outcome` (durum `delivered` / `failed` / `pending`, neden, zaman damgaları).

TAKILI BİRİM ÖZETİ (K4G-05 teşhis; hüküm DEĞİL): her koşu için tek satır — son sondadaki gönderen
kurtarma görüntüsü (`nodes.*.probe.sessions[].recovery`: teyitsiz birim, en eski teyitsiz birimin
emisyon kararı / ARQ retx düzeyi ve tavanı / motor kayıtları / kopya vadesi ve aralığı / son
gönderimden ve son ACK'ten beri, tail-probe turu ve PTO, motor durumu) ve alıcı sıralı teslim durumu
(`reorder`: beklenen birim, arkasında tutulan). Zaman çizelgesi örnek sayısı `timeline.samples`.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tempfile
from pathlib import Path

RTT_FLOOR_MS = 100.0
T4_MIN_SEEDS = 5
GATING_ARMS = ("gating1", "gating2", "gating6")
ALL_ARMS = GATING_ARMS + ("negatif", "ek-dosya-ptt", "ek-kesinti")
FILE_RE = re.compile(r"^seed-(\d+)-([a-z0-9-]+)\.json$")
DASH = "—"


# ── okuma ────────────────────────────────────────────────────────────────────────────────────

def load(directory: Path) -> dict[tuple[int, str], dict]:
    runs: dict[tuple[int, str], dict] = {}
    for path in sorted(directory.iterdir()):
        m = FILE_RE.match(path.name)
        if not m:
            continue
        seed, arm = int(m.group(1)), m.group(2)
        try:
            runs[(seed, arm)] = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as e:
            runs[(seed, arm)] = {"result": "error", "error": f"okunamadı: {e}"}
    return runs


def get(d, *keys, default=None):
    for k in keys:
        if not isinstance(d, dict) or k not in d:
            return default
        d = d[k]
    return d


def node(rec, name):
    return get(rec, "nodes", name, default={}) or {}


def probe(rec, name):
    return get(rec, "nodes", name, "probe", default={}) or {}


def flow(rec, key):
    return get(rec, "flows", key, default={}) or {}


# ── hüküm ────────────────────────────────────────────────────────────────────────────────────

def netem_check(rec) -> list[str]:
    ping = get(rec, "netem_check", "ping", default={}) or {}
    delta = get(rec, "netem_check", "run_delta", default={}) or {}
    why = []
    sent, received, avg = ping.get("sent"), ping.get("received"), ping.get("rtt_avg_ms")
    if not sent:
        why.append("ping kaydı yok")
    else:
        if received is None or received >= sent:
            why.append("ping kaybı görülmedi")
        if avg is None or avg < RTT_FLOOR_MS:
            why.append(f"ping ort. RTT < {RTT_FLOOR_MS:.0f} ms")
    if not delta.get("dropped"):
        why.append("koşu boyunca qdisc düşürmesi yok")
    return why


def common_clean(rec) -> list[str]:
    why = []
    for name in ("alice", "bob"):
        p = probe(rec, name)
        if not p:
            why.append(f"{name}: sonda yok")
            continue
        for key, label in (("app_retransmit", "app retransmit"),
                           ("crypto_retransmit", "crypto retransmit"),
                           ("ctx_mismatch_drops", "ctx_mismatch_drops"),
                           ("hub_frames_dropped", "hub_frames_dropped"),
                           ("delivery_channel_drops", "delivery_channel_drops")):
            if p.get(key) != 0:
                why.append(f"{name}: {label}={p.get(key)}")
    if rec.get("listener_drops") != 0:
        why.append(f"listener_drops={rec.get('listener_drops')}")
    return why


def flow_complete(rec, key) -> list[str]:
    f, count = flow(rec, key), get(rec, "params", "count")
    why = []
    for field in ("sent", "delivered", "receipts"):
        if f.get(field) != count:
            why.append(f"{key}.{field}={f.get(field)}≠{count}")
    for field in ("duplicate_deliveries", "mismatched_texts", "unknown_deliveries"):
        if f.get(field) != 0:
            why.append(f"{key}.{field}={f.get(field)}")
    return why


def single_session(rec) -> list[str]:
    why = []
    for name in ("alice", "bob"):
        ev, n = get(probe(rec, name), "session_events", default={}) or {}, node(rec, name)
        checks = ((ev.get("ready"), 1, "Ready"), (ev.get("closed"), 0, "Closed"),
                  (ev.get("renew_required"), 0, "RenewRequired"),
                  (probe(rec, name).get("sessions_live"), 1, "canlı oturum"),
                  (get(n, "events", "closed_session"), 0, "Closed olayı"),
                  (get(n, "events", "connect_abandoned"), 0, "ConnectAbandoned"))
        why += [f"{name}: {label}={got}≠{want}" for got, want, label in checks if got != want]
    return why


def epoch_in_flow(rec) -> list[str]:
    f = flow(rec, "bob_to_alice")
    why = []
    if f.get("first_send_epoch") != 0:
        why.append(f"bob ilk gönderim epoch'u={f.get('first_send_epoch')}≠0")
    first_send, epoch1 = f.get("first_send_ms"), get(node(rec, "bob"), "epoch", "first_epoch1_ms")
    if first_send is None or epoch1 is None or epoch1 <= first_send:
        why.append(f"bob epoch-1 ({epoch1}) ilk gönderimden ({first_send}) sonra değil")
    for name in ("alice", "bob"):
        last = get(node(rec, name), "epoch", "last")
        if last != 1:
            why.append(f"{name}: son epoch={last}≠1")
    return why


def connected_once(rec) -> list[str]:
    why = []
    for name in ("alice", "bob"):
        ev = get(node(rec, name), "events", default={}) or {}
        if ev.get("connected") != 1:
            why.append(f"{name}: Connected={ev.get('connected')}≠1")
        if ev.get("closed_session") or ev.get("connect_abandoned"):
            why.append(f"{name}: Closed={ev.get('closed_session')} Abandoned={ev.get('connect_abandoned')}")
    return why


def arq_sum(rec, field):
    total = 0
    for name in ("alice", "bob"):
        value = get(probe(rec, name), "arq_total", field)
        if value is None:
            return None
        total += value
    return total


def verdict(rec, arm) -> tuple[str, list[str]]:
    """('GEÇTİ' | 'KALDI' | 'GEÇERSİZ' | 'KAYIP GÖRÜNDÜ' | 'KAYIP GÖRÜNMEDİ' | 'NON-GATING', gerekçeler)."""
    if rec is None:
        return ("YOK", ["koşu dosyası yok"])
    netem = netem_check(rec)
    if netem:
        return ("GEÇERSİZ", ["netem: " + w for w in netem])
    if arm == "negatif":
        f = flow(rec, "alice_to_bob")
        why = []
        if not (f.get("sent") and f.get("delivered") is not None and f["delivered"] < f["sent"]):
            why.append(f"teslim {f.get('delivered')}/{f.get('sent')} — kayıp görünmedi")
        suppressed, wire = arq_sum(rec, "recovery_suppressed"), arq_sum(rec, "tx_retx_bytes")
        if not suppressed:
            why.append(f"bastırılan kurtarma kararı={suppressed}")
        if wire != 0:
            why.append(f"tele çıkan ARQ baytı={wire}")
        return ("KAYIP GÖRÜNMEDİ", why) if why else ("KAYIP GÖRÜNDÜ", [])
    if arm not in GATING_ARMS:
        return ("NON-GATING", [])
    why = []
    if rec.get("result") != "complete":
        why.append(f"sonuç={rec.get('result')} {rec.get('error') or ''}".strip())
    if arm == "gating1":
        why += flow_complete(rec, "alice_to_bob")
    elif arm == "gating2":
        why += flow_complete(rec, "bob_to_alice") + epoch_in_flow(rec) + single_session(rec)
    else:
        why += flow_complete(rec, "alice_to_bob") + flow_complete(rec, "bob_to_alice")
        why += connected_once(rec)
    why += common_clean(rec)
    return ("KALDI", why) if why else ("GEÇTİ", [])


# ── biçim ────────────────────────────────────────────────────────────────────────────────────

def fmt(value, nd=1):
    if value is None:
        return DASH
    if isinstance(value, bool):
        return "evet" if value else "hayır"
    if isinstance(value, float):
        return f"{value:.{nd}f}"
    return str(value)


def pair(a, b, nd=1):
    return f"{fmt(a, nd)} / {fmt(b, nd)}"


def lat(f, key="latency_ms"):
    s = f.get(key) or {}
    return f"{fmt(s.get('median'), 0)} / {fmt(s.get('p95'), 0)}"


def table(header, rows):
    out = ["| " + " | ".join(header) + " |", "|" + "|".join("---" for _ in header) + "|"]
    out += ["| " + " | ".join(str(c) for c in row) + " |" for row in rows]
    return "\n".join(out)


def netem_cell(rec):
    ping = get(rec, "netem_check", "ping", default={}) or {}
    delta = get(rec, "netem_check", "run_delta", default={}) or {}
    loss = None
    if ping.get("sent"):
        loss = 100.0 * (ping["sent"] - (ping.get("received") or 0)) / ping["sent"]
    return (f"ping {fmt(ping.get('received'))}/{fmt(ping.get('sent'))} "
            f"(%{fmt(loss, 0)} kayıp, ort. {fmt(ping.get('rtt_avg_ms'), 0)} ms) · "
            f"qdisc %{fmt(delta.get('loss_pct'), 1)}")


def arq_cell(rec, field="retransmits"):
    return pair(get(probe(rec, "alice"), "arq_total", field), get(probe(rec, "bob"), "arq_total", field), 0)


def zero_cell(rec, key):
    return pair(probe(rec, "alice").get(key), probe(rec, "bob").get(key), 0)


def epoch_cell(rec):
    return pair(get(node(rec, "alice"), "epoch", "epoch1_after_connected_ms"),
                get(node(rec, "bob"), "epoch", "epoch1_after_connected_ms"), 0)


def window_cell(rec):
    """Pencere süresi (s) ve nasıl bittiği (v1 dosyalarında `params.drain_s`)."""
    seconds = get(rec, "params", "window_s")
    if seconds is None:
        seconds = get(rec, "params", "drain_s")
    return f"{fmt(seconds)} · {fmt(get(rec, 'window', 'ended_by'))}"


def mark(rec, name, which):
    return get(rec, "outage", "session", name, which, default={}) or {}


def transition(rec, field):
    """Kesinti başı → koşu sonu değeri, A / B."""
    return " / ".join(f"{fmt(mark(rec, n, 'start').get(field))}→{fmt(mark(rec, n, 'end').get(field))}"
                      for n in ("alice", "bob"))


def file_since_sent(rec, key):
    o = get(rec, "file", "outcome", default={}) or {}
    a, b = o.get("sent_ms"), o.get(key)
    return None if a is None or b is None else b - a


def outage_criterion(rec):
    """CEVAP PAKETİ 4: 60 s kesinti, AYNI OTURUMDA %100 metin teslimi (PTT ayrı). Ölçüm özeti."""
    if netem_check(rec):
        return "GEÇERSİZ"
    o = rec.get("outage") or {}
    ok = o.get("texts_complete") is True and o.get("same_session") is True
    return "KARŞILANDI" if ok else "KARŞILANMADI"


def fmt_s(ms):
    return DASH if ms is None else f"{ms / 1000.0:.1f} s"


def first_session(rec, name):
    sessions = probe(rec, name).get("sessions") or []
    return sessions[0] if sessions else {}


def stuck_cell(rec, name):
    """Bir düğümün gönderen kurtarma görüntüsü (son sonda) — tek hücre."""
    r = first_session(rec, name).get("recovery") or {}
    if not r:
        return "görüntü yok"
    t, u = r.get("timers") or {}, r.get("oldest_unacked")
    parts = [f"teyitsiz {fmt(r.get('unacked_units'))}"]
    if u:
        parts.append(f"en eski u{u.get('unit_id')}: {fmt(u.get('emission_decisions'))} karar, "
                     f"retx {fmt(u.get('max_retx_count'))}/{fmt(t.get('arq_retx_cap'))}, "
                     f"inf {fmt(u.get('inflight_records'))} tomb {fmt(u.get('tombstone_records'))} "
                     f"pl {fmt(u.get('pending_lost_retx'))}, son gönd. {fmt_s(u.get('since_latest_send_ms'))}")
        if u.get("dup_interval_ms") is not None:
            parts.append(f"kopya vade {fmt_s(u.get('dup_due_in_ms'))} aralık {fmt_s(u.get('dup_interval_ms'))}")
    parts.append(f"son ACK {fmt_s(r.get('since_last_ack_ms'))}")
    parts.append(f"tail {fmt(t.get('tail_probe_index'))}/{fmt(t.get('tail_max_probes'))} "
                 f"PTO {fmt_s(t.get('tail_pto_ms'))}")
    parts.append(f"arq‰ {fmt(t.get('arq_aggressiveness_permille'))}")
    parts.append(str(t.get("state", DASH)))
    return " · ".join(parts)


def reorder_cell(rec, name):
    ro = first_session(rec, name).get("reorder") or {}
    if not ro:
        return DASH
    return f"next {fmt(ro.get('next_unit'))} · tutulan {fmt(ro.get('held_units'))}"


def session_diag(rec, name, field):
    sessions = probe(rec, name).get("sessions") or []
    if len(sessions) != 1:
        return None
    return get(sessions[0], "diag", field)


def render(runs, seeds) -> tuple[str, dict]:
    def rec(seed, arm):
        return runs.get((seed, arm))

    md: list[str] = []
    summary: dict = {"seeds": seeds, "arms": {}}
    for arm in ALL_ARMS:
        verdicts = {s: verdict(rec(s, arm), arm) for s in seeds if rec(s, arm) is not None}
        if verdicts:
            summary["arms"][arm] = {str(s): {"verdict": v, "reasons": r} for s, (v, r) in verdicts.items()}

    # GATING (1)
    rows = []
    for s in seeds:
        r = rec(s, "gating1")
        if r is None:
            continue
        f = flow(r, "alice_to_bob")
        rows.append([s, r.get("result"), fmt(f.get("delivered_pct")), fmt(f.get("receipt_pct")),
                     lat(f), lat(f, "receipt_rtt_ms"), arq_cell(r), arq_cell(r, "tail_probes"),
                     zero_cell(r, "app_retransmit"), zero_cell(r, "crypto_retransmit"),
                     zero_cell(r, "ctx_mismatch_drops"), zero_cell(r, "hub_frames_dropped"),
                     zero_cell(r, "delivery_channel_drops"), zero_cell(r, "not_ready_drops"),
                     epoch_cell(r), netem_cell(r), verdict(r, "gating1")[0]])
    if rows:
        md += ["### GATING (1) — alice → bob 100 TEXT + 100 RECEIPT",
               table(["tohum", "sonuç", "teslim %", "alındı %", "gecikme medyan / 95p (ms)",
                      "alındı RTT medyan / 95p (ms)", "ARQ retx A / B", "tail-probe A / B",
                      "app retx A / B", "crypto retx A / B", "ctx A / B", "hub A / B", "dcd A / B",
                      "not_ready A / B", "epoch-1 (Connected'dan, ms) A / B", "netem doğrulama",
                      "HÜKÜM"], rows), ""]

    # GATING (2)
    rows, karn = [], []
    for s in seeds:
        r = rec(s, "gating2")
        if r is None:
            continue
        f = flow(r, "bob_to_alice")
        e1, fs = get(node(r, "bob"), "epoch", "first_epoch1_ms"), f.get("first_send_ms")
        gap = (e1 - fs) if (e1 is not None and fs is not None) else None
        ev = [get(probe(r, n), "session_events", "ready") for n in ("alice", "bob")]
        rows.append([s, r.get("result"), window_cell(r), fmt(f.get("delivered_pct")), fmt(f.get("receipt_pct")), lat(f),
                     fmt(f.get("first_send_epoch")), fmt(gap, 0), fmt(f.get("sends_at_epoch0")),
                     pair(get(node(r, "alice"), "epoch", "last"), get(node(r, "bob"), "epoch", "last"), 0),
                     pair(ev[0], ev[1], 0),
                     pair(probe(r, "alice").get("sessions_live"), probe(r, "bob").get("sessions_live"), 0),
                     arq_cell(r), zero_cell(r, "app_retransmit"), zero_cell(r, "crypto_retransmit"),
                     zero_cell(r, "ctx_mismatch_drops"), netem_cell(r), verdict(r, "gating2")[0]])
        karn.append([s, zero_cell(r, "not_ready_drops"),
                     pair(session_diag(r, "alice", "rtt_samples"), session_diag(r, "bob", "rtt_samples"), 0),
                     pair(session_diag(r, "alice", "left_preconverge_ms"),
                          session_diag(r, "bob", "left_preconverge_ms"), 0),
                     pair(session_diag(r, "alice", "srtt_ms"), session_diag(r, "bob", "srtt_ms"), 0),
                     pair(session_diag(r, "alice", "min_rtt_ms"), session_diag(r, "bob", "min_rtt_ms"), 0),
                     fmt(get(r, "netem_check", "ping", "rtt_avg_ms"), 0)])
    if rows:
        g2 = [verdict(rec(s, "gating2"), "gating2")[0] for s in seeds if rec(s, "gating2") is not None]
        windows = sorted({str(get(rec(s, "gating2"), "params", "window_s")) for s in seeds
                          if rec(s, "gating2") is not None})
        summary["gating2"] = {"passed": sum(v == "GEÇTİ" for v in g2), "run": len(g2), "windows_s": windows}
        md += ["### GATING (2) — bob (yanıtlayan) → alice; epoch 0→1 akış içinde, tek oturum",
               f"**GATING (2): {summary['gating2']['passed']}/{len(g2)} tohum GEÇTİ** · pencere "
               f"{', '.join(windows)} s (ölçüm tablosu; kapı ifadesi değildir).",
               table(["tohum", "sonuç", "pencere (s) · sonu", "teslim %", "alındı %", "gecikme medyan / 95p (ms)",
                      "bob ilk gönderim epoch'u", "bob epoch-1 − ilk gönderim (ms)",
                      "epoch-0'da gönderim", "son epoch A / B", "Ready A / B", "canlı oturum A / B",
                      "ARQ retx A / B", "app retx A / B", "crypto retx A / B", "ctx A / B",
                      "netem doğrulama", "HÜKÜM"], rows), "",
               "#### DUR-K4G-01 (iii) — NotReady düşüşleri ve Karn / PreConverge (GATING (2))",
               table(["tohum", "not_ready_drops A / B", "RTT örneği A / B",
                      "PreConverge çıkışı (uç açılışından, ms) A / B", "sRTT (ms) A / B",
                      "min RTT (ms) A / B", "ping ort. RTT (ms)"], karn), ""]

    # GATING (6)
    rows = []
    for s in seeds:
        r = rec(s, "gating6")
        if r is None:
            continue
        ab, ba = flow(r, "alice_to_bob"), flow(r, "bob_to_alice")
        conn = [get(node(r, n), "events", "connected") for n in ("alice", "bob")]
        closed = [(get(node(r, n), "events", "closed_session") or 0) + (get(node(r, n), "events", "connect_abandoned") or 0)
                  for n in ("alice", "bob")]
        crypto = [get(probe(r, n), "crypto", default={}) or {} for n in ("alice", "bob")]
        rows.append([s, r.get("result"), pair(conn[0], conn[1], 0), pair(closed[0], closed[1], 0),
                     f"{fmt(ab.get('delivered'))}/{fmt(ab.get('sent'))} · {fmt(ab.get('receipts'))}",
                     f"{fmt(ba.get('delivered'))}/{fmt(ba.get('sent'))} · {fmt(ba.get('receipts'))}",
                     pair(ab.get("duplicate_deliveries"), ba.get("duplicate_deliveries"), 0),
                     pair(crypto[0].get("inits_sent"), crypto[1].get("inits_sent"), 0),
                     pair(crypto[0].get("responses_sent"), crypto[1].get("responses_sent"), 0),
                     pair(get(probe(r, "alice"), "session_events", "ready"),
                          get(probe(r, "bob"), "session_events", "ready"), 0),
                     zero_cell(r, "app_retransmit"), zero_cell(r, "crypto_retransmit"),
                     zero_cell(r, "ctx_mismatch_drops"), netem_cell(r), verdict(r, "gating6")[0]])
    if rows:
        md += ["### GATING (6) — eşzamanlı connect (glare), iki yönde TEXT + RECEIPT",
               table(["tohum", "sonuç", "Connected A / B", "Closed+Abandoned A / B",
                      "A→B teslim/gönderilen · alındı", "B→A teslim/gönderilen · alındı",
                      "tekrar A→B / B→A", "INIT A / B", "RESPONSE A / B", "Ready A / B",
                      "app retx A / B", "crypto retx A / B", "ctx A / B", "netem doğrulama",
                      "HÜKÜM"], rows), ""]

    # T4
    t4_rows, t4_pass = [], True
    for arm in GATING_ARMS:
        vs = [verdict(rec(s, arm), arm)[0] for s in seeds if rec(s, arm) is not None]
        passed = sum(v == "GEÇTİ" for v in vs)
        ok = len(vs) >= T4_MIN_SEEDS and passed == len(vs)
        t4_pass &= ok
        t4_rows.append([arm, f"{passed}/{len(vs)}", "GEÇTİ" if ok else "KALDI"])
    summary["t4"] = {"pass": t4_pass, "rows": t4_rows}
    md += [f"### T4 — sabit tohum, en az {T4_MIN_SEEDS} tohum, hepsi GEÇTİ",
           table(["kol", "geçen / koşulan", "T4"], t4_rows), "",
           f"**T4 (ölçüm tablosu): {'GEÇTİ' if t4_pass else 'KALDI'}** — kapı ifadesi değildir.", ""]

    # NEGATİF
    rows = []
    for s in seeds:
        r = rec(s, "negatif")
        if r is None:
            continue
        f = flow(r, "alice_to_bob")
        rows.append([s, r.get("result"), f"{fmt(f.get('delivered'))}/{fmt(f.get('sent'))}",
                     fmt(f.get("receipt_pct")), arq_cell(r, "recovery_suppressed"),
                     arq_cell(r, "retransmits"), arq_cell(r, "tx_retx_bytes"),
                     netem_cell(r), verdict(r, "negatif")[0]])
    if rows:
        md += ["### Negatif kol — recovery OFF (ölçümün kaybı görebildiğinin kanıtı)",
               table(["tohum", "sonuç", "teslim / gönderilen", "alındı %",
                      "bastırılan kurtarma kararı A / B", "motor retx kararı A / B",
                      "tele çıkan retx baytı A / B", "netem doğrulama", "HÜKÜM"], rows), ""]

    # NON-GATING: dosya + PTT
    rows = []
    for s in seeds:
        r = rec(s, "ek-dosya-ptt")
        if r is None:
            continue
        f, fl, pt = flow(r, "alice_to_bob"), r.get("file") or {}, r.get("ptt") or {}
        oc = fl.get("outcome") or {}
        rows.append([s, r.get("result"), window_cell(r), f"{fmt(f.get('delivered'))}/{fmt(f.get('sent'))}", lat(f),
                     fmt(fl.get("bytes")), fmt(oc.get("status")), fmt(oc.get("reason")),
                     fmt(file_since_sent(r, "at_ms"), 0),
                     fmt(fl.get("send_to_received_ms"), 0),
                     fmt(fl.get("send_to_receipt_ms"), 0), fmt(file_since_sent(r, "sender_failed_ms"), 0),
                     fmt(oc.get("receiver_failed_reason")), fmt(fl.get("bytes_match")),
                     f"{fmt(pt.get('received'))}/{fmt(pt.get('pushed'))}", fmt(pt.get("in_order_complete")),
                     fmt(pt.get("press_to_first_audio_ms"), 0), netem_cell(r)])
        summary.setdefault("ek_dosya_ptt", {})[str(s)] = {
            "status": oc.get("status"), "reason": oc.get("reason"),
            "texts_delivered": f.get("delivered"), "texts_sent": f.get("sent"),
            "ptt_in_order_complete": pt.get("in_order_complete"), "window_ended_by": get(r, "window", "ended_by")}
    if rows:
        md += ["### NON-GATING — 100 TEXT + 1 dosya + 1 PTT klibi (aynı profil)",
               "Dosya sonucu ayrı alandadır (`file.outcome`); kol hükmü değişmedi: başarısız dosya kolu "
               "tamamlanmamış sayar. Metin ve PTT tam pencere boyunca ölçülür.",
               table(["tohum", "sonuç", "pencere (s) · sonu", "TEXT teslim", "gecikme medyan / 95p (ms)",
                      "dosya bayt", "dosya durumu", "neden", "gönderim→sonuç (ms)",
                      "gönderim→alındı (ms)", "gönderim→FILE_RECEIPT (ms)",
                      "gönderim→gönderen başarısızlığı (ms)", "alıcı başarısızlığı", "bayt-özdeş",
                      "PTT demet alınan/verilen", "sıralı+tam", "bas→ilk ses (ms)",
                      "netem doğrulama"], rows), ""]

    # NON-GATING: 60 s kesinti
    rows, ptt_rows = [], []
    for s in seeds:
        r = rec(s, "ek-kesinti")
        if r is None:
            continue
        f, o, pt = flow(r, "alice_to_bob"), r.get("outage") or {}, r.get("ptt") or {}
        after = o.get("delivery_after_restore_ms") or {}
        rows.append([s, r.get("result"), window_cell(r), f"{fmt(f.get('delivered'))}/{fmt(f.get('sent'))}",
                     fmt(o.get("texts_complete")), fmt(o.get("duration_ms"), 0),
                     f"{fmt(o.get('texts_delivered_of_during'))}/{fmt(o.get('texts_sent_during'))}",
                     f"{fmt(after.get('median'), 0)} / {fmt(after.get('p95'), 0)} / {fmt(after.get('max'), 0)}",
                     fmt(o.get("same_session")), fmt(o.get("same_session_outage_only")),
                     fmt(o.get("same_session_handles")), transition(r, "epoch"),
                     transition(r, "connected_events"), transition(r, "closed_events"),
                     arq_cell(r), netem_cell(r), outage_criterion(r)])
        ptt_rows.append([s, f"{fmt(pt.get('received'))}/{fmt(pt.get('pushed'))}",
                         fmt(pt.get("in_order_complete")), fmt(pt.get("peer_opened_events")),
                         fmt(pt.get("peer_closed_events")), fmt(pt.get("press_to_first_audio_ms"), 0),
                         pair(get(probe(r, "alice"), "session_events", "ready"),
                              get(probe(r, "bob"), "session_events", "ready"), 0)])
        summary.setdefault("ek_kesinti", {})[str(s)] = {
            "texts_complete": o.get("texts_complete"), "same_session": o.get("same_session"),
            "same_session_outage_only": o.get("same_session_outage_only"),
            "same_session_handles": o.get("same_session_handles"),
            "ptt_in_order_complete": pt.get("in_order_complete"), "criterion": outage_criterion(r)}
    if rows:
        met = sum(v["criterion"] == "KARŞILANDI" for v in summary["ek_kesinti"].values())
        md += ["### NON-GATING — akış ortasında 60 s tam kesinti (`loss 100%`)",
               f"**Sahip ölçütü (CEVAP PAKETİ 4 — 60 s kesinti, aynı oturumda %100 metin teslimi): "
               f"{met}/{len(rows)} tohum** (metin %100 ∧ same_session; PTT ayrı tabloda, ölçüte girmez; "
               "ölçüm özeti, kapı ifadesi değildir).",
               "#### Metin ve aynı oturum",
               "`same_session` = kesinti başından koşu sonuna iki düğümde yeni `Closed` yok, ikinci "
               "`Connected` yok, epoch değişmemiş; 'yalnız kesinti' = aynı ölçüt kesinti bitişine dek; "
               "'tutamaç + S1' = oturum tutamaçları ve S1 Ready/Closed sayıları da aynı.",
               table(["tohum", "sonuç", "pencere (s) · sonu", "metin teslim/gönderilen", "metin %100",
                      "kesinti (ms)", "kesintide gönderilenden teslim",
                      "kesinti bitişinden teslim medyan / 95p / max (ms)", "same_session",
                      "yalnız kesinti", "tutamaç + S1", "epoch başı→sonu A / B",
                      "Connected başı→sonu A / B", "Closed başı→sonu A / B", "ARQ retx A / B",
                      "netem doğrulama", "sahip ölçütü"], rows), "",
               "#### PTT (ayrı)",
               table(["tohum", "demet alınan/verilen", "sıralı+tam", "PttOpened olayı",
                      "PttClosed olayı", "bas→ilk ses (ms)", "Ready A / B"], ptt_rows), ""]

    # TAKILI BİRİM ÖZETİ (teşhis; hüküm değil) — koşu başına tek satır
    rows = []
    for arm in ALL_ARMS:
        for s in seeds:
            r = rec(s, arm)
            if r is None:
                continue
            samples = get(r, "timeline", "samples", default=[]) or []
            rows.append([s, arm, r.get("result"),
                         pair(probe(r, "alice").get("outstanding_units"), probe(r, "bob").get("outstanding_units"), 0),
                         stuck_cell(r, "alice"), stuck_cell(r, "bob"),
                         reorder_cell(r, "alice"), reorder_cell(r, "bob"),
                         f"{len(samples)} (düşen {fmt(get(r, 'timeline', 'dropped'))})"])
            summary.setdefault("stuck", []).append({
                "seed": s, "arm": arm, "result": r.get("result"),
                "alice": first_session(r, "alice").get("recovery"),
                "bob": first_session(r, "bob").get("recovery"),
                "reorder": {"alice": first_session(r, "alice").get("reorder"),
                            "bob": first_session(r, "bob").get("reorder")}})
    if rows:
        md += ["### Takılı birim özeti (K4G-05 teşhis; hüküm değil — son sonda)",
               "A = alice, B = bob. Gönderen hücresi o düğümün GÖNDERDİĞİ yöndür; `reorder` o düğümün "
               "ALDIĞI yönde sıralı teslimin beklediği birim ve arkasında tutulanlardır.",
               table(["tohum", "kol", "sonuç", "teyitsiz birim A / B", "gönderen A", "gönderen B",
                      "alıcı reorder A", "alıcı reorder B", "zaman çizelgesi örneği"], rows), ""]

    # gerekçeler
    reasons = []
    for arm, per_seed in summary["arms"].items():
        for s, v in per_seed.items():
            if v["reasons"]:
                reasons.append(f"- `{arm}` tohum {s}: {v['verdict']} — " + "; ".join(v["reasons"]))
    if reasons:
        md += ["### Hüküm gerekçeleri (GEÇTİ olmayan koşular)", *reasons, ""]
    return "\n".join(md), summary


# ── öz-test ──────────────────────────────────────────────────────────────────────────────────

def _synthetic(arm: str, seed: int, *, lossy_negative=True, broken=False) -> dict:
    count = 100
    ping = {"sent": 20, "received": 9, "rtt_avg_ms": 401.0}
    probe_ = {"app_retransmit": 0, "crypto_retransmit": 0, "ctx_mismatch_drops": 0,
              "hub_frames_dropped": 0, "delivery_channel_drops": 0, "not_ready_drops": 0,
              "sessions_live": 1, "session_events": {"ready": 1, "closed": 0, "renew_required": 0},
              "arq_total": {"retransmits": 40, "tail_probes": 3, "recovery_suppressed": 0,
                            "tx_retx_bytes": 4000},
              "crypto": {"inits_sent": 1, "responses_sent": 0},
              "outstanding_units": 0,
              "sessions": [{"diag": {"rtt_samples": 90, "srtt_ms": 410.0, "min_rtt_ms": 20.0,
                                     "left_preconverge_ms": 800.0},
                            "recovery": {"unacked_units": 1, "since_last_ack_ms": 400.0,
                                         "timers": {"state": "RecoverFwd", "arq_retx_cap": 3,
                                                    "tail_probe_index": 3, "tail_max_probes": 3,
                                                    "tail_pto_ms": 30000.0, "arq_aggressiveness_permille": 500},
                                         "oldest_unacked": {"unit_id": 17, "emission_decisions": 7,
                                                            "max_retx_count": 3, "inflight_records": 0,
                                                            "tombstone_records": 2, "pending_lost_retx": None,
                                                            "since_latest_send_ms": 12000.0,
                                                            "dup_due_in_ms": 41000.0, "dup_interval_ms": 51200.0}},
                            "reorder": {"next_unit": 17, "held_units": 31, "first_held_unit": 18}}]}
    full = {"sent": count, "delivered": count, "receipts": count, "duplicate_deliveries": 0,
            "mismatched_texts": 0, "unknown_deliveries": 0, "delivered_pct": 100.0, "receipt_pct": 100.0,
            "first_send_epoch": 0, "first_send_ms": 50.0,
            "latency_ms": {"median": 420.0, "p95": 1900.0}, "receipt_rtt_ms": {"median": 900.0, "p95": 3100.0}}
    empty = {"sent": 0, "delivered": 0, "receipts": 0, "duplicate_deliveries": 0, "mismatched_texts": 0,
             "unknown_deliveries": 0}
    rec = {"arm": arm, "seed": seed, "result": "complete", "params": {"count": count}, "listener_drops": 0,
           "netem_check": {"ping": ping, "run_delta": {"dropped": 900, "loss_pct": 30.1}},
           "flows": {"alice_to_bob": dict(full), "bob_to_alice": dict(empty)},
           "nodes": {n: {"events": {"connected": 1, "closed_session": 0, "connect_abandoned": 0},
                         "epoch": {"last": 1, "first_epoch1_ms": 300.0, "epoch1_after_connected_ms": 250.0},
                         "probe": json.loads(json.dumps(probe_))} for n in ("alice", "bob")}}
    if arm in ("gating2", "gating6"):
        rec["flows"]["bob_to_alice"] = dict(full)
    if arm == "gating2":
        rec["flows"]["alice_to_bob"] = dict(empty)
    if arm == "negatif":
        rec["result"] = "incomplete"
        rec["flows"]["alice_to_bob"].update(delivered=3 if lossy_negative else count, receipts=2)
        for n in ("alice", "bob"):
            rec["nodes"][n]["probe"]["arq_total"].update(recovery_suppressed=50, tx_retx_bytes=0)
    if broken:
        rec["nodes"]["bob"]["probe"]["crypto_retransmit"] = 1
    # [CEVAP PAKETİ 4] v2 alanları.
    rec["params"]["window_s"] = 600
    rec["window"] = {"s": 600, "anchor": "outage_end" if arm == "ek-kesinti" else "last_send",
                     "ended_by": "complete" if rec["result"] == "complete" else "deadline"}
    if arm == "ek-kesinti":
        mark = {"epoch": 1, "connected_events": 1, "closed_events": 0, "s1_ready": 1, "s1_closed": 0,
                "session_ids": ["SessionHandle(3)"]}
        rec["outage"] = {"duration_ms": 60010.0, "texts_sent_during": 50, "texts_delivered_of_during": 50,
                         "delivery_after_restore_ms": {"median": 9000.0, "p95": 31000.0, "max": 40000.0},
                         "texts_sent": count, "texts_delivered": count, "texts_complete": True,
                         "session": {n: {"start": dict(mark), "restore": dict(mark), "end": dict(mark)}
                                     for n in ("alice", "bob")},
                         "same_session": True, "same_session_outage_only": True, "same_session_handles": True}
        rec["ptt"] = {"received": 50, "pushed": 50, "in_order_complete": True, "peer_opened_events": 1,
                      "peer_closed_events": 1, "press_to_first_audio_ms": 900.0}
    if arm == "ek-dosya-ptt":
        rec["file"] = {"bytes": 153600, "send_to_received_ms": 21000.0, "send_to_receipt_ms": 22000.0,
                       "bytes_match": True,
                       "outcome": {"status": "delivered", "reason": None, "sent_ms": 11000.0,
                                   "at_ms": 33000.0, "sender_failed_ms": None, "receiver_failed_reason": None}}
        rec["ptt"] = {"received": 50, "pushed": 50, "in_order_complete": True, "press_to_first_audio_ms": 700.0}
    return rec


def selftest() -> int:
    cases = [
        ("gating1 temiz → GEÇTİ", _synthetic("gating1", 1), "gating1", "GEÇTİ"),
        ("gating2 temiz → GEÇTİ", _synthetic("gating2", 1), "gating2", "GEÇTİ"),
        ("gating6 temiz → GEÇTİ", _synthetic("gating6", 1), "gating6", "GEÇTİ"),
        ("crypto retransmit → KALDI", _synthetic("gating1", 1, broken=True), "gating1", "KALDI"),
        ("negatif kayıplı → KAYIP GÖRÜNDÜ", _synthetic("negatif", 1), "negatif", "KAYIP GÖRÜNDÜ"),
        ("negatif kayıpsız → KAYIP GÖRÜNMEDİ", _synthetic("negatif", 1, lossy_negative=False), "negatif",
         "KAYIP GÖRÜNMEDİ"),
    ]
    no_netem = _synthetic("gating1", 1)
    no_netem["netem_check"]["ping"].update(received=20, rtt_avg_ms=0.05)
    cases.append(("netem ateşlemedi → GEÇERSİZ", no_netem, "gating1", "GEÇERSİZ"))
    epoch_before = _synthetic("gating2", 1)
    epoch_before["flows"]["bob_to_alice"]["first_send_epoch"] = 1
    cases.append(("gating2 ilk gönderim epoch-1 → KALDI", epoch_before, "gating2", "KALDI"))
    two_sessions = _synthetic("gating2", 1)
    two_sessions["nodes"]["alice"]["probe"]["session_events"]["ready"] = 2
    cases.append(("gating2 iki oturum → KALDI", two_sessions, "gating2", "KALDI"))
    dup = _synthetic("gating6", 1)
    dup["flows"]["bob_to_alice"]["duplicate_deliveries"] = 1
    cases.append(("gating6 çift teslim → KALDI", dup, "gating6", "KALDI"))
    failed = 0
    for name, rec, arm, want in cases:
        got = verdict(rec, arm)[0]
        ok = got == want
        failed += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  {name}: {got}")
    with tempfile.TemporaryDirectory() as tmp:
        d = Path(tmp)
        for seed in range(1, 6):
            for arm in ("gating1", "gating2", "gating6", "negatif"):
                (d / f"seed-{seed}-{arm}.json").write_text(json.dumps(_synthetic(arm, seed)), encoding="utf-8")
        md, summary = render(load(d), list(range(1, 6)))
        ok = (summary["t4"]["pass"] and "### T4" in md and "DUR-K4G-01 (iii)" in md
              and "### Takılı birim özeti" in md and "en eski u17: 7 karar, retx 3/3" in md
              and "next 17 · tutulan 31" in md)
        failed += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  5 tohum × 4 kol sentetik → T4 GEÇTİ ve tablolar")
    # [CEVAP PAKETİ 4] Yeniden koşu biçimi: 10 tohum GATING (2) pencere 600 s; ek-kesinti 5 tohum (biri
    # oturum değiştirmiş); ek-dosya-ptt 5 tohum (biri gönderende başarısız); v1 `drain_s` uyumu.
    with tempfile.TemporaryDirectory() as tmp:
        d = Path(tmp)
        for seed in range(1, 11):
            (d / f"seed-{seed}-gating2.json").write_text(json.dumps(_synthetic("gating2", seed)), encoding="utf-8")
        for seed in range(1, 6):
            k = _synthetic("ek-kesinti", seed)
            if seed == 3:
                k["outage"]["same_session"] = False
                k["outage"]["session"]["bob"]["end"]["connected_events"] = 2
            (d / f"seed-{seed}-ek-kesinti.json").write_text(json.dumps(k), encoding="utf-8")
            f = _synthetic("ek-dosya-ptt", seed)
            if seed == 2:
                f["result"] = "incomplete"
                f["window"]["ended_by"] = "settled"
                f["file"]["outcome"].update(status="failed", reason="Outgoing(ReceiptTimeout)",
                                            sender_failed_ms=91000.0, at_ms=91000.0)
            (d / f"seed-{seed}-ek-dosya-ptt.json").write_text(json.dumps(f), encoding="utf-8")
        md, summary = render(load(d), list(range(1, 11)))
        old = _synthetic("gating1", 1)
        old["params"] = {"count": 100, "drain_s": 120}
        ok = ("**GATING (2): 10/10 tohum GEÇTİ** · pencere 600 s" in md
              and "60 s kesinti, aynı oturumda %100 metin teslimi): 4/5 tohum**" in md
              and "#### PTT (ayrı)" in md and "1→2" in md
              and "| failed | Outgoing(ReceiptTimeout) |" in md
              and summary["ek_dosya_ptt"]["2"]["status"] == "failed"
              and summary["ek_kesinti"]["3"]["criterion"] == "KARŞILANMADI"
              and window_cell(old).startswith("120 · "))
        failed += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  yeniden koşu biçimi (10×GATING (2), ek-kesinti, ek-dosya-ptt, v1 uyumu)")
    print("int02_analyze öz-test:", "YEŞİL" if not failed else f"KIRMIZI ({failed})")
    return 0 if not failed else 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("directory", nargs="?", type=Path)
    ap.add_argument("--md", type=Path, help="Markdown çıktısı (varsayılan: stdout)")
    ap.add_argument("--json", type=Path, help="hüküm özeti (JSON)")
    ap.add_argument("--selftest", action="store_true")
    args = ap.parse_args()
    if args.selftest:
        return selftest()
    if args.directory is None or not args.directory.is_dir():
        ap.error("çıktı dizini gerekli (int02_run.sh --out)")
    runs = load(args.directory)
    if not runs:
        print(f"{args.directory}: seed-<S>-<kol>.json yok", file=sys.stderr)
        return 2
    seeds = sorted({s for s, _ in runs})
    md, summary = render(runs, seeds)
    header = (f"<!-- int02_analyze.py · kaynak: {args.directory} · tohumlar: {' '.join(map(str, seeds))} -->\n")
    if args.md:
        args.md.write_text(header + md + "\n", encoding="utf-8")
    else:
        print(header + md)
    if args.json:
        args.json.write_text(json.dumps(summary, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
