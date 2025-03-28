/*
 * SPDX-License-Identifier: MIT
 * Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
 *
 * Author: Laurentiu Cristian Preda (criseda)
 * GitHub: https://github.com/criseda
 *
 * Rust Interface for CPOS
 * 
 * This header file declares the interface functions provided by the Rust
 * components for use in the C kernel code.
 */

#ifndef RUST_INTERFACE_H
#define RUST_INTERFACE_H

#include <stdint.h>
#include <stddef.h>

// Memory management functions provided by Rust
void rust_init_heap(uintptr_t heap_start, size_t heap_size);
void *rust_heap_alloc(size_t size);
void rust_heap_free(void *ptr);

// Syscall interface
int32_t rust_syscall(uint32_t number, uint32_t arg1, uint32_t arg2, uint32_t arg3);
int32_t rust_handle_svc(uint32_t svc_number, uint32_t *args);

// System call numbers
#define SYS_WRITE 1
#define SYS_READ  2
#define SYS_EXIT  10
#define SYS_SLEEP 11
#define SYS_ALLOC 20
#define SYS_FREE  21

#endif // RUST_INTERFACE_H
