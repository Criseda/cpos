#include <stdint.h>
#include "uart.h"
#include "vectors.h"

void SVC_Handler(void)
{
	uart_send_string("[SUCCESS] SVC Handler Invoked!\n");
}

void main(void)
{
	/* Print a message to UART */
	uart_send_string("Kernel: Hello, World!\n");

	/* TESTS */
	uart_send_string("[TEST] test_svc_handler\n");
	__asm volatile("svc #0");

	/* Infinite loop to keep the kernel running */
	while (1) {
	}
}
