#include "uart.h"

#define UART0_BASE 0x4000C000 /* LM3S6965 UART0 base address */ 
#define UART0_DR *((volatile uint32_t *)(UART0_BASE + 0x00))
#define UART0_FR *((volatile uint32_t *)(UART0_BASE + 0x18))

void uart_init(uint32_t baudrate)
{
	/* For QEMU, no real initialization is needed for PL011 UART */
	(void)baudrate; /* Suppress unused variable warning */
}

void uart_send_char(char c)
{
	/* Wait until UART is ready to transmit */
	while (UART0_FR & (1 << 5)) {
	}
	/* Write character to UART Data Register */
	UART0_DR = c;
}

void uart_send_string(const char *str)
{
	/* Send the actual string */
	while (*str) {
		uart_send_char(*str++);
	}
}

char uart_receive_char(void)
{
	/* Wait until there's data to read */
	while (UART0_FR & (1 << 4)) {
	}
	return (char)UART0_DR;
}
