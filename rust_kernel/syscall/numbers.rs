// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
//
// Author: Laurentiu Cristian Preda (criseda)
// GitHub: https://github.com/criseda
//
//! System call numbers for CPOS
//!
//! This module defines the system call numbers used by the kernel.
//! Keep in sync with `include/rust_interface.h`.

// File operations
pub const SYS_WRITE: usize = 1;
pub const SYS_READ: usize = 2;

// Process operations
pub const SYS_EXIT: usize = 10;
pub const SYS_SLEEP: usize = 11;
pub const SYS_YIELD: usize = 12;
pub const SYS_TICKS: usize = 13;

// Memory operations
pub const SYS_ALLOC: usize = 20;
pub const SYS_FREE: usize = 21;

// Message passing
pub const SYS_SEND: usize = 30;
pub const SYS_RECV: usize = 31;

// Define syscall result type
//
// usize/isize match uint32_t/int32_t on the 32-bit target
pub type SyscallResult = Result<usize, usize>;

// Error codes (returned negated)
pub const ERROR_INVALID_SYSCALL: usize = 1;
pub const ERROR_INVALID_ARGUMENT: usize = 2;
pub const ERROR_NOT_IMPLEMENTED: usize = 3;
pub const ERROR_OUT_OF_MEMORY: usize = 4;
pub const ERROR_DOUBLE_FREE: usize = 5;
/// A pointer argument is outside the memory the calling task owns
pub const ERROR_BAD_ADDRESS: usize = 6;
/// No such task, or the call needs a calling task and there is none
pub const ERROR_NO_TASK: usize = 7;
