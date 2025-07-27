pub mod compact_str;

use std::sync::atomic::AtomicU32;

use crossbeam_utils::CachePadded;
use rpc_ring_macro::def_schema;
use static_assertions::const_assert;

use crate::compact_str::CompactString;

#[repr(C, align(4096))]
struct Ring {
    // Offset 0
    metadata: Metadata,

    // Client write.
    sq_tail: CachePadded<AtomicU32>,
    cq_head: CachePadded<AtomicU32>,

    // Server write.
    sq_head: CachePadded<AtomicU32>,
    cq_tail: CachePadded<AtomicU32>,

    _padding: [u8; 4096 - size_of::<Metadata>() - 4 * size_of::<CachePadded<AtomicU32>>()],

    // Offset 4096
    /// Submission Queue.
    sq: [Sqe; 32], // size = 2048
    /// Completion Queue.
    cq: [Cqe; 32], // size = 2048
}

#[repr(C)]
struct Metadata {
    magic: [u8; 4],
    version: u8,
    _resv: [u8; 3],
}

const_assert!(size_of::<Sqe>() <= 64);
#[repr(C, align(64))]
struct Sqe {
    id: u64,
    req: Request,
}

const_assert!(size_of::<Cqe>() <= 64);
#[repr(C, align(64))]
struct Cqe {
    id: u64,
    resp: Response,
}

def_schema! {
    1000:
    LinkRead -> Result<CompactString<48>, i32>;
    FileStat -> Result<Box<std::fs::Metadata>, i32>;

    2000:
    FileRemove -> i32;
}

pub struct FileStat {
    path: CompactString<48>,
}

pub struct LinkRead {
    path: CompactString<48>,
}

pub struct FileRemove {
    path: CompactString<48>,
}
