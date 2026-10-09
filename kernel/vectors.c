/*
 * SPDX-License-Identifier: MIT
 * Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
 *
 * Author: Laurentiu Cristian Preda (criseda)
 * GitHub: https://github.com/criseda
 *
 * Interrupt Vector Table for CPOS
 * 
 * This file sets up the ARM Cortex-M interrupt vector table, defining the
 * entry points for various exception handlers and system interrupts.
 */

#include <stdint.h>

/* Provided by linker.ld */
extern uint32_t _sidata, _sdata, _edata, _sbss, _ebss, _stack_top;

/* Declarations for interrupt handlers */
void Reset_Handler(void);
void Default_Handler(void);
void NMI_Handler(void) __attribute__((weak, alias("Default_Handler")));
void HardFault_Handler(void) __attribute__((weak, alias("Default_Handler")));
void MemManage_Handler(void) __attribute__((weak, alias("Default_Handler")));
void BusFault_Handler(void) __attribute__((weak, alias("Default_Handler")));
void UsageFault_Handler(void) __attribute__((weak, alias("Default_Handler")));
void SVC_Handler(void) __attribute__((weak, alias("Default_Handler")));
void DebugMon_Handler(void) __attribute__((weak, alias("Default_Handler")));
void PendSV_Handler(void) __attribute__((weak, alias("Default_Handler")));
void SysTick_Handler(void) __attribute__((weak, alias("Default_Handler")));
void UART0_Handler(void) __attribute__((weak, alias("Default_Handler")));

/* 
 * Defined section for interrupt vector table
 * Will align with .vectors section in linker script
 */
__attribute__((section(".vectors"))) void (*const g_pfnVectors[])(void) = {
	(void (*)(void))(&_stack_top), 	/* Initial stack pointer value */
	Reset_Handler, 			/* Reset handler */
	NMI_Handler,			/* NMI handler */
	HardFault_Handler, 		/* Hard fault handler */
	MemManage_Handler, 		/* Memory management fault */
	BusFault_Handler, 		/* Bus fault */
	UsageFault_Handler, 		/* Usage fault */
	0, 				/* Reserved */
	0, 				/* Reserved */
	0, 				/* Reserved */
	0, 				/* Reserved */
	SVC_Handler, 			/* SVCall */
	DebugMon_Handler, 		/* Debug monitor */
	0, 				/* Reserved */
	PendSV_Handler, 		/* PendSV */
	SysTick_Handler, 		/* SysTick */

	/* External interrupts (LM3S6965); only UART0 is used */
	Default_Handler, 		/* IRQ 0: GPIO port A */
	Default_Handler, 		/* IRQ 1: GPIO port B */
	Default_Handler, 		/* IRQ 2: GPIO port C */
	Default_Handler, 		/* IRQ 3: GPIO port D */
	Default_Handler, 		/* IRQ 4: GPIO port E */
	UART0_Handler 			/* IRQ 5: UART0 */
};

void Default_Handler(void)
{
	while (1)
		;
}

extern void init(void);
void Reset_Handler(void)
{
	/* Copy initialised data from flash and zero .bss before any C or
	 * Rust code relies on its statics */
	uint32_t *src = &_sidata;
	uint32_t *dst = &_sdata;
	while (dst < &_edata) {
		*dst++ = *src++;
	}
	for (dst = &_sbss; dst < &_ebss;) {
		*dst++ = 0;
	}

	/* Call the init function */
	init();

	/* Should never return */
	while (1)
		;
}
