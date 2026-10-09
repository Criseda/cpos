/*
 * SPDX-License-Identifier: MIT
 * Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
 *
 * Author: Laurentiu Cristian Preda (criseda)
 * GitHub: https://github.com/criseda
 *
 * UART implementation for CPOS
 * 
 * This file provides UART (Universal Asynchronous Receiver/Transmitter)
 * functionality for serial communication.
 */

#include "uart.h"

#define UART0_BASE 0x4000C000 /* LM3S6965 UART0 base address */
#define UART0_DR *((volatile uint32_t *)(UART0_BASE + 0x00))
#define UART0_FR *((volatile uint32_t *)(UART0_BASE + 0x18))

/*
 * Once tasks run, the console server owns the UART and the kernel should
 * never write to it. These let the kernel prove that: after the scheduler
 * starts, every byte sent from privileged code is counted.
 */
static volatile uint32_t scheduler_started;
static volatile uint32_t privileged_writes;

void uart_mark_scheduler_started(void)
{
	scheduler_started = 1;
}

uint32_t uart_privileged_writes(void)
{
	return privileged_writes;
}

static int running_privileged(void)
{
	uint32_t ipsr, control;
	__asm volatile("mrs %0, ipsr" : "=r"(ipsr));
	__asm volatile("mrs %0, control" : "=r"(control));
	/* Handler mode is always privileged; thread mode is unless nPRIV */
	return ipsr != 0 || (control & 1) == 0;
}

void uart_init(uint32_t baudrate)
{
	/* For QEMU, no real initialization is needed for PL011 UART */
	(void)baudrate; /* Suppress unused variable warning */
}

void uart_send_char(char c)
{
	if (scheduler_started && running_privileged()) {
		privileged_writes++;
	}
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
