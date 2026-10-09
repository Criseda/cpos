// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
//
//! A task owns its heap memory and can scribble over the allocator's
//! headers. This target interleaves allocations and frees (of real and
//! made-up pointers) with arbitrary writes anywhere in the heap.
//!
//! Properties: the allocator never crashes or hangs, and never writes
//! outside the heap (guard bytes on both sides stay intact).

#![no_main]

use libfuzzer_sys::fuzz_target;
use rust_kernel::memory;

const SIZE: usize = 4096;
const GUARD: usize = 256;
const FILL: u8 = 0xA5;

#[repr(C, align(8))]
struct Arena([u8; GUARD + SIZE + GUARD]);

fuzz_target!(|data: &[u8]| {
    let arena = Box::into_raw(Box::new(Arena([FILL; GUARD + SIZE + GUARD])));
    let base = arena as usize + GUARD;
    // SAFETY: the arena is ours alone until it is freed below
    unsafe { memory::init_heap(base, SIZE) };

    let mut handed_out: Vec<usize> = Vec::new();

    for op in data.chunks(4) {
        let arg = u32::from_le_bytes([
            *op.get(1).unwrap_or(&0),
            *op.get(2).unwrap_or(&0),
            *op.get(3).unwrap_or(&0),
            0,
        ]) as usize;
        match op[0] % 5 {
            0 => {
                let p = memory::alloc(arg % 1024) as usize;
                if p != 0 {
                    assert!(p >= base && p < base + SIZE, "pointer outside heap");
                    handed_out.push(p);
                }
            }
            1 if !handed_out.is_empty() => {
                let p = handed_out[arg % handed_out.len()];
                let _ = unsafe { memory::free(p as *mut u8) };
            }
            2 => {
                // Free a made-up pointer in or around the heap
                let p = base - GUARD + arg % (SIZE + 2 * GUARD);
                let _ = unsafe { memory::free(p as *mut u8) };
            }
            _ => {
                // Scribble a word anywhere inside the heap, headers included
                let offset = (arg % SIZE) & !3;
                let value = u32::from_le_bytes([op[0], *op.get(1).unwrap_or(&0), *op.get(3).unwrap_or(&0), *op.get(2).unwrap_or(&0)]);
                // SAFETY: inside the heap memory we own
                unsafe { ((base + offset) as *mut u32).write_unaligned(value) };
            }
        }
        let _ = memory::stats();
    }

    // SAFETY: from `Box::into_raw` above
    let arena = unsafe { Box::from_raw(arena) };
    assert!(arena.0[..GUARD].iter().all(|&b| b == FILL), "write below the heap");
    assert!(arena.0[GUARD + SIZE..].iter().all(|&b| b == FILL), "write above the heap");
});
