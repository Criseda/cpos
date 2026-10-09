// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
//
//! Random sequences of valid allocations and frees, plus double frees.
//!
//! Properties: every pointer is aligned and inside the heap, live blocks
//! never overlap (their fill pattern survives), double frees are always
//! rejected, and freeing everything restores one fully coalesced block.

#![no_main]

use libfuzzer_sys::fuzz_target;
use rust_kernel::memory;

const SIZE: usize = 8192;

#[repr(C, align(8))]
struct Heap([u8; SIZE]);

fuzz_target!(|data: &[u8]| {
    let heap = Box::into_raw(Box::new(Heap([0; SIZE])));
    let base = heap as usize;
    // SAFETY: the heap memory is ours alone until it is freed below
    unsafe { memory::init_heap(base, SIZE) };
    let pristine = memory::stats();

    let mut live: Vec<(usize, usize, u8)> = Vec::new();
    let mut freed: Vec<usize> = Vec::new();

    for (step, op) in data.chunks(3).enumerate() {
        let arg = u16::from_le_bytes([*op.get(1).unwrap_or(&0), *op.get(2).unwrap_or(&0)]) as usize;
        match op[0] % 4 {
            0 | 1 => {
                let size = arg % 2048;
                let p = memory::alloc(size) as usize;
                if p != 0 {
                    assert_eq!(p % 8, 0, "misaligned");
                    assert!(p >= base && p + size <= base + SIZE, "outside heap");
                    let tag = step as u8;
                    // SAFETY: the allocator just handed us `size` bytes at `p`
                    unsafe { (p as *mut u8).write_bytes(tag, size) };
                    live.push((p, size, tag));
                    freed.retain(|&f| f != p);
                }
            }
            2 if !live.is_empty() => {
                let (p, size, tag) = live.swap_remove(arg % live.len());
                // SAFETY: still allocated, so still ours
                let bytes = unsafe { core::slice::from_raw_parts(p as *const u8, size) };
                assert!(bytes.iter().all(|&b| b == tag), "live block overwritten");
                assert_eq!(unsafe { memory::free(p as *mut u8) }, Ok(()));
                freed.push(p);
            }
            3 if !freed.is_empty() => {
                // Free again a pointer whose block is still free
                let p = freed[arg % freed.len()];
                // Skip it if a live block now covers its header. A block
                // can be longer than requested (a remainder too small to
                // split stays attached), so use the size in its header.
                // Freeing a pointer into a live block is use-after-free,
                // which no header-based allocator can detect;
                // `allocator_hostile` checks it still stays inside the heap.
                let header = p - 16;
                let reused = live.iter().any(|&(q, _, _)| {
                    // SAFETY: `q` is live, so its header is intact
                    let block = unsafe { ((q - 16) as *const usize).read() };
                    header + 16 > q - 16 && header < q - 16 + block
                });
                if !reused {
                    assert!(unsafe { memory::free(p as *mut u8) }.is_err(), "double free accepted");
                }
            }
            _ => {}
        }
    }

    for (p, _, _) in live.drain(..) {
        assert_eq!(unsafe { memory::free(p as *mut u8) }, Ok(()));
    }
    assert_eq!(memory::stats(), pristine, "leak or fragmentation after freeing everything");

    // SAFETY: from `Box::into_raw` above; the allocator is re-initialised
    // before its next use
    drop(unsafe { Box::from_raw(heap) });
});
