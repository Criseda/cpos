// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
//
// Author: Laurentiu Cristian Preda (criseda)
// GitHub: https://github.com/criseda
//
//! Byte-level access to the C UART driver
//!
//! Host tests swap the driver for in-memory buffers.

#[cfg(target_arch = "arm")]
mod imp {
    extern "C" {
        fn uart_send_char(c: u8);
        fn uart_receive_char() -> u8;
        fn uart_mark_scheduler_started();
        fn uart_privileged_writes() -> u32;
    }

    pub fn send(byte: u8) {
        // SAFETY: the C driver polls the UART registers; it has no
        // preconditions
        unsafe { uart_send_char(byte) }
    }

    pub fn receive() -> u8 {
        // SAFETY: as above
        unsafe { uart_receive_char() }
    }

    /// From now on the driver counts every byte the kernel writes itself
    pub fn mark_scheduler_started() {
        // SAFETY: sets a flag in the C driver
        unsafe { uart_mark_scheduler_started() }
    }

    /// Bytes privileged code wrote since `mark_scheduler_started`
    pub fn privileged_writes() -> u32 {
        // SAFETY: reads a counter in the C driver
        unsafe { uart_privileged_writes() }
    }
}

#[cfg(not(target_arch = "arm"))]
pub(crate) mod imp {
    extern crate std;

    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::vec::Vec;

    std::thread_local! {
        pub static OUTPUT: RefCell<Vec<u8>> = RefCell::new(Vec::new());
        pub static INPUT: RefCell<VecDeque<u8>> = RefCell::new(VecDeque::new());
    }

    pub fn send(byte: u8) {
        OUTPUT.with(|o| o.borrow_mut().push(byte));
    }

    pub fn receive() -> u8 {
        INPUT.with(|i| i.borrow_mut().pop_front().expect("test UART input exhausted"))
    }

    pub fn mark_scheduler_started() {}

    pub fn privileged_writes() -> u32 {
        0
    }
}

pub use imp::{mark_scheduler_started, privileged_writes, receive, send};

/// Send raw bytes; unlike the C `uart_send_string` this needs no NUL terminator
pub fn send_bytes(bytes: &[u8]) {
    for &b in bytes {
        send(b);
    }
}
