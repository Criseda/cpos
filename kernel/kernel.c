#include <stdint.h>
#include "uart.h"
#include "vectors.h"
#include "rust_interface.h"

#define HEAP_START 0x20001000
#define HEAP_SIZE 0x6000 // 24KB

void SVC_Handler(void)
{
	uart_send_string("[SUCCESS] SVC Handler Invoked!\n");
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
	/* svc test */
	uart_send_string("[TEST] test_svc_handler\n");
	__asm volatile("svc #0");
	/* memory test */
	memory_test();

	/* Infinite loop to keep the kernel running */
	while (1) {
	}
}
