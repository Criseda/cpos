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
pub const SYS_WRITE: u32 = 1;
pub const SYS_READ: u32 = 2;

// Process operations
pub const SYS_EXIT: u32 = 10;
pub const SYS_SLEEP: u32 = 11;

// Memory operations
pub const SYS_ALLOC: u32 = 20;
pub const SYS_FREE: u32 = 21;

// Define syscall result type
pub type SyscallResult = Result<u32, u32>;

// Error codes
pub const ERROR_INVALID_SYSCALL: u32 = 1;
pub const ERROR_INVALID_ARGUMENT: u32 = 2;
pub const ERROR_NOT_IMPLEMENTED: u32 = 3;
