// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
//
// Author: Laurentiu Cristian Preda (criseda)
// GitHub: https://github.com/criseda
//
//! Rust kernel components for CPOS
//!
//! This crate provides Rust implementations of kernel components
//! that interface with the existing C kernel.

#![no_std]

pub mod memory;

use core::panic::PanicInfo;

// Required for no_std environments
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

// Export C-compatible functions

/// Initialize heap memory
///
/// This function must be called before any memory allocation.
#[no_mangle]
pub extern "C" fn rust_init_heap(heap_start: usize, heap_size: usize) {
    unsafe {
        // This will be called once during kernel initialization
        memory::init_heap(heap_start, heap_size);
    }
}

/// Allocate memory from the heap
///
/// Returns a pointer to allocated memory, or NULL if out of memory
#[no_mangle]
pub extern "C" fn rust_heap_alloc(size: usize) -> *mut u8 {
    memory::alloc(size)
}

/// Free previously allocated memory
#[no_mangle]
pub extern "C" fn rust_heap_free(ptr: *mut u8) {
    // We trust the C code to only free valid pointers
    unsafe {
        memory::free(ptr);
    }
}
