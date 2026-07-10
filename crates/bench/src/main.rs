//! Kama transport benchmark harness CLI (§4.4). SoC: `serve`/`client` HAM event üretir;
//! `metrics` HESAP yapar (transport'a dokunmaz). quinn baseline (§4.4.1) + kama stub.

mod error;
mod event;
mod metrics;
mod quic;
mod transport;

use clap::{Parser, Subcommand, ValueEnum};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use crate::error::BenchError;
use crate::transport::{BenchTransport, Cc, ClientCfg, ServerCfg};

#[derive(Parser)]
#[command(name = "bench", about = "Kama transport benchmark harness (§4.4)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Transport server'ı çalıştır.
    Serve {
        #[arg(long, default_value = "[::]:4433")]
        bind: SocketAddr,
        #[arg(long, value_enum, default_value_t = CcArg::Cubic)]
        cc: CcArg,
        #[arg(long, value_enum, default_value_t = TransportArg::Quinn)]
        transport: TransportArg,
    },
    /// Client: N mesaj gönder, ham event JSONL yaz.
    Client {
        #[arg(long)]
        server: SocketAddr,
        #[arg(long, default_value_t = 100)]
        msgs: u64,
        #[arg(long, default_value_t = 256)]
        payload: u64,
        #[arg(long, default_value_t = 120)]
        max_deadline_s: u64,
        #[arg(long, value_enum, default_value_t = CcArg::Cubic)]
        cc: CcArg,
        #[arg(long, default_value_t = 0)]
        seed: u64,
        #[arg(long, default_value_t = 0)]
        repeat: u32,
        #[arg(long, default_value = "default")]
        profile: String,
        #[arg(long, value_enum, default_value_t = TransportArg::Quinn)]
        transport: TransportArg,
        #[arg(long)]
        out: PathBuf,
    },
    /// Ham JSONL koşularından metrik hesapla (A: varyans, B: deadline eğrisi).
    Metrics {
        #[arg(long, num_args = 1.., required = true)]
        runs: Vec<PathBuf>,
        #[arg(long, value_delimiter = ',', default_value = "10,30,120")]
        deadlines_s: Vec<u64>,
    },
}

#[derive(Copy, Clone, ValueEnum)]
enum CcArg {
    Cubic,
    Bbr,
}

impl From<CcArg> for Cc {
    fn from(c: CcArg) -> Cc {
        match c {
            CcArg::Cubic => Cc::Cubic,
            CcArg::Bbr => Cc::Bbr,
        }
    }
}

#[derive(Copy, Clone, ValueEnum, PartialEq)]
enum TransportArg {
    Quinn,
    Kama,
}

#[tokio::main]
async fn main() -> Result<(), BenchError> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Serve {
            bind,
            cc,
            transport,
        } => {
            run_server(
                transport,
                ServerCfg {
                    bind,
                    cc: cc.into(),
                },
            )
            .await
        }
        Cmd::Client {
            server,
            msgs,
            payload,
            max_deadline_s,
            cc,
            seed,
            repeat,
            profile,
            transport,
            out,
        } => {
            let cfg = ClientCfg {
                server,
                msg_count: msgs,
                payload_bytes: payload,
                max_deadline: Duration::from_secs(max_deadline_s),
                cc: cc.into(),
                seed,
                repeat,
                profile,
            };
            let output = run_client(transport, cfg).await?;
            output.write_jsonl(&out)?;
            eprintln!("wrote {} events → {}", output.events.len(), out.display());
            Ok(())
        }
        Cmd::Metrics { runs, deadlines_s } => {
            let deadlines: Vec<Duration> = deadlines_s
                .iter()
                .map(|&s| Duration::from_secs(s))
                .collect();
            // Eksik/bozuk koşu dosyasında ABORT etme (workflow bulgusu #4: harsh profilde bir
            // handshake fail'i dosya yazmaz → tüm agregasyon ölmemeli). Atla + uyar.
            let outputs: Vec<_> = runs
                .iter()
                .filter_map(|p| match event::RunOutput::read_jsonl(p) {
                    Ok(o) => Some(o),
                    Err(e) => {
                        eprintln!("skip {}: {e}", p.display());
                        None
                    }
                })
                .collect();
            let per_run: Vec<_> = outputs
                .iter()
                .map(|o| metrics::run_metrics(o, &deadlines))
                .collect();
            let agg = metrics::aggregate(&per_run, &deadlines);
            println!(
                "# completion vs deadline — {} koşu (A: varyans, B: eğri)",
                per_run.len()
            );
            for a in &agg {
                println!(
                    "deadline={:>4}s  median={:.3}  IQR=[{:.3},{:.3}]  min={:.3}  max={:.3}  (n={})",
                    a.deadline.as_secs(),
                    a.completion_median,
                    a.completion_q1,
                    a.completion_q3,
                    a.completion_min,
                    a.completion_max,
                    a.runs,
                );
            }
            let sec = metrics::aggregate_secondary(&per_run);
            println!(
                "# secondary (§4.4.3): bytes/delivered-msg median={:.1}  ACK-overhead median={:.3}",
                sec.bytes_per_delivered_msg_median, sec.ack_overhead_ratio_median,
            );
            Ok(())
        }
    }
}

async fn run_server(t: TransportArg, cfg: ServerCfg) -> Result<(), BenchError> {
    match t {
        TransportArg::Quinn => quic::QuinnTransport.run_server(cfg).await,
        TransportArg::Kama => transport::KamaTransport.run_server(cfg).await,
    }
}

async fn run_client(t: TransportArg, cfg: ClientCfg) -> Result<event::RunOutput, BenchError> {
    match t {
        TransportArg::Quinn => quic::QuinnTransport.run_client(cfg).await,
        TransportArg::Kama => transport::KamaTransport.run_client(cfg).await,
    }
}
