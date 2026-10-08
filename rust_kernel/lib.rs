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

#![cfg_attr(not(test), no_std)]

pub mod memory;
pub mod syscall;
mod uart;

// Required for no_std environments
#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
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
///
/// Returns 0 on success, -1 for a pointer the allocator did not hand out
/// and -2 for a double free.
#[no_mangle]
pub extern "C" fn rust_heap_free(ptr: *mut u8) -> i32 {
    match unsafe { memory::free(ptr) } {
        Ok(()) => 0,
        Err(memory::FreeError::InvalidPointer) => -1,
        Err(memory::FreeError::DoubleFree) => -2,
    }
}

/// Number of bytes currently free on the heap
#[no_mangle]
pub extern "C" fn rust_heap_free_bytes() -> usize {
    memory::stats().free_bytes
}

/// Number of blocks on the free list (1 means fully coalesced)
#[no_mangle]
pub extern "C" fn rust_heap_free_blocks() -> usize {
    memory::stats().free_blocks
}

/// Process a system call from C code
///
/// This function is called by the C SVC handler
#[no_mangle]
pub extern "C" fn rust_syscall(number: usize, arg1: usize, arg2: usize, arg3: usize) -> isize {
    syscall::handle_syscall(number, arg1, arg2, arg3)
}
