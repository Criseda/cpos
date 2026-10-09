// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
//
//! The syscall layer as an unprivileged task sees it: a started scheduler
//! with real task slots, driven by random system calls with hostile
//! arguments (pointers at slot edges, into other slots, into flash, or
//! plain garbage), random ticks and random task switches.
//!
//! Properties: the kernel never panics, never touches memory outside the
//! slot area (guard bytes stay intact, and AddressSanitizer watches every
//! access), and keeps running.

#![no_main]

use libfuzzer_sys::fuzz_target;
use rust_kernel::syscall::handler::dispatch;
use rust_kernel::syscall::numbers::*;
use rust_kernel::task::{Config, Kernel, MAX_TASKS, SLOT_SIZE, TASK_CONSOLE, TASK_IDLE};

const GUARD: usize = SLOT_SIZE;
const FILL: u8 = 0x5A;

#[repr(C, align(2048))]
struct Memory([u8; GUARD + SLOT_SIZE * MAX_TASKS + GUARD]);

static FLASH: [u8; 256] = [0x42; 256];

const CALLS: [usize; 11] = [
    SYS_WRITE, SYS_READ, SYS_EXIT, SYS_SLEEP, SYS_YIELD, SYS_TICKS, SYS_ALLOC, SYS_FREE, SYS_SEND, SYS_RECV, 999,
];

struct Input<'a>(&'a [u8]);

impl Input<'_> {
    fn byte(&mut self) -> u8 {
        match self.0.split_first() {
            Some((&b, rest)) => {
                self.0 = rest;
                b
            }
            None => 0,
        }
    }

    fn word(&mut self) -> usize {
        u32::from_le_bytes([self.byte(), self.byte(), self.byte(), self.byte()]) as usize
    }

    /// An argument biased towards interesting addresses and small values
    fn arg(&mut self, slots: usize) -> usize {
        match self.byte() % 5 {
            0 => slots + (self.byte() as usize % MAX_TASKS) * SLOT_SIZE + self.word() % (SLOT_SIZE + 64),
            1 => slots - 64 + self.word() % (SLOT_SIZE * MAX_TASKS + 128),
            2 => FLASH.as_ptr() as usize + self.word() % 300,
            3 => self.byte() as usize,
            _ => self.word(),
        }
    }
}

fn worker(_: u32) {}

fuzz_target!(|data: &[u8]| {
    let mem = Box::into_raw(Box::new(Memory([FILL; GUARD + SLOT_SIZE * MAX_TASKS + GUARD])));
    let slots = mem as usize + GUARD;

    let mut k = Box::new(Kernel::new());
    let entry = worker as usize;
    k.register(b"idle", entry, 0, TASK_IDLE).unwrap();
    k.register(b"console", entry, 0, TASK_CONSOLE).unwrap();
    for _ in 0..12 {
        k.register(b"worker", entry, 0, 0).unwrap();
    }
    k.start(Config {
        slot_base: slots,
        flash_start: FLASH.as_ptr() as usize,
        flash_end: FLASH.as_ptr() as usize + FLASH.len(),
        exit_trampoline: entry,
    })
    .unwrap();
    k.switch(0);

    let mut input = Input(data);
    while !input.0.is_empty() {
        match input.byte() % 8 {
            0 => k.tick(),
            1 => {
                k.switch(0);
            }
            _ => {
                let raw = input.byte();
                let number = if raw < 200 { CALLS[raw as usize % CALLS.len()] } else { raw as usize };
                let (a1, a2, a3) = (input.arg(slots), input.arg(slots), input.arg(slots));
                if k.current().is_none() {
                    k.switch(0);
                }
                dispatch(&mut k, number, a1, a2, a3);
                // What PendSV would do after a blocking call or an exit
                if k.current().is_none() || input.byte() % 2 == 0 {
                    k.switch(0);
                }
            }
        }
    }

    // SAFETY: from `Box::into_raw` above
    let mem = unsafe { Box::from_raw(mem) };
    assert!(mem.0[..GUARD].iter().all(|&b| b == FILL), "write below the slots");
    assert!(mem.0[GUARD + SLOT_SIZE * MAX_TASKS..].iter().all(|&b| b == FILL), "write above the slots");
});
