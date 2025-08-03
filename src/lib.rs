pub mod compact_str;

use core::mem;
use core::sync::atomic::AtomicU32;

use crossbeam_utils::CachePadded;
pub use rpc_ring_macro::def_schema;

#[repr(C, align(4096))]
pub struct SpscRing<Sqe, Cqe, const N_SQE: usize, const N_CQE: usize, Meta = ()> {
    /// Submission Queue.
    sq: [Sqe; N_SQE],
    /// Completion Queue.
    cq: [Cqe; N_CQE],

    // Client write.
    sq_tail: CachePadded<AtomicU32>,
    cq_head: CachePadded<AtomicU32>,

    // Server write.
    sq_head: CachePadded<AtomicU32>,
    cq_tail: CachePadded<AtomicU32>,

    meta: Meta,
}

impl<Sqe, Cqe, const N_SQE: usize, const N_CQE: usize, Meta> Default
    for SpscRing<Sqe, Cqe, N_SQE, N_CQE, Meta>
where
    Sqe: Copy,
    Cqe: Copy,
    Meta: Default,
{
    fn default() -> Self {
        Self {
            sq: unsafe { mem::MaybeUninit::uninit().assume_init() },
            cq: unsafe { mem::MaybeUninit::uninit().assume_init() },
            sq_tail: Default::default(),
            cq_head: Default::default(),
            sq_head: Default::default(),
            cq_tail: Default::default(),
            meta: Default::default(),
        }
    }
}

impl<Sqe, Cqe, const N_SQE: usize, const N_CQE: usize, Meta>
    SpscRing<Sqe, Cqe, N_SQE, N_CQE, Meta>
{
    pub fn meta(&self) -> &Meta {
        &self.meta
    }

    pub fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }
}
