// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
//
// Author: Laurentiu Cristian Preda (criseda)
// GitHub: https://github.com/criseda
//
//! System call handler implementation
//!
//! This module handles system call dispatching and implementation.
//!
//! Calls come from one of two places:
//!
//! * The kernel itself, before the scheduler starts (boot-time tests).
//!   There is no calling task: buffers are trusted, I/O goes straight to
//!   the UART and memory comes from the kernel heap.
//! * An unprivileged task. Every pointer is checked against the memory the
//!   task owns, output goes to the console server as a message, and memory
//!   comes from the task's own heap.

use crate::memory::FreeError;
use crate::syscall::numbers::*;
use crate::task::{Kernel, Outcome, KERNEL};
use crate::uart;

/// Largest buffer a single read or write may transfer
const MAX_IO_SIZE: usize = 1024;

const STDIN: usize = 0;
const STDOUT: usize = 1;

pub fn free_error_code(e: FreeError) -> usize {
    match e {
        FreeError::InvalidPointer | FreeError::HeapCorrupted => ERROR_INVALID_ARGUMENT,
        FreeError::DoubleFree => ERROR_DOUBLE_FREE,
    }
}

/// Kernel-context write: send `size` bytes from `buffer` to the UART
fn kernel_write(fd: usize, buffer: usize, size: usize) -> SyscallResult {
    if fd != STDOUT || buffer == 0 || size > MAX_IO_SIZE {
        return Err(ERROR_INVALID_ARGUMENT);
    }

    // SAFETY: only kernel code calls without a task, and it passes its
    // own valid buffers
    let bytes = unsafe { core::slice::from_raw_parts(buffer as *const u8, size) };
    uart::send_bytes(bytes);
    Ok(size)
}

/// Kernel-context read: block until `size` bytes arrive or a line ends.
///
/// Returns the number of bytes stored, including the line terminator.
fn kernel_read(fd: usize, buffer: usize, size: usize) -> SyscallResult {
    if fd != STDIN || buffer == 0 || size == 0 || size > MAX_IO_SIZE {
        return Err(ERROR_INVALID_ARGUMENT);
    }

    // SAFETY: as in `kernel_write`
    let buf = unsafe { core::slice::from_raw_parts_mut(buffer as *mut u8, size) };
    let mut count = 0;
    for slot in buf.iter_mut() {
        let byte = uart::receive();
        *slot = byte;
        count += 1;
        if byte == b'\n' || byte == b'\r' {
            break;
        }
    }
    Ok(count)
}

fn kernel_alloc(size: usize) -> SyscallResult {
    if size == 0 {
        return Err(ERROR_INVALID_ARGUMENT);
    }

    let ptr = crate::memory::alloc(size);
    if ptr.is_null() {
        Err(ERROR_OUT_OF_MEMORY)
    } else {
        Ok(ptr as usize)
    }
}

fn kernel_free(ptr: usize) -> SyscallResult {
    if ptr == 0 {
        return Err(ERROR_INVALID_ARGUMENT);
    }

    // SAFETY: the allocator validates the pointer against the heap and
    // its free list before touching anything
    unsafe { crate::memory::free(ptr as *mut u8) }.map(|()| 0).map_err(free_error_code)
}

/// A call made without a calling task (the kernel before the scheduler runs)
fn kernel_call(k: &mut Kernel, number: usize, arg1: usize, arg2: usize, arg3: usize) -> SyscallResult {
    match number {
        SYS_WRITE => kernel_write(arg1, arg2, arg3),
        SYS_READ => kernel_read(arg1, arg2, arg3),
        SYS_ALLOC => kernel_alloc(arg1),
        SYS_FREE => kernel_free(arg1),
        SYS_TICKS => Ok(k.ticks() as usize),
        // Only meaningful for a task
        SYS_EXIT | SYS_SLEEP | SYS_YIELD | SYS_SEND | SYS_RECV => Err(ERROR_NO_TASK),
        _ => {
            uart::send_bytes(b"Unknown syscall\n");
            Err(ERROR_INVALID_SYSCALL)
        }
    }
}

/// A call made by the task in `slot`
fn task_call(k: &mut Kernel, slot: usize, number: usize, arg1: usize, arg2: usize, arg3: usize) -> Result<Outcome, usize> {
    if k.is_idle(slot) && number != SYS_YIELD && number != SYS_TICKS {
        // The scheduler falls back on idle, so idle must never block
        return Err(ERROR_INVALID_SYSCALL);
    }
    let value = match number {
        SYS_WRITE => {
            if arg1 != STDOUT || arg2 == 0 || arg3 > MAX_IO_SIZE {
                return Err(ERROR_INVALID_ARGUMENT);
            }
            k.check_readable(slot, arg2, arg3)?;
            return k.sys_write(slot, arg2, arg3);
        }
        // Console input would be a request to the console server; the
        // kernel no longer touches the UART once tasks run
        SYS_READ => return Err(ERROR_NOT_IMPLEMENTED),
        SYS_EXIT => return Ok(k.sys_exit(slot, arg1)),
        SYS_SLEEP => return Ok(k.sys_sleep(slot, arg1)),
        SYS_YIELD => return Ok(k.sys_yield()),
        SYS_TICKS => k.ticks() as usize,
        SYS_ALLOC if arg1 == 0 => return Err(ERROR_INVALID_ARGUMENT),
        SYS_ALLOC => k.sys_alloc(slot, arg1)?,
        SYS_FREE => k.sys_free(slot, arg1)?,
        SYS_SEND => return k.sys_send(slot, arg1, arg2, arg3),
        SYS_RECV => return k.sys_recv(slot, arg1, arg2, arg3),
        _ => return Err(ERROR_INVALID_SYSCALL),
    };
    Ok(Outcome::Return(value as isize))
}

