//! Does ARQ contribute in IID loss, or only spend bandwidth?
//!
//! FS-FUSION-CELL-1 showed tail-capable ARQ completes 0/12 under IID-20% while FEC completes 9/12.
//! That raises Abkarada's question: in that regime is ARQ doing anything except retransmitting?
//! Every arm in this harness has ARQ always-on by construction, so "FEC without ARQ" is not an arm
//! that exists. What CAN be measured is ARQ's byte spend and whether recovery came from coding.
#![allow(clippy::duplicate_mod)]
#[path = "../tests/common/fs_campaign.rs"]
mod fs_campaign;
use fs_campaign::*;

fn main() {
    let cells = amended_base_cells();
    println!("cell,arm,n,complete,arq_retx_bytes,parity_bytes,total_bytes,fec_decoded_sources,retransmits");
    for c in &cells {
        let cid = base_cell_id(c);
        for (label, mk) in [("tail-only", 0u16), ("fusion", 2)] {
            let (mut comp, mut arqb, mut interv, mut tot, mut dec, mut retx) =
                (0u32, 0u64, 0u64, 0u64, 0usize, 0u64);
            for rep in 0..12u8 {
                let r = run_spine(SpineArm::A0Tail { r: mk }, &build_cfg(c, rep));
                comp += u32::from(r.completed);
                arqb += r.arq_retransmit_bytes;
                interv += r.intervention_bytes;
                tot += r.total_outcome_bytes;
                dec += r.fec_decoded_sources;
                retx += r.retransmits;
            }
            println!("{cid},{label},12,{comp},{arqb},{interv},{tot},{dec},{retx}");
        }
    }
}
