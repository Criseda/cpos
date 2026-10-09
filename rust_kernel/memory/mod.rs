// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
//
// Author: Laurentiu Cristian Preda (criseda)
// GitHub: https://github.com/criseda
//
//! Memory management module for the kernel
//!
//! This module provides memory management functionality for the kernel,
//! including heap allocation and memory layout definitions.

pub mod allocator;

// Re-export the allocator functions
pub use allocator::{alloc, free, init_heap, stats, FreeError, HeapStats};
pub(crate) use allocator::LinkedListAllocator;

// Define memory constants
pub const HEAP_START: usize = 0x20002000;
pub const HEAP_SIZE: usize = 0x1000; // 4KB, see linker.ld
