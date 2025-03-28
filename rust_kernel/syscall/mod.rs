// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
//
// Author: Laurentiu Cristian Preda (criseda)
// GitHub: https://github.com/criseda
//
//! System call interface for CPOS
//!
//! This module provides the system call interface for the kernel.

pub mod numbers;
pub mod handler;

pub use handler::handle_syscall;
