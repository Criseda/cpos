// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
//
// Author: Laurentiu Cristian Preda (criseda)
// GitHub: https://github.com/criseda
//
//! System call handler implementation
//!
//! This module handles system call dispatching and implementation.

use crate::syscall::numbers::*;

extern "C" {
    fn uart_send_string(s: *const u8);
}

// Helper to send debug messages
fn debug_print(msg: &str) {
    unsafe {
        uart_send_string(msg.as_ptr());
    }
}

/// Handle write syscall
fn sys_write(fd: u32, buffer: u32, size: u32) -> SyscallResult {
    // Only support stdout (fd=1) for now
    if fd == 1 {
        // Safety checks
        if buffer == 0 || size > 1024 {
            return Err(ERROR_INVALID_ARGUMENT);
        }

        // Write to UART (our stdout)
        let buf_ptr = buffer as *const u8;
        unsafe {
            // Make sure not to read past the end of the buffer
            let slice = core::slice::from_raw_parts(buf_ptr, size as usize);
            
            // Treat as UTF-8 string with error handling
            if let Ok(s) = core::str::from_utf8(slice) {
                uart_send_string(s.as_ptr());
                return Ok(size);
            }
        }
    }
    Err(ERROR_INVALID_ARGUMENT)
}

/// Handle memory allocation syscall
fn sys_alloc(size: u32) -> SyscallResult {
    if size == 0 {
        return Err(ERROR_INVALID_ARGUMENT);
    }
    
    let ptr = crate::memory::alloc(size as usize);
    if ptr.is_null() {
        Err(ERROR_INVALID_ARGUMENT)
    } else {
        Ok(ptr as u32)
    }
}

/// Handle memory free syscall
fn sys_free(ptr: u32) -> SyscallResult {
    if ptr == 0 {
        return Err(ERROR_INVALID_ARGUMENT);
    }
    
    unsafe {
        crate::memory::free(ptr as *mut u8);
    }
    Ok(0)
}

/// Main syscall dispatcher
pub fn handle_syscall(number: u32, arg1: u32, arg2: u32, arg3: u32) -> i32 {
    let result = match number {
        SYS_WRITE => sys_write(arg1, arg2, arg3),
        SYS_ALLOC => sys_alloc(arg1),
        SYS_FREE => sys_free(arg1),
        _ => {
            debug_print("Unknown syscall\n");
            Err(ERROR_INVALID_SYSCALL)
        }
    };
    
    // Convert Result to i32 return value
    match result {
        Ok(val) => val as i32,
        Err(err) => -(err as i32)  // Negative values for errors
    }
}

/// SVC handler implementation called from assembly
#[no_mangle]
pub extern "C" fn rust_handle_svc(svc_number: u32, args: *const u32) -> i32 {
    // Read arguments from stack
    let arg1 = unsafe { *args.offset(1) }; // R1
    let arg2 = unsafe { *args.offset(2) }; // R2
    let arg3 = unsafe { *args.offset(3) }; // R3
    
    handle_syscall(svc_number, arg1, arg2, arg3)
}
