use std::alloc::Layout;

use rpc_ring::compact_str::CompactString;

fn main() {
    dbg!(Layout::new::<CompactString<56>>());
}
