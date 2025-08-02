use rpc_ring::SpscRing;
use rpc_ring::compact_str::CompactString48;
use rpc_ring::def_schema;

type MyRing = SpscRing<Sqe, Cqe, 32, 32, Metadata>;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct Metadata {
    magic: [u8; 4],
    version: u8,
    _resv: [u8; 3],
}

impl Default for Metadata {
    fn default() -> Self {
        Self {
            magic: *b"ring",
            version: 1,
            _resv: Default::default(),
        }
    }
}

def_schema! {
    struct Sqe: size = 64, enum Request;
    struct Cqe: size = 64, union Response;

    0x1000:
    LinkRead -> Result<CompactString48, i32>;
    FileStat -> Result<Box<std::fs::Metadata>, i32>;

    0x2000:
    FileRemove -> i32;
}

pub struct FileStat {
    path: CompactString48,
}

pub struct LinkRead {
    path: CompactString48,
}

pub struct FileRemove {
    path: CompactString48,
}

fn main() {
    
}
