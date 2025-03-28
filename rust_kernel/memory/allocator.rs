// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
//
// Author: Laurentiu Cristian Preda (criseda)
// GitHub: https://github.com/criseda
//
//! Linked list allocator for kernel heap
//!
//! This module provides a memory allocator implementation using a linked list
//! to track free memory blocks. It supports allocation, deallocation, and
//! memory block coalescing for efficient memory reuse.

use core::ptr::null_mut;
use spin::Mutex;

// Define a memory block structure to track allocations
#[repr(C)]
struct BlockHeader {
    size: usize,
    next: Option<*mut BlockHeader>,
}

impl BlockHeader {
    #[allow(dead_code)]
    const fn new(size: usize) -> Self {
        BlockHeader { size, next: None }
    }

    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}

// The actual allocator implementation
struct LinkedListAllocator {
    head: Option<*mut BlockHeader>,
    heap_start: usize,
    heap_end: usize,
}

// Explicitly mark as thread-safe
unsafe impl Send for LinkedListAllocator {}
unsafe impl Sync for LinkedListAllocator {}

impl LinkedListAllocator {
    // Create a new empty allocator
    const fn new() -> Self {
        LinkedListAllocator {
            head: None,
            heap_start: 0,
            heap_end: 0,
        }
    }

    // Initialize the allocator with heap bounds
    unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.add_free_region(heap_start, heap_size);
    }

    // Add a memory region to the free list
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        // Ensure the region is large enough for a block header
        assert!(size >= core::mem::size_of::<BlockHeader>());

        // Create a new block header
        let block_header = addr as *mut BlockHeader;
        (*block_header).size = size;
        (*block_header).next = self.head;

        // Add the new block to the start of the list
        self.head = Some(block_header);
    }

    // Find a free region with the given size and alignment
    fn find_region(
        &mut self,
        size: usize,
        align: usize,
    ) -> Option<(*mut BlockHeader, *mut BlockHeader)> {
        // Start with the head of our free list
        let mut current = self.head;
        let mut previous = null_mut();

        // Look through the list for a suitable block
        while let Some(region) = current {
            unsafe {
                let region_ref = &*region;
                let header_size = core::mem::size_of::<BlockHeader>();
                let alloc_start = align_up(region as usize + header_size, align);
                let alloc_end = alloc_start + size;

                // If this region is big enough
                if alloc_end <= region_ref.end_addr() {
                    // Region is suitable, return it
                    return Some((region, previous));
                }

                // Move to the next region
                previous = region;
                current = region_ref.next;
            }
        }

        // No suitable region found
        None
    }

    // Allocate a block with given size and alignment
    fn allocate(&mut self, size: usize, align: usize) -> *mut u8 {
        let size = core::cmp::max(size, 1);

        // Try to find a suitable region
        if let Some((region, previous)) = self.find_region(size, align) {
            unsafe {
                let region_ref = &*region;
                let header_size = core::mem::size_of::<BlockHeader>();
                let alloc_start = align_up(region as usize + header_size, align);
                let alloc_end = alloc_start + size;

                // Calculate the leftover parts
                let excess_size = region_ref.end_addr() - alloc_end;

                // Update our linked list
                if previous.is_null() {
                    // This was the first block
                    self.head = region_ref.next;
                } else {
                    // This was not the first block
                    (*previous).next = region_ref.next;
                }

                // If we have enough excess space, add it back as a free block
                if excess_size > header_size {
                    self.add_free_region(alloc_end, excess_size);
                }

                // Store the allocation size just before the returned pointer
                let size_ptr = (alloc_start - core::mem::size_of::<usize>()) as *mut usize;
                *size_ptr = size;

                return alloc_start as *mut u8;
            }
        }

        // No suitable region found
        null_mut()
    }

    // Free a previously allocated block
    unsafe fn deallocate(&mut self, ptr: *mut u8) {
        // Get the allocation size stored before the pointer
        let size_ptr = (ptr as usize - core::mem::size_of::<usize>()) as *mut usize;
        let size = *size_ptr;
        let mut total_size = size + core::mem::size_of::<usize>();

        // Calculate address of the block we're freeing
        let block_addr = ptr as usize - core::mem::size_of::<usize>();
        let block_end = block_addr + total_size;

        // Find where to insert in the sorted free list
        let mut current = self.head;
        let mut prev = None;

        // Find the right position (keeping the list sorted by address)
        while let Some(current_ptr) = current {
            if current_ptr as usize > block_addr {
                // Found the insertion point
                break;
            }
            prev = current;
            current = (*current_ptr).next;
        }

        // Check if we can coalesce with the block after us
        if let Some(next_block) = current {
            if next_block as usize == block_end {
                // The next block is adjacent to us, merge it
                let next_size = (*next_block).size;
                total_size += next_size;

                // Skip the next block in the list
                current = (*next_block).next;
            }
        }

        // Check if we can coalesce with the block before us
        if let Some(prev_block) = prev {
            if (*prev_block).end_addr() == block_addr {
                // The previous block is adjacent to us, merge with it
                (*prev_block).size += total_size;
                (*prev_block).next = current;
                return; // We're done, block is merged with previous
            }
        }

        // If we didn't merge with the previous block, create a new one
        let new_block = block_addr as *mut BlockHeader;
        (*new_block).size = total_size;
        (*new_block).next = current;

        // Update the list
        if let Some(prev_block) = prev {
            (*prev_block).next = Some(new_block);
        } else {
            self.head = Some(new_block);
        }
    }
}

// Global allocator instance
static ALLOCATOR: Mutex<LinkedListAllocator> = Mutex::new(LinkedListAllocator::new());

/// Initialize the heap allocator
///
/// # Safety
///
/// This function must be called exactly once before any allocation
pub unsafe fn init_heap(heap_start: usize, heap_size: usize) {
    ALLOCATOR.lock().init(heap_start, heap_size);
}

/// Allocate memory on the heap
///
/// # Returns
///
/// Pointer to allocated memory or null if out of memory
pub fn alloc(size: usize) -> *mut u8 {
    ALLOCATOR.lock().allocate(size, 4) // 4-byte alignment
}

/// Free previously allocated memory
///
/// # Safety
///
/// The pointer must have been previously returned by `alloc`
pub unsafe fn free(ptr: *mut u8) {
    if !ptr.is_null() {
        ALLOCATOR.lock().deallocate(ptr);
    }
}

// Helper function for alignment
fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}
