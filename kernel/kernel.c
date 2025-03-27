#include <stdint.h>
#include "uart.h"

void main(void)
{
	// Print a message to UART
	uart_send_string("Kernel: Hello, World!\n");

	// Infinite loop to keep the kernel running
	while (1) {
	}
}
