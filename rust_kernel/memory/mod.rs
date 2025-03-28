//! Memory management module for the kernel

pub mod allocator;

// Re-export the allocator functions
pub use allocator::{alloc, free, init_heap};

// Define memory constants
pub const HEAP_START: usize = 0x20001000;
pub const HEAP_SIZE: usize = 0x6000; // 24KB
