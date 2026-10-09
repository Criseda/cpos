// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
//
// Author: Laurentiu Cristian Preda (criseda)
// GitHub: https://github.com/criseda
//
//! System call numbers for CPOS
//!
//! This module defines the system call numbers used by the kernel.

// File operations
pub const SYS_WRITE: usize = 1;
pub const SYS_READ: usize = 2;

// Process operations
pub const SYS_EXIT: usize = 10;
pub const SYS_SLEEP: usize = 11;

// Memory operations
pub const SYS_ALLOC: usize = 20;
pub const SYS_FREE: usize = 21;

// Define syscall result type
//
// usize/isize match uint32_t/int32_t on the 32-bit target
pub type SyscallResult = Result<usize, usize>;

// Error codes
pub const ERROR_INVALID_SYSCALL: usize = 1;
pub const ERROR_INVALID_ARGUMENT: usize = 2;
pub const ERROR_NOT_IMPLEMENTED: usize = 3;
pub const ERROR_OUT_OF_MEMORY: usize = 4;
pub const ERROR_DOUBLE_FREE: usize = 5;
