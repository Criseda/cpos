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

mod arch;
pub mod memory;
pub mod syscall;
pub mod task;
mod uart;

use task::{Config, KERNEL};

/// Print a message and stop. Used when the kernel itself is broken, so it
/// writes to the UART directly rather than through the console server.
fn halt(msg: &[u8], value: Option<u32>) -> ! {
    arch::disable_irq();
    uart::send_bytes(b"\nKERNEL PANIC: ");
    uart::send_bytes(msg);
    if let Some(v) = value {
        let mut line = task::ipc::Line::new();
        line.bytes(b" ").hex(v);
        uart::send_bytes(line.as_bytes());
    }
    uart::send_bytes(b"\n");
    loop {}
}

// Required for no_std environments
#[cfg(all(not(test), target_arch = "arm"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    halt(b"Rust panic", None)
}

// Export C-compatible functions

/// Initialize heap memory
///
/// This function must be called before any memory allocation.
#[no_mangle]
pub extern "C" fn rust_init_heap(heap_start: usize, heap_size: usize) {
    // SAFETY: called once during kernel initialization with the heap
    // region reserved in linker.ld
    unsafe {
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
/// (or a corrupted heap) and -2 for a double free.
#[no_mangle]
pub extern "C" fn rust_heap_free(ptr: *mut u8) -> i32 {
    // SAFETY: the allocator validates `ptr` before using it
    match unsafe { memory::free(ptr) } {
        Ok(()) => 0,
        Err(memory::FreeError::InvalidPointer) | Err(memory::FreeError::HeapCorrupted) => -1,
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
/// This function is called by the C kernel before the scheduler starts
#[no_mangle]
pub extern "C" fn rust_syscall(number: usize, arg1: usize, arg2: usize, arg3: usize) -> isize {
    syscall::handle_syscall(number, arg1, arg2, arg3)
}

/// Queue a task. `name` must be a NUL-terminated string in flash.
/// Returns the task id, or -1 if the queue is full.
///
/// # Safety
///
/// `name` must point to a NUL-terminated string that is never freed.
#[no_mangle]
pub unsafe extern "C" fn rust_task_register(name: *const u8, entry: usize, arg: usize, flags: u32) -> isize {
    let mut len = 0;
    while *name.add(len) != 0 {
        len += 1;
    }
    let name: &'static [u8] = core::slice::from_raw_parts(name, len);
    match KERNEL.lock().register(name, entry, arg, flags) {
        Some(id) => id as isize,
        None => -1,
    }
}

/// Start multitasking. Only returns, with a negative value, if the
/// configuration is unusable.
#[no_mangle]
pub extern "C" fn rust_sched_start(slot_base: usize, flash_end: usize, exit_trampoline: usize, tick_reload: u32) -> isize {
    let flash = match task::mpu::flash(512 * 1024) {
        Some(r) => r,
        None => return -1,
    };
    let cfg = Config { slot_base, flash_start: 0, flash_end, exit_trampoline };

    // No interrupt may take the kernel lock while we hold it in thread mode
    arch::disable_irq();
    if KERNEL.lock().start(cfg).is_err() {
        arch::enable_irq();
        return -1;
    }
    arch::set_priorities();
    arch::enable_fault_handlers();
    arch::mpu_enable(&[flash.pair()]);
    arch::irq_unmask(arch::UART0_IRQ);
    uart::mark_scheduler_started();
    arch::start_systick(tick_reload);
    arch::pend_switch();
    // PendSV runs as soon as interrupts are back on and never returns here
    arch::enable_irq();
    0
}

/// UART0 interrupt: received input. The kernel never touches the UART;
/// it masks the line and tells the console server, which reads the data.
#[no_mangle]
pub extern "C" fn rust_uart_irq() {
    KERNEL.lock().input_irq();
}

/// PendSV: called with the outgoing task's stack pointer (after r4-r11
/// were pushed), returns the stack pointer of the task to run next
#[no_mangle]
pub extern "C" fn rust_switch_context(saved_sp: usize) -> usize {
    KERNEL.lock().switch(saved_sp)
}

#[no_mangle]
pub extern "C" fn rust_systick() {
    KERNEL.lock().tick();
}

/// Fault entry. `exc_return` tells us where the fault came from; a fault
/// in a task kills that task, a fault in the kernel halts the system.
#[no_mangle]
pub extern "C" fn rust_fault(exc_return: usize, exception: usize) {
    let (cfsr, addr) = arch::take_fault_status();
    // Thread mode on the process stack means a task was running
    let from_task = exc_return & 0b1100 == 0b1100;
    if !from_task {
        halt(b"fault in kernel, CFSR", Some(cfsr));
    }
    let mut k = match KERNEL.try_lock() {
        Some(k) => k,
        None => halt(b"fault while kernel state locked", Some(cfsr)),
    };
    match k.current() {
        Some(slot) => k.task_fault(slot, exception, cfsr, addr),
        None => halt(b"task fault with no current task", Some(cfsr)),
    }
}
