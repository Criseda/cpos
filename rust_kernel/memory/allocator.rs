// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
//
// Author: Laurentiu Cristian Preda (criseda)
// GitHub: https://github.com/criseda
//
//! Linked list allocator for kernel heap
//!
//! Every block, free or allocated, starts with the same `BlockHeader`, and
//! `size` always holds the full block size including the header. That keeps
//! the accounting exact: freeing a block hands back exactly the bytes that
//! allocating it took.
//!
//! The free list is kept sorted by address so that neighbouring free blocks
//! can always be coalesced on free.

use core::mem::{align_of, size_of};
use core::ptr::null_mut;
use spin::Mutex;

/// Alignment of every block and every returned pointer (AAPCS requires 8 for
/// 64-bit types).
const ALIGN: usize = 8;

#[repr(C)]
struct BlockHeader {
    /// Full block size in bytes, header included
    size: usize,
    /// Next free block (only meaningful while the block is free)
    next: *mut BlockHeader,
}

/// Header size rounded up so the payload stays `ALIGN`-aligned
const HEADER: usize = align_up(size_of::<BlockHeader>(), ALIGN);

/// Smallest block worth splitting off as a separate free block
const MIN_BLOCK: usize = HEADER + ALIGN;

const _: () = assert!(ALIGN >= align_of::<BlockHeader>());

/// Heap usage snapshot
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeapStats {
    pub heap_size: usize,
    pub free_bytes: usize,
    pub free_blocks: usize,
}

/// Reasons `deallocate` can reject a pointer
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FreeError {
    /// Pointer is outside the heap or misaligned
    InvalidPointer,
    /// Pointer refers to a block that is already free
    DoubleFree,
    /// The free list holds a header that cannot be valid
    HeapCorrupted,
}

pub(crate) struct LinkedListAllocator {
    head: *mut BlockHeader,
    heap_start: usize,
    heap_end: usize,
}

// SAFETY: the raw pointers refer to heap memory the allocator owns
// exclusively; every instance is reached through a mutex (the global heap)
// or through the kernel state mutex (task heaps), so it is never used from
// two contexts at once.
unsafe impl Send for LinkedListAllocator {}

impl LinkedListAllocator {
    pub(crate) const fn new() -> Self {
        LinkedListAllocator {
            head: null_mut(),
            heap_start: 0,
            heap_end: 0,
        }
    }

    /// Hand the region `[heap_start, heap_start + heap_size)` to the allocator.
    ///
    /// # Safety
    ///
    /// The region must be valid, writable, unused by anything else, and
    /// this must be called once before any allocation.
    pub(crate) unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        let start = align_up(heap_start, ALIGN);
        let end = (heap_start + heap_size) & !(ALIGN - 1);
        self.heap_start = start;
        self.heap_end = start;
        self.head = null_mut();

