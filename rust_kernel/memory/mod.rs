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
pub use allocator::{alloc, free, init_heap};

// Define memory constants
pub const HEAP_START: usize = 0x20001000;
pub const HEAP_SIZE: usize = 0x6000; // 24KB
