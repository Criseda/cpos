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

#endif // RUST_INTERFACE_H