        if end > start && end - start >= MIN_BLOCK {
            self.heap_end = end;
            let block = start as *mut BlockHeader;
            (*block).size = end - start;
            (*block).next = null_mut();
            self.head = block;
        }
    }

    /// Validate a free-list entry before trusting it.
    ///
    /// The heap may live in memory an unprivileged task can write, so a
    /// header can hold anything. A free block must lie inside the heap, be
    /// aligned, be at least `MIN_BLOCK` long and start at or after
    /// `min_addr`, which keeps the walk strictly ascending and therefore
    /// finite. Returns the block size, or `None` if the entry cannot be a
    /// real free block.
    fn check_free(&self, block: *mut BlockHeader, min_addr: usize) -> Option<usize> {
        let start = block as usize;
        if start % ALIGN != 0 || start < self.heap_start || start < min_addr {
            return None;
        }
        if start >= self.heap_end || self.heap_end - start < MIN_BLOCK {
            return None;
        }
        // SAFETY: `start` is aligned and a whole header fits before `heap_end`
        let size = unsafe { (*block).size };
        if size < MIN_BLOCK || size % ALIGN != 0 || size > self.heap_end - start {
            return None;
        }
        Some(size)
    }

    /// Allocate `size` bytes, returning null if the heap cannot satisfy it
    /// or its free list is corrupted
    pub(crate) fn allocate(&mut self, size: usize) -> *mut u8 {
        let need = match size
            .max(1)
            .checked_add(ALIGN - 1)
            .map(|s| (s & !(ALIGN - 1)) + HEADER)
        {
            Some(n) => n,
            None => return null_mut(),
        };

        let mut prev: *mut BlockHeader = null_mut();
        let mut current = self.head;
        let mut min_addr = self.heap_start;

        while !current.is_null() {
            let block_size = match self.check_free(current, min_addr) {
                Some(s) => s,
                None => return null_mut(),
            };
            // SAFETY: `check_free` proved `current` is a whole, aligned
            // block inside the heap, and so is `rest` when we split, since
            // it lies within `current`. `prev` was checked the same way on
            // the previous iteration.
            unsafe {
                if block_size >= need {
                    let replacement = if block_size - need >= MIN_BLOCK {
                        // Split: the tail stays free and takes our place in the list
                        let rest = (current as usize + need) as *mut BlockHeader;
                        (*rest).size = block_size - need;
                        (*rest).next = (*current).next;
                        (*current).size = need;
                        rest
                    } else {
                        // Too small to split: hand out the whole block
                        (*current).next
                    };

                    if prev.is_null() {
                        self.head = replacement;
                    } else {
                        (*prev).next = replacement;
                    }

                    (*current).next = null_mut();
                    return (current as usize + HEADER) as *mut u8;
                }
                min_addr = current as usize + block_size + 1;
                prev = current;
                current = (*current).next;
            }
        }

        null_mut()
    }

    /// Return a block to the free list, coalescing with its neighbours.
    ///
    /// Pointers outside the heap, misaligned pointers and pointers into
    /// already-free memory are rejected instead of corrupting the list.
    /// Every header is bounds-checked before it is used, so even a
    /// corrupted heap only ever causes writes inside `[heap_start, heap_end)`.
    ///
    /// # Safety
    ///
    /// The heap region passed to `init` must still be valid memory.
    pub(crate) unsafe fn deallocate(&mut self, ptr: *mut u8) -> Result<(), FreeError> {
        let addr = ptr as usize;
        if addr % ALIGN != 0 || addr < self.heap_start + HEADER || addr >= self.heap_end {
            return Err(FreeError::InvalidPointer);
        }

        let block = (addr - HEADER) as *mut BlockHeader;
        let block_start = block as usize;
        // SAFETY: `block_start` is aligned and a whole header lies inside the heap
        let size = (*block).size;
        if size < MIN_BLOCK || size % ALIGN != 0 || size > self.heap_end - block_start {
            return Err(FreeError::InvalidPointer);
        }
        let block_end = block_start + size;

        // Find the free blocks on either side of us
        let mut prev: *mut BlockHeader = null_mut();
        let mut prev_size = 0;
        let mut next = self.head;
        let mut next_size = 0;
        let mut min_addr = self.heap_start;
        while !next.is_null() {
            next_size = self.check_free(next, min_addr).ok_or(FreeError::HeapCorrupted)?;
            if next as usize >= block_start {
                break;
            }
            min_addr = next as usize + next_size + 1;
            prev = next;
            prev_size = next_size;
            // SAFETY: `next` passed `check_free`
            next = (*next).next;
        }

        // Any overlap with a free block means this one is already free
        if !prev.is_null() && prev as usize + prev_size > block_start {
            return Err(FreeError::DoubleFree);
        }
        if !next.is_null() && (next as usize) < block_end {
            return Err(FreeError::DoubleFree);
        }

        // SAFETY (rest of the function): `block`, `prev` and `next` are
        // validated blocks inside the heap and do not overlap each other.

        // Merge with the following block
        if !next.is_null() && next as usize == block_end {
            (*block).size += next_size;
            (*block).next = (*next).next;
        } else {
            (*block).next = next;
        }

        // Merge with the preceding block, or link in after it
        if !prev.is_null() && prev as usize + prev_size == block_start {
            (*prev).size += (*block).size;
            (*prev).next = (*block).next;
        } else if prev.is_null() {
            self.head = block;
        } else {
            (*prev).next = block;
        }

        Ok(())
    }

    /// Free-space summary; stops counting at the first corrupted entry
    pub(crate) fn stats(&self) -> HeapStats {
        let mut free_bytes = 0;
        let mut free_blocks = 0;
        let mut current = self.head;
        let mut min_addr = self.heap_start;
        while !current.is_null() {
            let size = match self.check_free(current, min_addr) {
                Some(s) => s,
                None => break,
            };
            free_bytes += size;
            free_blocks += 1;
            min_addr = current as usize + size + 1;
            // SAFETY: `current` passed `check_free`
            current = unsafe { (*current).next };
        }
        HeapStats {
            heap_size: self.heap_end - self.heap_start,
            free_bytes,
            free_blocks,
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
/// Pointer to allocated memory (8-byte aligned) or null if out of memory
pub fn alloc(size: usize) -> *mut u8 {
    ALLOCATOR.lock().allocate(size)
}

/// Free previously allocated memory. Freeing null is a no-op.
///
/// # Safety
///
/// The pointer must have been previously returned by `alloc`
pub unsafe fn free(ptr: *mut u8) -> Result<(), FreeError> {
    if ptr.is_null() {
        return Ok(());
    }
    ALLOCATOR.lock().deallocate(ptr)
}

/// Current heap usage
pub fn stats() -> HeapStats {
    ALLOCATOR.lock().stats()
}

// Helper function for alignment
const fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::boxed::Box;
    use std::vec::Vec;

    const HEAP_SIZE: usize = 24 * 1024;

    /// Miri interprets every step, so it runs fewer iterations of the same
    /// tests
    const SCALE: usize = if cfg!(miri) { 50 } else { 1 };

    #[repr(C, align(8))]
    struct Heap([u8; HEAP_SIZE]);

    /// Test heap memory, held only as a raw pointer. Moving a `Box` or
    /// borrowing it again would invalidate the pointers the allocator has
    /// derived from it (Miri checks this).
    struct HeapMem(*mut Heap);

    impl HeapMem {
        fn start(&self) -> *mut u8 {
            self.0.cast()
        }
    }

    impl Drop for HeapMem {
        fn drop(&mut self) {
            // SAFETY: created by `Box::into_raw` in `new_heap`
            drop(unsafe { Box::from_raw(self.0) });
        }
    }

    fn new_heap() -> (HeapMem, LinkedListAllocator) {
        let mem = HeapMem(Box::into_raw(Box::new(Heap([0; HEAP_SIZE]))));
        let mut a = LinkedListAllocator::new();
        unsafe { a.init(mem.start() as usize, HEAP_SIZE) };
        (mem, a)
    }

    /// Tiny deterministic PRNG so the tests need no dependencies
    struct XorShift(u64);
    impl XorShift {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    fn block_size(ptr: *mut u8) -> usize {
        unsafe { (*((ptr as usize - HEADER) as *const BlockHeader)).size }
    }

    /// Free list is sorted, fully coalesced, in bounds, and together with
    /// the live blocks accounts for every byte of the heap.
    fn check_invariants(a: &LinkedListAllocator, live: &[(*mut u8, usize, u8)]) {
        let mut current = a.head;
        let mut last_end = 0;
        while !current.is_null() {
            let start = current as usize;
            let size = unsafe { (*current).size };
            assert!(start >= a.heap_start && start + size <= a.heap_end, "block out of bounds");
            assert!(start % ALIGN == 0 && size % ALIGN == 0, "misaligned block");
            assert!(start > last_end || last_end == 0, "free list unsorted or not coalesced");
            last_end = start + size;
            current = unsafe { (*current).next };
        }

        let used: usize = live.iter().map(|&(p, _, _)| block_size(p)).sum();
        assert_eq!(a.stats().free_bytes + used, HEAP_SIZE, "bytes leaked");
    }

    fn assert_pristine(a: &LinkedListAllocator) {
        assert_eq!(
            a.stats(),
            HeapStats { heap_size: HEAP_SIZE, free_bytes: HEAP_SIZE, free_blocks: 1 }
        );
    }

    #[test]
    fn alloc_free_restores_heap() {
        let (_mem, mut a) = new_heap();
        let p = a.allocate(128);
        assert!(!p.is_null());
        unsafe { a.deallocate(p).unwrap() };
        assert_pristine(&a);
    }

    #[test]
    fn repeated_cycles_do_not_leak() {
        let (_mem, mut a) = new_heap();
        for size in [1, 7, 8, 13, 128, 1000] {
            for _ in 0..10_000 / SCALE {
                let p = a.allocate(size);
                assert!(!p.is_null());
                unsafe { a.deallocate(p).unwrap() };
            }
        }
        assert_pristine(&a);
    }

    #[test]
    fn pointers_are_aligned_and_usable() {
        let (_mem, mut a) = new_heap();
        let mut ptrs = Vec::new();
        for size in 1..64 {
            let p = a.allocate(size);
            assert_eq!(p as usize % ALIGN, 0);
            unsafe { p.write_bytes(0xAB, size) };
            ptrs.push(p);
        }
        for p in ptrs {
            unsafe { a.deallocate(p).unwrap() };
        }
        assert_pristine(&a);
    }

    #[test]
    fn exhaustion_then_recovery() {
        let (_mem, mut a) = new_heap();
        let mut ptrs = Vec::new();
        loop {
            let p = a.allocate(100);
            if p.is_null() {
                break;
            }
            ptrs.push(p);
        }
        assert!(ptrs.len() > 100);
        assert!(a.allocate(HEAP_SIZE).is_null());
        assert!(a.allocate(usize::MAX).is_null());

        // Free every other block first to force out-of-order coalescing
        for p in ptrs.iter().step_by(2) {
            unsafe { a.deallocate(*p).unwrap() };
        }
        for p in ptrs.iter().skip(1).step_by(2) {
            unsafe { a.deallocate(*p).unwrap() };
        }
        assert_pristine(&a);
        assert!(!a.allocate(HEAP_SIZE - HEADER).is_null());
    }

    #[test]
    fn rejects_bad_frees() {
        let (mem, mut a) = new_heap();
        let p = a.allocate(64);
        let q = a.allocate(64);
        unsafe {
            a.deallocate(p).unwrap();
            assert_eq!(a.deallocate(p), Err(FreeError::DoubleFree));
            assert_eq!(a.deallocate(p.add(1)), Err(FreeError::InvalidPointer));
            assert_eq!(a.deallocate(mem.start()), Err(FreeError::InvalidPointer));
            let outside = (mem.start() as usize + HEAP_SIZE + 64) as *mut u8;
            assert_eq!(a.deallocate(outside), Err(FreeError::InvalidPointer));
            a.deallocate(q).unwrap();
        }
        assert_pristine(&a);
    }

    #[test]
    fn corrupted_headers_never_escape_the_heap() {
        let (_mem, mut a) = new_heap();
        let p = a.allocate(64);
        let q = a.allocate(64);
        unsafe { a.deallocate(p).unwrap() };

        // Point the free block at memory outside the heap
        let outside = Box::new([0u64; 16]);
        let free_hdr = (p as usize - HEADER) as *mut BlockHeader;
        unsafe { (*free_hdr).next = outside.as_ptr() as *mut BlockHeader };
        assert!(a.allocate(HEAP_SIZE / 2).is_null());
        assert_eq!(unsafe { a.deallocate(q) }, Err(FreeError::HeapCorrupted));
        assert!(outside.iter().all(|&w| w == 0), "allocator wrote outside the heap");

        // A cycle must not hang the walk
        unsafe { (*free_hdr).next = free_hdr };
        assert!(a.allocate(HEAP_SIZE / 2).is_null());
        let _ = a.stats();

        // A size that runs past the end is rejected
        unsafe {
            (*free_hdr).next = null_mut();
            (*free_hdr).size = HEAP_SIZE * 2;
        }
        assert!(a.allocate(32).is_null());
    }

    #[test]
    fn randomized_stress() {
        for seed in 1..=(20 / SCALE.min(10)) as u64 {
            let (_mem, mut a) = new_heap();
            let mut rng = XorShift(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let mut live: Vec<(*mut u8, usize, u8)> = Vec::new();

            for step in 0..10_000 / SCALE {
                if live.is_empty() || rng.below(100) < 55 {
                    let max = if rng.below(10) == 0 { 4096 } else { 256 };
                    let size = 1 + rng.below(max);
                    let p = a.allocate(size);
                    if !p.is_null() {
                        let tag = step as u8;
                        unsafe { p.write_bytes(tag, size) };
                        live.push((p, size, tag));
                    }
                } else {
                    let (p, size, tag) = live.swap_remove(rng.below(live.len()));
                    // Contents untouched means no other allocation overlapped it
                    let data = unsafe { core::slice::from_raw_parts(p, size) };
                    assert!(data.iter().all(|&b| b == tag), "seed {seed}: overlap");
                    unsafe { a.deallocate(p).unwrap() };
                }
                check_invariants(&a, &live);
            }

            for (p, _, _) in live.drain(..) {
                unsafe { a.deallocate(p).unwrap() };
            }
            assert_pristine(&a);
        }
    }
}
