/*
 * SPDX-License-Identifier: MIT
 * Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
 *
 * Author: Laurentiu Cristian Preda (criseda)
 * GitHub: https://github.com/criseda
 *
 * Main Kernel Implementation for CPOS
 * 
 * This file contains the main kernel entry point and core functionality
 * including memory management tests and system call handling.
 */

#include <stdint.h>
#include "uart.h"
#include "vectors.h"
#include "rust_interface.h"

#define HEAP_START 0x20001000
#define HEAP_SIZE 0x6000 // 24KB

void __attribute__((naked)) SVC_Handler(void)
{
	__asm volatile(
		"tst lr, #4\n" // Test bit 2 of EXC_RETURN to determine stack used
		"ite eq\n" // If-Then-Else block
		"mrseq r0, msp\n" // If bit 2 is clear, the frame is on MSP
		"mrsne r0, psp\n" // If bit 2 is set, the frame is on PSP
		"push {r4, lr}\n" // Save EXC_RETURN, keep the stack 8-byte aligned
		// r0 = exception frame; Rust reads the syscall number and
		// arguments from it and writes the result to the stacked r0
		"bl rust_handle_svc\n" // Call Rust handler
		"pop {r4, pc}\n" // Exception return
	);
}

/* Issue a system call through the SVC instruction; the result comes back in r0 */
static int32_t svc_call(uint32_t number, uint32_t arg1, uint32_t arg2,
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

static void report(const char *name, int ok)
{
	uart_send_string(name);
	uart_send_string(ok ? ": OK\n" : ": FAILED\n");
}

void syscall_test(void)
{
	uart_send_string("[TEST] System Call Interface Test\n");

	// Test write syscall
	const char *test_str = "Hello from syscall!\n";
	int result = rust_syscall(SYS_WRITE, 1, (uint32_t)test_str, 20);
	report("  - SYS_WRITE", result == 20);

	// Test memory allocation through syscall
	uint32_t ptr = 0;
	result = rust_syscall(SYS_ALLOC, 128, 0, 0);
	if (result > 0) {
		ptr = (uint32_t)result;
		uart_send_string("  - SYS_ALLOC: OK\n");

		// Try to write to the allocated memory
		uint8_t *mem = (uint8_t *)ptr;
		for (int i = 0; i < 10; i++) {
			mem[i] = i + 1;
		}

		// Verify the memory writes
		int error = 0;
		for (int i = 0; i < 10; i++) {
			if (mem[i] != i + 1) {
				error = 1;
				break;
			}
		}

		if (!error) {
			uart_send_string(
				"  - Memory access through syscall: OK\n");
		} else {
			uart_send_string(
				"  - Memory access through syscall: FAILED\n");
		}

		// Free memory through syscall
		result = rust_syscall(SYS_FREE, ptr, 0, 0);
		report("  - SYS_FREE", result == 0);

		// Freeing the same pointer again must be rejected
		result = rust_syscall(SYS_FREE, ptr, 0, 0);
		report("  - SYS_FREE double free rejected",
		       result == -ERROR_DOUBLE_FREE);
	} else {
		uart_send_string("  - SYS_ALLOC: FAILED\n");
	}

	// Test invalid syscall number
	result = rust_syscall(999, 0, 0, 0);
	report("  - Invalid syscall handling", result == -ERROR_INVALID_SYSCALL);

	// Exit and sleep need the scheduler; they must say so explicitly
	report("  - SYS_EXIT not implemented",
	       rust_syscall(SYS_EXIT, 0, 0, 0) == -ERROR_NOT_IMPLEMENTED);
	report("  - SYS_SLEEP not implemented",
	       rust_syscall(SYS_SLEEP, 10, 0, 0) == -ERROR_NOT_IMPLEMENTED);

	// Test the SVC instruction path, including the return value in r0
	const char *direct_msg = "Test from direct SVC!\n";
	result = svc_call(SYS_WRITE, 1, (uint32_t)direct_msg, 22);
	report("  - SVC SYS_WRITE return value", result == 22);

	result = svc_call(SYS_ALLOC, 64, 0, 0);
	report("  - SVC SYS_ALLOC", result > 0);
	if (result > 0) {
		report("  - SVC SYS_FREE",
		       svc_call(SYS_FREE, (uint32_t)result, 0, 0) == 0);
	}

	result = svc_call(999, 0, 0, 0);
	report("  - SVC invalid syscall", result == -ERROR_INVALID_SYSCALL);

	uart_send_string("[TEST] System Call Interface Test Complete\n");
}

void memory_test(void)
{
	uart_send_string("[TEST] Memory Allocator Test\n");

	size_t free_before = rust_heap_free_bytes();

	/* Allocate memory */
	uint32_t *block1 = (uint32_t *)rust_heap_alloc(sizeof(uint32_t) * 10);
	if (block1) {
		uart_send_string("  - Allocated block1: OK\n");

		/* Write to the memory */
		for (int i = 0; i < 10; i++) {
			block1[i] = 0xAA000000 + i;
		}

		/* Read back to verify */
		int error = 0;
		for (int i = 0; i < 10; i++) {
			if (block1[i] != 0xAA000000 + i) {
				error = 1;
				break;
			}
		}

		if (!error) {
			uart_send_string("  - Memory write/read: OK\n");
		} else {
			uart_send_string("  - Memory write/read: FAILED\n");
		}

		/* Free the memory */
		rust_heap_free((void *)block1);
		uart_send_string("  - Freed block1\n");
	} else {
		uart_send_string("  - Allocation failed\n");
	}

	/* Test multiple allocations and frees */
	void *blocks[5];
	uart_send_string("  - Multiple allocation test:\n");

	for (int i = 0; i < 5; i++) {
		blocks[i] = rust_heap_alloc(1024); /* 1KB blocks */
		if (blocks[i]) {
			uart_send_string("    - Block allocated\n");
		} else {
			uart_send_string("    - Allocation failed\n");
		}
	}

	/* Free in reverse order */
	for (int i = 4; i >= 0; i--) {
		if (blocks[i]) {
			rust_heap_free(blocks[i]);
			uart_send_string("    - Block freed\n");
		}
	}

	/* Many alloc/free cycles of mixed sizes, interleaved */
	for (int round = 0; round < 1000; round++) {
		void *a = rust_heap_alloc(1 + (round % 97));
		void *b = rust_heap_alloc(1 + (round % 513));
		void *c = rust_heap_alloc(1 + (round % 31));
		rust_heap_free(b);
		rust_heap_free(a);
		rust_heap_free(c);
	}

	/* Everything freed: heap must be back to one block of the same size */
	report("  - No bytes leaked", rust_heap_free_bytes() == free_before);
	report("  - Free list fully coalesced", rust_heap_free_blocks() == 1);

	uart_send_string("[TEST] Memory Allocator Test Complete\n");
}

void main(void)
{
	/* Print a message to UART */
	uart_send_string("Kernel: Hello, World!\n");

	/* Initialize Rust heap allocator */
	uart_send_string("Initializing Rust heap allocator...\n");
	rust_init_heap(HEAP_START, HEAP_SIZE);

	/* TESTS */
	memory_test();
	syscall_test();

	/* Infinite loop to keep the kernel running */
	while (1) {
	}
}
