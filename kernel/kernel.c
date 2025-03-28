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
		"mrseq r1, msp\n" // If bit 2 is clear, use MSP (note: r1 not r0)
		"mrsne r1, psp\n" // If bit 2 is set, use PSP (note: r1 not r0)
		"push {lr}\n" // Save link register
		// r0 already contains syscall number, r1 has stack pointer
		"bl rust_handle_svc\n" // Call Rust handler
		"pop {pc}\n" // Return
	);
}

void syscall_test(void)
{
	uart_send_string("[TEST] System Call Interface Test\n");

	// Test write syscall
	const char *test_str = "Hello from syscall!\n";
	int result = rust_syscall(SYS_WRITE, 1, (uint32_t)test_str, 19);
	if (result == 19) {
		uart_send_string("  - SYS_WRITE: OK\n");
	} else {
		uart_send_string("  - SYS_WRITE: FAILED\n");
	}

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
		if (result == 0) {
			uart_send_string("  - SYS_FREE: OK\n");
		} else {
			uart_send_string("  - SYS_FREE: FAILED\n");
		}
	} else {
		uart_send_string("  - SYS_ALLOC: FAILED\n");
	}

	// Test invalid syscall number
	result = rust_syscall(999, 0, 0, 0);
	if (result < 0) {
		uart_send_string("  - Invalid syscall handling: OK\n");
	} else {
		uart_send_string("  - Invalid syscall handling: FAILED\n");
	}

	// Test direct SVC instruction
	uart_send_string("  - Testing SVC instruction directly:\n");
	// Create a test message
	const char *direct_msg = "Test from direct SVC!\n";

	// Use the SVC instruction with proper arguments on stack
	__asm volatile(
		"mov r0, #1\n" // SYS_WRITE syscall number
		"mov r1, #1\n" // fd = 1 (stdout)
		"ldr r2, %[msg]\n" // buffer address
		"mov r3, #20\n" // length of message
		"svc #0\n" // SVC instruction with 0 (we extract real syscall # from r0)
		:
		: [msg] "m"(direct_msg) // Input constraints
		: "r0", "r1", "r2", "r3", "memory" // Clobber list
	);

	uart_send_string("[TEST] System Call Interface Test Complete\n");
}

void memory_test(void)
{
	uart_send_string("[TEST] Memory Allocator Test\n");

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
