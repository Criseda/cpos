#include <stdint.h>
#include "uart.h"
#include "kernel.h"

extern void main(void);

void init(void)
{
	// Initialize UART with a baud rate of 115200
	uart_init(115200);

	// Print a message to UART
	uart_send_string("Init: Hello, World!\n");

	// Call the main function of the kernel
	main();

	// Infinite loop to keep the init process running
	while (1) {
	}
}
