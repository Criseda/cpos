/*
 * SPDX-License-Identifier: MIT
 * Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
 *
 * Author: Laurentiu Cristian Preda (criseda)
 * GitHub: https://github.com/criseda
 *
 * User-mode system call wrappers for CPOS tasks
 *
 * Tasks run unprivileged and may only touch their own stack, their own
 * heap (SYS_ALLOC) and flash. Everything else goes through these calls.
 */

#ifndef USER_H
#define USER_H

#include <stdint.h>
#include "rust_interface.h"

static inline int32_t sys_call(uint32_t number, uint32_t arg1, uint32_t arg2,
			       uint32_t arg3)
{
	register uint32_t r0 __asm__("r0") = number;
	register uint32_t r1 __asm__("r1") = arg1;
	register uint32_t r2 __asm__("r2") = arg2;
	register uint32_t r3 __asm__("r3") = arg3;

	__asm volatile("svc #0"
		       : "+r"(r0)
		       : "r"(r1), "r"(r2), "r"(r3)
		       : "memory");
	return (int32_t)r0;
}

static inline int32_t sys_write(const void *buf, uint32_t len)
{
	return sys_call(SYS_WRITE, 1, (uint32_t)buf, len);
}

static inline void sys_exit(uint32_t code)
{
	sys_call(SYS_EXIT, code, 0, 0);
}

static inline int32_t sys_sleep(uint32_t ms)
{
	return sys_call(SYS_SLEEP, ms, 0, 0);
}

static inline int32_t sys_yield(void)
{
	return sys_call(SYS_YIELD, 0, 0, 0);
}

static inline uint32_t sys_ticks(void)
{
	return (uint32_t)sys_call(SYS_TICKS, 0, 0, 0);
}

static inline int32_t sys_alloc(uint32_t size)
{
	return sys_call(SYS_ALLOC, size, 0, 0);
}

static inline int32_t sys_free(int32_t ptr)
{
	return sys_call(SYS_FREE, (uint32_t)ptr, 0, 0);
}

static inline int32_t sys_send(uint32_t dest, const void *buf, uint32_t len)
{
	return sys_call(SYS_SEND, dest, (uint32_t)buf, len);
}

static inline int32_t sys_recv(void *buf, uint32_t max, uint32_t *from)
{
	return sys_call(SYS_RECV, (uint32_t)buf, max, (uint32_t)from);
}

/* Tasks started by the kernel (user/tasks.c) */
void idle_task(uint32_t arg);
void console_task(uint32_t arg);
void register_user_tasks(void);

#endif /* USER_H */
