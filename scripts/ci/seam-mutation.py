#!/usr/bin/env python3
"""KM-SEAM-AT-001 §6 davranis satirlarinin MUTASYON turu — sahteye karsi, AGAC DISI.

    scripts/ci/seam-mutation.py                 # butun mutasyonlar
    scripts/ci/seam-mutation.py --list          # mutasyon listesini bas, kosma
    scripts/ci/seam-mutation.py --only ID [ID…] # secili mutasyonlar
    scripts/ci/seam-mutation.py --online        # cargo'yu --offline'siz kostur (kayit onbellegi yoksa)
    scripts/ci/seam-mutation.py --keep          # gecici calisma agacini silme (teshis)

NEDEN VAR. Bir sozlesme satirinin sahtede YESIL olmasi, satirin bir sey OLCTUGUNU gostermez: govde
hicbir seyi sinamiyor da olabilir. Bu betik sahtenin (`crates/kama-app/tests/common/mock_transport.rs`)
davranisini tek tek bozar ve her bozulmanin HEDEF satiri kirmizi yaptigini olcer. Olcum sahteyle
sinirlidir: satirin gercek hedefteki gucu hakkinda bir sey soylemez (GAP-SEAM-REAL-1).

NASIL. Depoya DOKUNULMAZ (git ile geri alma yok): gecici bir dizinde minik bir workspace kurulur —
`kama-contracts` kaynagi + harness dosyalari (`common/seam/*.rs`, `seam_conformance_core/main.rs`) +
sahtenin kopyasi. Her mutasyon sahtenin KOPYASINA metin degisimi olarak uygulanir (capa tam olarak
bir kez bulunmali), `seam_conformance_core` kosturulur, kirmizi olan `mock_contract::h_*` testleri
okunur.

KAPI DEGILDIR: CI'a bagli degildir (her mutasyon bir derleme; ~40 derleme). Elle/istege bagli kosulur.

OLCUT VE SINIRI. Bir mutasyon, HEDEF satiri kirmizi olunca "oldu" sayilir; satirin HANGI iddiasinin
kirmizi oldugu ayristirilmaz ve baska satirlarin da kirmizi olmasi engel degildir (her mutasyonun
kirmizi satir listesi basilir; butun satirlar birden kirmiziysa isaretlenir). Derlenmeyen ya da
RUN_TIMEOUT_S icinde bitmeyen mutasyon "oldu" SAYILMAZ: ortam hatasidir.

Cikis kodu: 0 = her mutasyon hedef satirinda kirmizi VE her davranis satirinin en az bir mutasyonu var
            1 = yasayan mutasyon ya da mutasyonsuz satir
            2 = ortam/capa hatasi (taban kirmizi ya da CONTRACT_ROWS ile ayrisiyor, derlenmedi, zaman
                asimi, capa bulunamadi)
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
# Tek bir koşumun azami süresi: döngüye giren bir mutant aracı asılı bırakmasın.
RUN_TIMEOUT_S = 900
MOCK_REL = "crates/kama-app/tests/common/mock_transport.rs"
SEAM_REL = "crates/kama-integration/tests/common/seam"
CORE_REL = "crates/kama-integration/tests/seam_conformance_core/main.rs"
CONTRACTS_REL = "crates/kama-contracts"
HARNESS_FILES = ("harness.rs", "target.rs", "contract.rs")

# ── capalar: sahtenin kaynagindan AYNEN alinmis parcalar ─────────────────────────────────────

SEND_DELIVERED = """        let delivered = Delivered {
            session,
            unit: unit.bytes,
        };"""
ACTIVE_CONFLICT = """        if let Some(&handle) = self.active_by_peer.get(&peer) {
            if early_unit.is_some() {
                return Err(TransportError::EarlyUnitConflict);
            }"""
PENDING_BRANCH = """        if let Some(&ticket) = self.pending_by_peer.get(&peer) {
            if early_unit.is_some() {
                return Err(TransportError::EarlyUnitConflict);
            }
            return Ok(ConnectResult::Pending(ticket));
        }"""
SEND_CAP = """        if unit.bytes.len() > self.capabilities.max_plaintext_unit_bytes as usize {
            return Err(TransportError::UnitTooLarge);
        }
        if self.inbox.len() + self.in_recovery.len() >= MAX_INBOX {"""
CANCEL_TAIL = """        self.pending_by_peer.remove(&peer);
        self.pending_early.remove(&ticket);
        self.push_event(SessionEvent::ConnectAbandoned(
            ticket,
            AbandonReason::Cancelled,
        ));"""
ADVANCE_HEAD = """    pub fn advance_clock(&mut self, by: Duration) {
        if !self.link_up {
            return;
        }"""
SETLINK_PHASE = """        let phase = if up {
            self.stats.probes += 1;
            self.since_probe = Duration::ZERO;
            ConnectPhase::Measuring
        } else {
            ConnectPhase::Suspended
        };
        let tickets: Vec<ConnectTicket> = self.pending_by_peer.values().copied().collect();
        for ticket in tickets {
            self.push_event(SessionEvent::ConnectProgress(ticket, phase));
        }"""
INVALID = """    pub fn inject_invalid_response(&mut self, _peer: &PeerRef) {
        self.stats.invalid_responses += 1;
    }"""
DROP_BRANCH = """            if self.recovery {
                self.in_recovery.push_back(Held {
                    delivered,
                    lost: true,
                });
            }
            return Ok(());"""
BEHIND = """        if self
            .in_recovery
            .iter()
            .any(|held| held.delivered.session == session)
        {"""
PUMP_PUSH = """            self.inbox.push_back(held.delivered);"""
RENEW_EVENTS = """        self.push_event(SessionEvent::Ready(new_handle, peer));
        self.push_event(SessionEvent::Closed(old, CloseReason::Superseded));"""
POLL_FORGET = """        if let SessionEvent::Closed(h, _) = ev {
            self.forget(h);
        }
        Some(ev)"""
SUPERSEDED_CHECK = """        if self.superseded.contains(&session) {
            return Err(TransportError::SessionSuperseded);
        }"""
REQ_RENEW = """        self.push_event(SessionEvent::RenewRequired(session));"""
EARLY_DELIVER = """            let delivered = Delivered {
                session: handle,
                unit: early_bytes,
            };"""
PENDING_CAP = """        if self.pending_by_peer.len() >= PENDING_INIT_CAP"""

M = [
 # (kimlik, hedef satir (test adi), aciklama, eski, yeni)
 ("roundtrip-truncate", "h_roundtrip", "send: son bayt kirpilir", SEND_DELIVERED,
  SEND_DELIVERED.replace("unit: unit.bytes,", "unit: unit.bytes[..unit.bytes.len().saturating_sub(1)].to_vec(),")),
 ("roundtrip-early-corrupt", "h_roundtrip", "early_unit baytlari ters cevrilir", EARLY_DELIVER,
  EARLY_DELIVER.replace("unit: early_bytes,", "unit: early_bytes.into_iter().rev().collect(),")),
 ("roundtrip-duplicate", "h_roundtrip", "send: birim iki kez teslim edilir", "            self.inbox.push_back(delivered);\n        }\n        Ok(())",
  "            self.inbox.push_back(delivered.clone());\n            self.inbox.push_back(delivered);\n        }\n        Ok(())"),
 ("events-order-swapped", "h_events", "cutover: Closed, Ready(h')'den ONCE", RENEW_EVENTS,
  "        self.push_event(SessionEvent::Closed(old, CloseReason::Superseded));\n        self.push_event(SessionEvent::Ready(new_handle, peer));"),
 ("events-no-renew-required", "h_events", "RenewRequired hic cikmaz", REQ_RENEW, "        let _ = session;"),
 ("events-wrong-close-reason", "h_events", "Closed(h, UserClosed)", RENEW_EVENTS,
  RENEW_EVENTS.replace("CloseReason::Superseded", "CloseReason::UserClosed")),
 ("events-closed-not-forgotten", "h_events", "Closed okunduktan sonra tutamac unutulmaz (SessionSuperseded donmeye devam eder)", POLL_FORGET,
  "        Some(ev)"),
 ("events-same-handle", "h_events", "yenileme ayni tutamaci verir", "        let new_handle = SessionHandle::from_raw(self.fresh_id());\n        self.active_by_peer.insert(peer, new_handle);",
  "        let new_handle = old;\n        self.active_by_peer.insert(peer, new_handle);"),
 ("cutover-old-still-accepts", "h_cutover", "cutover sonrasi send(h) kabul edilir", SUPERSEDED_CHECK, ""),
 ("cutover-outstanding-dropped", "h_cutover", "cutover'da h'nin outstanding'i atilir", RENEW_EVENTS,
  "        self.inbox.retain(|d| d.session != old);\n" + RENEW_EVENTS),
 ("cutover-in-recovery-dropped", "h_cutover", "cutover'da h'nin YOLDAKI (kurtarilmayi bekleyen) birimi atilir", RENEW_EVENTS,
  "        self.in_recovery.retain(|held| held.delivered.session != old);\n" + RENEW_EVENTS),
 ("cutover-old-rejected-early", "h_cutover", "yenileme SURERKEN send(h) reddedilir", REQ_RENEW,
  REQ_RENEW + "\n        self.superseded.insert(session);"),
 ("idemp-pending-new-ticket", "h_connect_idemp", "pending ese ikinci connect YENI ticket verir", PENDING_BRANCH,
  PENDING_BRANCH.replace("return Ok(ConnectResult::Pending(ticket));",
   "let _ = ticket;\n            let fresh = ConnectTicket::from_raw(self.fresh_id());\n            self.pending_by_peer.insert(peer, fresh);\n            return Ok(ConnectResult::Pending(fresh));")),
 ("idemp-duplicate-ready", "h_connect_idemp", "aktif ese connect duplicate Ready olayi uretir", ACTIVE_CONFLICT,
  ACTIVE_CONFLICT + "\n            self.push_event(SessionEvent::Ready(handle, peer));"),
 ("early-conflict-active-ignored", "h_early_conflict", "aktif ese Some(X): sessizce yok sayilir (Ok)", ACTIVE_CONFLICT,
  "        if let Some(&handle) = self.active_by_peer.get(&peer) {"),
 ("early-conflict-pending-ignored", "h_early_conflict", "pending ese Some(X): sessizce yok sayilir (Ok)", PENDING_BRANCH,
  PENDING_BRANCH.replace("            if early_unit.is_some() {\n                return Err(TransportError::EarlyUnitConflict);\n            }\n", "")),
 ("early-conflict-leaks", "h_early_conflict", "aktif ese Some(X): hata doner AMA X teslim yoluna girer", ACTIVE_CONFLICT,
  """        if let Some(&handle) = self.active_by_peer.get(&peer) {
            if let Some(x) = early_unit {
                self.inbox.push_back(Delivered {
                    session: handle,
                    unit: x.bytes,
                });
                return Err(TransportError::EarlyUnitConflict);
            }"""),
 ("caps-strict", "h_caps", "len == max reddedilir (>=)", SEND_CAP, SEND_CAP.replace("unit.bytes.len() > self", "unit.bytes.len() >= self")),
 ("caps-loose", "h_caps", "len == max+1 kabul edilir", SEND_CAP,
  SEND_CAP.replace("unit.bytes.len() > self.capabilities.max_plaintext_unit_bytes as usize", "unit.bytes.len() > self.capabilities.max_plaintext_unit_bytes as usize + 1")),
 ("pending-cap-off-by-one", "h_pending_cap", "tavan + 1 pending kabul edilir", PENDING_CAP, "        if self.pending_by_peer.len() > PENDING_INIT_CAP"),
 ("pending-cap-none", "h_pending_cap", "pending tavani yok", PENDING_CAP, "        if false"),
 ("cancel-silent", "h_cancel", "cancel olay uretmez", CANCEL_TAIL,
  "        self.pending_by_peer.remove(&peer);\n        self.pending_early.remove(&ticket);"),
 ("cancel-ticket-survives", "h_cancel", "cancel olay uretir ama niyet/ticket yasar", CANCEL_TAIL,
  CANCEL_TAIL.replace("        self.pending_by_peer.remove(&peer);\n        self.pending_early.remove(&ticket);\n", "        let _ = peer;\n")),
 ("cancel-early-leaks", "h_cancel", "iptal edilen niyetin early_unit'i teslim yoluna girer", CANCEL_TAIL,
  CANCEL_TAIL.replace("        self.pending_early.remove(&ticket);",
   "        if let Some(bytes) = self.pending_early.remove(&ticket) {\n            self.inbox.push_back(Delivered {\n                session: SessionHandle::from_raw(0),\n                unit: bytes,\n            });\n        }")),
 ("cancel-double-event", "h_cancel", "cancel iki olay uretir", CANCEL_TAIL,
  CANCEL_TAIL + "\n        self.push_event(SessionEvent::ConnectAbandoned(\n            ticket,\n            AbandonReason::Cancelled,\n        ));"),
 ("idle-teardown", "h_cancel", "saat ilerleyince bekleyen niyet sonlanir (7 gun)", ADVANCE_HEAD,
  """    pub fn advance_clock(&mut self, by: Duration) {
        if by >= Duration::from_secs(7 * 24 * 60 * 60) {
            let peers: Vec<PeerRef> = self.pending_by_peer.keys().copied().collect();
            for p in peers {
                self.abandon_connect(&p, AbandonReason::KeyHygiene);
            }
        }
        if !self.link_up {
            return;
        }"""),
 ("progress-probes-while-suspended", "h_progress", "link yokken prob surer", ADVANCE_HEAD,
  "    pub fn advance_clock(&mut self, by: Duration) {"),
 ("progress-no-immediate-probe", "h_progress", "link gelince anlik prob yok", SETLINK_PHASE,
  SETLINK_PHASE.replace("            self.stats.probes += 1;\n", "")),
 ("progress-no-suspended-event", "h_progress", "link dusunce olay yok", SETLINK_PHASE,
  SETLINK_PHASE.replace("        for ticket in tickets {\n            self.push_event(SessionEvent::ConnectProgress(ticket, phase));\n        }",
   "        for ticket in tickets {\n            if up {\n                self.push_event(SessionEvent::ConnectProgress(ticket, phase));\n            }\n        }")),
 ("progress-wrong-phase", "h_progress", "link gelince Measuring yerine LongOutage", SETLINK_PHASE,
  SETLINK_PHASE.replace("            ConnectPhase::Measuring", "            ConnectPhase::LongOutage")),
 ("progress-resolves-intent", "h_progress", "link gelince niyet Ready'ye cozulur", SETLINK_PHASE,
  SETLINK_PHASE + "\n        if up {\n            self.complete_all_pending();\n        }"),
 ("progress-no-probes-ever", "h_progress", "link acikken de prob cikmaz (karsi-kontrol)", "            self.since_probe -= PROBE_INTERVAL;\n            self.stats.probes += 1;",
  "            self.since_probe -= PROBE_INTERVAL;"),
 ("silent-reject-abandons", "h_silent_reject", "sahte RESPONSE niyeti sonlandirir (olay)", INVALID,
  """    pub fn inject_invalid_response(&mut self, peer: &PeerRef) {
        self.stats.invalid_responses += 1;
        if self.pending_by_peer.contains_key(peer) {
            self.abandon_connect(peer, AbandonReason::LocalPermanent);
        }
    }"""),
 ("silent-reject-drops-pending", "h_silent_reject", "sahte RESPONSE niyeti SESSIZCE dusurur", INVALID,
  """    pub fn inject_invalid_response(&mut self, peer: &PeerRef) {
        self.stats.invalid_responses += 1;
        if let Some(ticket) = self.pending_by_peer.remove(peer) {
            self.pending_early.remove(&ticket);
        }
    }"""),
 ("silent-reject-resolves", "h_silent_reject", "sahte RESPONSE niyeti Ready'ye cozer", INVALID,
  """    pub fn inject_invalid_response(&mut self, peer: &PeerRef) {
        self.stats.invalid_responses += 1;
        if self.pending_by_peer.contains_key(peer) {
            self.complete_connect(peer);
        }
    }"""),
 ("compress-touched", "h_compress_boundary", "AppCompressed birimin ilk bayti atilir (Transport 'acar')", SEND_DELIVERED,
  """        let mut unit = unit;
        if unit.compression_policy == kama_contracts::s1::CompressionPolicy::AppCompressed {
            unit.bytes.remove(0);
        }
""" + SEND_DELIVERED),
 ("compress-dropped", "h_compress_boundary", "AppCompressed birim kabul edilir ama teslim edilmez", SEND_DELIVERED,
  """        if unit.compression_policy == kama_contracts::s1::CompressionPolicy::AppCompressed {
            return Ok(());
        }
""" + SEND_DELIVERED),
 ("retry-off-still-recovers", "h_app_retry_0", "recovery KAPALIYKEN de kurtarilir", DROP_BRANCH, DROP_BRANCH.replace("if self.recovery {", "if true {")),
 ("retry-on-loses", "h_app_retry_0", "recovery ACIKKEN kurtarilmaz", DROP_BRANCH, DROP_BRANCH.replace("if self.recovery {", "if false {")),
 ("retry-reorder", "h_app_retry_0", "sonraki birim kurtarilani gecer (sira bozulur)", BEHIND, "        if false {"),
 ("retry-duplicate", "h_app_retry_0", "kurtarilan birim iki kez teslim edilir", PUMP_PUSH,
  "            self.inbox.push_back(held.delivered.clone());\n            self.inbox.push_back(held.delivered);"),
 ("retry-drop-ignored", "h_app_retry_0", "kayip enjeksiyonu hic uygulanmaz", "        if self.drops_armed > 0 {", "        if false {"),
]


def die(msg: str) -> "NoReturn":  # noqa: F821
    print(f"ORTAM HATASI: {msg}", file=sys.stderr)
    sys.exit(2)


def scaffold(work: Path) -> None:
    """Gecici workspace: kama-contracts (kopya, bagimsiz manifest) + harness'i tasiyan minik crate."""
    (work / "crates/kama-integration/src").mkdir(parents=True)
    (work / "crates/kama-integration/src/lib.rs").write_text("", encoding="utf-8")
    (work / SEAM_REL).mkdir(parents=True)
    (work / CORE_REL).parent.mkdir(parents=True)
    (work / MOCK_REL).parent.mkdir(parents=True)
    shutil.copytree(REPO / CONTRACTS_REL / "src", work / CONTRACTS_REL / "src")
    # H-PENDING-CAP spec'teki sayiyi `<manifest>/../../specs/…` yolundan okur.
    (work / "specs").symlink_to(REPO / "specs", target_is_directory=True)
    (work / "Cargo.toml").write_text(
        '[workspace]\nresolver = "2"\nmembers = ["crates/kama-contracts", "crates/kama-integration"]\n',
        encoding="utf-8")
    (work / CONTRACTS_REL / "Cargo.toml").write_text(
        '[package]\nname = "kama-contracts"\nversion = "0.0.0"\nedition = "2021"\npublish = false\n\n'
        '[features]\ntest-util = []\n', encoding="utf-8")
    (work / "crates/kama-integration/Cargo.toml").write_text(
        '[package]\nname = "kama-integration"\nversion = "0.0.0"\nedition = "2021"\npublish = false\n\n'
        '[dev-dependencies]\nkama-contracts = { path = "../kama-contracts" }\nthiserror = "2"\n',
        encoding="utf-8")
    # Deponun kilidi: harici bagimliliklar (thiserror ve gecislileri) deponun surumlerinde kalsin.
    shutil.copy(REPO / "Cargo.lock", work / "Cargo.lock")


def sync(work: Path, mock_text: str) -> None:
    for f in HARNESS_FILES:
        shutil.copy(REPO / SEAM_REL / f, work / SEAM_REL / f)
    shutil.copy(REPO / CORE_REL, work / CORE_REL)
    (work / MOCK_REL).write_text(mock_text, encoding="utf-8")


def run(work: Path, offline: bool) -> tuple[list[str], list[str]] | str:
    env = dict(os.environ)
    env.setdefault("CARGO_BUILD_JOBS", "2")
    env["CARGO_TARGET_DIR"] = str(work / "target")
    cmd = ["cargo", "test", "-p", "kama-integration", "--test", "seam_conformance_core"]
    if offline:
        cmd.insert(2, "--offline")
    try:
        p = subprocess.run(cmd, cwd=work, env=env, capture_output=True, text=True, timeout=RUN_TIMEOUT_S)
    except subprocess.TimeoutExpired:
        return f"ZAMAN ASIMI: {RUN_TIMEOUT_S} s icinde bitmedi"
    out = p.stdout + p.stderr
    if "test result:" not in out:
        return out  # derlenmedi / cargo hatasi
    failed = sorted(set(re.findall(r"^test mock_contract::(h_\w+) \.\.\. FAILED$", out, re.M)))
    passed = sorted(set(re.findall(r"^test mock_contract::(h_\w+) \.\.\. ok$", out, re.M)))
    return failed, passed


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", nargs="+", default=None, metavar="ID")
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--online", action="store_true")
    ap.add_argument("--keep", action="store_true")
    args = ap.parse_args()

    ids = [m[0] for m in M]
    if len(set(ids)) != len(ids):
        die("mutasyon kimligi tekrar ediyor")
    if args.list:
        for mid, target, what, _old, _new in M:
            print(f"{mid:34s} {target:20s} {what}")
        return 0
    if args.only:
        unknown = sorted(set(args.only) - set(ids))
        if unknown:
            die(f"bilinmeyen mutasyon: {unknown}")

    pristine = (REPO / MOCK_REL).read_text(encoding="utf-8")
    for mid, _target, _what, old, _new in M:
        n = pristine.count(old)
        if n != 1:
            die(f"capa '{mid}': eski metin sahtede {n} kez bulundu (tam 1 olmali) — sahte degisti, capayi guncelleyin")

    work = Path(tempfile.mkdtemp(prefix="seam-mutation-"))
    try:
        scaffold(work)
        sync(work, pristine)
        base = run(work, not args.online)
        if isinstance(base, str):
            print(base[-3000:])
            die("taban derlenmedi (kayit onbellegi yoksa --online deneyin)")
        base_failed, rows = base
        if base_failed or not rows:
            die(f"taban yesil degil: kirmizi {base_failed}, yesil {len(rows)} satir")
        # Taban, suitin KENDI beyaniyla karsilastirilir (CONTRACT_ROWS): eksik kosan satir gorunsun.
        m = re.search(r'pub const CONTRACT_ROWS: &\[&str\] = &\[(.*?)\];', (REPO / SEAM_REL / "contract.rs").read_text(encoding="utf-8"), re.S)
        if not m:
            die("contract.rs icinde CONTRACT_ROWS bulunamadi")
        declared = sorted(r.lower().replace("-", "_") for r in re.findall(r'"(H-[A-Z0-9-]+)"', m.group(1)))
        if declared != rows:
            die(f"taban CONTRACT_ROWS ile ayrisiyor: beyan {declared}, kosan {rows}")
        print(f"taban: {len(rows)}/{len(declared)} davranis satiri yesil (mutasyonsuz sahte; CONTRACT_ROWS ile ayni kume)")

        results = []
        for mid, target, what, old, new in M:
            if args.only and mid not in args.only:
                continue
            if target not in rows:
                die(f"mutasyon '{mid}' var olmayan bir satiri hedefliyor: {target}")
            sync(work, pristine.replace(old, new))
            res = run(work, not args.online)
            if isinstance(res, str):
                print("\n".join(l for l in res.splitlines() if "error" in l or "-->" in l or "ZAMAN" in l)[:2000])
                die(f"mutasyon '{mid}' derlenmedi ya da zaman asimina ugradi")
            failed, _passed = res
            killed = target in failed
            results.append((mid, target, killed))
            # Butun satirlar birden kirmiziysa mutant hedefe ozgu bir seyi degil ortak kurulumu bozmustur;
            # "oldu" sayilir ama isaretlenir (elle bakilsin).
            wide = "  [!] BUTUN satirlar kirmizi" if len(failed) == len(rows) else ""
            print(f"{'OLDU  ' if killed else 'YASADI'}  {mid:34s} hedef={target:20s} kirmizi={','.join(failed) or '-'}{wide}")

        alive = [mid for mid, _t, killed in results if not killed]
        # Mutasyon kumesinin KENDI kapsami: mutasyonu olmayan satirin "olcuyor" oldugu gosterilmemistir.
        uncovered = [] if args.only else sorted(set(rows) - {m[1] for m in M})
        print(f"\n{len(results) - len(alive)}/{len(results)} mutasyon hedef satirinda KIRMIZI"
              + (f" · YASAYAN: {alive}" if alive else "")
              + (f" · MUTASYONSUZ SATIR: {uncovered}" if uncovered else ""))
        return 1 if (alive or uncovered) else 0
    finally:
        if args.keep:
            print(f"calisma agaci birakildi: {work}")
        else:
            shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
