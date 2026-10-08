// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
//
// Author: Laurentiu Cristian Preda (criseda)
// GitHub: https://github.com/criseda
//
//! System call handler implementation
//!
//! This module handles system call dispatching and implementation.

use crate::memory::FreeError;
use crate::syscall::numbers::*;
use crate::uart;

/// Largest buffer a single read or write may transfer
const MAX_IO_SIZE: usize = 1024;

const STDIN: usize = 0;
const STDOUT: usize = 1;

/// Handle write syscall: send `size` bytes from `buffer` to the UART
fn sys_write(fd: usize, buffer: usize, size: usize) -> SyscallResult {
    if fd != STDOUT || buffer == 0 || size > MAX_IO_SIZE {
        return Err(ERROR_INVALID_ARGUMENT);
    }

    let bytes = unsafe { core::slice::from_raw_parts(buffer as *const u8, size) };
    uart::send_bytes(bytes);
    Ok(size)
}

/// Handle read syscall: block until `size` bytes arrive or a line ends.
///
/// Returns the number of bytes stored, including the line terminator.
fn sys_read(fd: usize, buffer: usize, size: usize) -> SyscallResult {
    if fd != STDIN || buffer == 0 || size == 0 || size > MAX_IO_SIZE {
        return Err(ERROR_INVALID_ARGUMENT);
    }

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

/// Handle memory allocation syscall
fn sys_alloc(size: usize) -> SyscallResult {
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

/// Handle memory free syscall
fn sys_free(ptr: usize) -> SyscallResult {
    if ptr == 0 {
        return Err(ERROR_INVALID_ARGUMENT);
    }

    match unsafe { crate::memory::free(ptr as *mut u8) } {
        Ok(()) => Ok(0),
        Err(FreeError::InvalidPointer) => Err(ERROR_INVALID_ARGUMENT),
        Err(FreeError::DoubleFree) => Err(ERROR_DOUBLE_FREE),
    }
}

/// Main syscall dispatcher
pub fn handle_syscall(number: usize, arg1: usize, arg2: usize, arg3: usize) -> isize {
    let result = match number {
        SYS_WRITE => sys_write(arg1, arg2, arg3),
        SYS_READ => sys_read(arg1, arg2, arg3),
        SYS_ALLOC => sys_alloc(arg1),
        SYS_FREE => sys_free(arg1),
        // Need processes and a timer; see the scheduler milestone
        SYS_EXIT | SYS_SLEEP => Err(ERROR_NOT_IMPLEMENTED),
        _ => {
            uart::send_bytes(b"Unknown syscall\n");
            Err(ERROR_INVALID_SYSCALL)
        }
    };

    // Convert Result to the C return value
    match result {
        Ok(val) => val as isize,
        Err(err) => -(err as isize), // Negative values for errors
    }
}

/// SVC handler implementation called from assembly
///
/// `frame` points at the exception stack frame (r0, r1, r2, r3, r12, lr,
/// pc, xpsr). The syscall number and arguments are read from it, and the
/// result is written back to the stacked r0 so the caller sees it in r0
/// after exception return.
#[no_mangle]
pub unsafe extern "C" fn rust_handle_svc(frame: *mut usize) -> isize {
    let number = *frame;
    let arg1 = *frame.add(1);
    let arg2 = *frame.add(2);
    let arg3 = *frame.add(3);

    let result = handle_syscall(number, arg1, arg2, arg3);
    *frame = result as usize;
    result
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

    #[test]
    fn write_sends_exactly_size_bytes() {
        take_output();
        let msg = b"Hello, world! trailing garbage";
        assert_eq!(handle_syscall(SYS_WRITE, STDOUT, msg.as_ptr() as usize, 13), 13);
        assert_eq!(take_output(), b"Hello, world!");
    }

    #[test]
    fn write_rejects_bad_arguments() {
        let msg = b"x";
        let bad = -(ERROR_INVALID_ARGUMENT as isize);
        assert_eq!(handle_syscall(SYS_WRITE, 2, msg.as_ptr() as usize, 1), bad);
        assert_eq!(handle_syscall(SYS_WRITE, STDOUT, 0, 1), bad);
        assert_eq!(handle_syscall(SYS_WRITE, STDOUT, msg.as_ptr() as usize, MAX_IO_SIZE + 1), bad);
    }

    #[test]
    fn read_stops_at_newline_or_size() {
        INPUT.with(|i| i.borrow_mut().extend(b"hi\nabcdef"));
        let mut buf = [0u8; 16];
        let ptr = buf.as_mut_ptr() as usize;
        assert_eq!(handle_syscall(SYS_READ, STDIN, ptr, 16), 3);
        assert_eq!(&buf[..3], b"hi\n");
        assert_eq!(handle_syscall(SYS_READ, STDIN, ptr, 4), 4);
        assert_eq!(&buf[..4], b"abcd");
    }

    #[test]
    fn exit_and_sleep_report_not_implemented() {
        let not_impl = -(ERROR_NOT_IMPLEMENTED as isize);
        assert_eq!(handle_syscall(SYS_EXIT, 0, 0, 0), not_impl);
        assert_eq!(handle_syscall(SYS_SLEEP, 10, 0, 0), not_impl);
    }

    #[test]
    fn svc_writes_result_into_stacked_r0() {
        take_output();
        let msg = b"svc";
        let mut frame = [SYS_WRITE, STDOUT, msg.as_ptr() as usize, 3, 0, 0, 0, 0];
        let result = unsafe { rust_handle_svc(frame.as_mut_ptr()) };
        assert_eq!(result, 3);
        assert_eq!(frame[0], 3);
        assert_eq!(take_output(), b"svc");
    }
}