/// Main syscall dispatcher
pub fn dispatch(k: &mut Kernel, number: usize, arg1: usize, arg2: usize, arg3: usize) -> Outcome {
    let result = match k.current() {
        Some(slot) if k.started() => task_call(k, slot, number, arg1, arg2, arg3),
        _ => kernel_call(k, number, arg1, arg2, arg3).map(|v| Outcome::Return(v as isize)),
    };
    // Negative values for errors
    result.unwrap_or_else(Outcome::error)
}

/// Kernel-side entry for C code that calls the dispatcher directly
pub fn handle_syscall(number: usize, arg1: usize, arg2: usize, arg3: usize) -> isize {
    match dispatch(&mut KERNEL.lock(), number, arg1, arg2, arg3) {
        Outcome::Return(v) => v,
        // Only tasks block, and C kernel code is not a task
        Outcome::Retry => -(ERROR_NO_TASK as isize),
    }
}

/// SVC handler implementation called from assembly
///
/// `frame` points at the exception stack frame (r0, r1, r2, r3, r12, lr,
/// pc, xpsr). The syscall number and arguments are read from it, and the
/// result is written back to the stacked r0 so the caller sees it in r0
/// after exception return. A blocked call instead has its stacked pc moved
/// back over the 2-byte `svc` instruction so it runs again on wake-up.
///
/// # Safety
///
/// `frame` must point at a valid 8-word exception frame.
#[no_mangle]
pub unsafe extern "C" fn rust_handle_svc(frame: *mut usize) {
    let number = *frame;
    let arg1 = *frame.add(1);
    let arg2 = *frame.add(2);
    let arg3 = *frame.add(3);

    let outcome = dispatch(&mut KERNEL.lock(), number, arg1, arg2, arg3);
    match outcome {
        Outcome::Return(v) => *frame = v as usize,
        Outcome::Retry => *frame.add(6) -= 2,
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::uart::imp::{INPUT, OUTPUT};
    use std::vec::Vec;

    fn take_output() -> Vec<u8> {
        OUTPUT.with(|o| core::mem::take(&mut *o.borrow_mut()))
    }

    fn call(number: usize, arg1: usize, arg2: usize, arg3: usize) -> isize {
        match dispatch(&mut Kernel::new(), number, arg1, arg2, arg3) {
            Outcome::Return(v) => v,
            Outcome::Retry => panic!("kernel calls never block"),
        }
    }

    #[test]
    fn write_sends_exactly_size_bytes() {
        take_output();
        let msg = b"Hello, world! trailing garbage";
        assert_eq!(call(SYS_WRITE, STDOUT, msg.as_ptr() as usize, 13), 13);
        assert_eq!(take_output(), b"Hello, world!");
    }

    #[test]
    fn write_rejects_bad_arguments() {
        let msg = b"x";
        let bad = -(ERROR_INVALID_ARGUMENT as isize);
        assert_eq!(call(SYS_WRITE, 2, msg.as_ptr() as usize, 1), bad);
        assert_eq!(call(SYS_WRITE, STDOUT, 0, 1), bad);
        assert_eq!(call(SYS_WRITE, STDOUT, msg.as_ptr() as usize, MAX_IO_SIZE + 1), bad);
    }

    #[test]
    fn read_stops_at_newline_or_size() {
        INPUT.with(|i| i.borrow_mut().extend(b"hi\nabcdef"));
        let mut buf = [0u8; 16];
        let ptr = buf.as_mut_ptr() as usize;
        assert_eq!(call(SYS_READ, STDIN, ptr, 16), 3);
        assert_eq!(&buf[..3], b"hi\n");
        assert_eq!(call(SYS_READ, STDIN, ptr, 4), 4);
        assert_eq!(&buf[..4], b"abcd");
    }

    #[test]
    fn task_only_calls_need_a_task() {
        let no_task = -(ERROR_NO_TASK as isize);
        for number in [SYS_EXIT, SYS_SLEEP, SYS_YIELD, SYS_SEND, SYS_RECV] {
            assert_eq!(call(number, 0, 0, 0), no_task);
        }
        assert_eq!(call(999, 0, 0, 0), -(ERROR_INVALID_SYSCALL as isize));
    }

    #[test]
    fn svc_writes_result_into_stacked_r0() {
        take_output();
        let msg = b"svc";
        let mut frame = [SYS_WRITE, STDOUT, msg.as_ptr() as usize, 3, 0, 0, 0x100, 0];
        unsafe { rust_handle_svc(frame.as_mut_ptr()) };
        assert_eq!(frame[0], 3);
        assert_eq!(frame[6], 0x100, "pc untouched when the call completes");
        assert_eq!(take_output(), b"svc");
    }
}
