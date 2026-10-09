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
int32_t rust_heap_free(void *ptr); /* 0 ok, -1 invalid pointer, -2 double free */
size_t rust_heap_free_bytes(void);
size_t rust_heap_free_blocks(void);

// Syscall interface
int32_t rust_syscall(uint32_t number, uint32_t arg1, uint32_t arg2, uint32_t arg3);
void rust_handle_svc(uint32_t *frame);

// Scheduler (rust_kernel/task)
int32_t rust_task_register(const char *name, void (*entry)(uint32_t), uint32_t arg,
			   uint32_t flags); /* task id, or -1 */
int32_t rust_sched_start(uintptr_t slot_base, uintptr_t flash_end,
			 void (*exit_trampoline)(void), uint32_t tick_reload);
uint32_t rust_switch_context(uint32_t saved_sp);
void rust_systick(void);
void rust_fault(uint32_t exc_return, uint32_t exception);

#define TASK_IDLE         (1u << 0) /* runs when nothing else can */
#define TASK_CONSOLE      (1u << 1) /* owns the UART, prints for everyone */
#define TASK_EXPECT_FAULT (1u << 2) /* test task that must be killed by a fault */

#define TICK_MS 10

// System call numbers
#define SYS_WRITE 1
#define SYS_READ  2
#define SYS_EXIT  10
#define SYS_SLEEP 11
#define SYS_YIELD 12
#define SYS_TICKS 13
#define SYS_ALLOC 20
#define SYS_FREE  21
#define SYS_SEND  30
#define SYS_RECV  31

// System call error codes (returned negated)
#define ERROR_INVALID_SYSCALL  1
#define ERROR_INVALID_ARGUMENT 2
#define ERROR_NOT_IMPLEMENTED  3
#define ERROR_OUT_OF_MEMORY    4
#define ERROR_DOUBLE_FREE      5
#define ERROR_BAD_ADDRESS      6 /* pointer outside the caller's memory */
#define ERROR_NO_TASK          7 /* no such task, or no calling task */

#endif // RUST_INTERFACE_H
