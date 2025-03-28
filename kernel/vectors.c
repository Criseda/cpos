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

/* Declarations for interrupt handlers */
void Reset_Handler(void);
void NMI_Handler(void) __attribute__((weak, alias("Default_Handler")));
void HardFault_Handler(void) __attribute__((weak, alias("Default_Handler")));
void MemManage_Handler(void) __attribute__((weak, alias("Default_Handler")));
void BusFault_Handler(void) __attribute__((weak, alias("Default_Handler")));
void UsageFault_Handler(void) __attribute__((weak, alias("Default_Handler")));
void SVC_Handler(void) __attribute__((weak, alias("Default_Handler")));
void DebugMon_Handler(void) __attribute__((weak, alias("Default_Handler")));
void PendSV_Handler(void) __attribute__((weak, alias("Default_Handler")));
void SysTick_Handler(void) __attribute__((weak, alias("Default_Handler")));

/* 
 * Defined section for interrupt vector table
 * Will align with .vectors section in linker script
 */
__attribute__((section(".vectors"))) void (*const g_pfnVectors[])(void) = {
	(void (*)(void))(0x20007F00), 	/* Initial stack pointer value */
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
	SysTick_Handler 		/* SysTick */
};

void Default_Handler(void)
{
	while (1)
		;
}

extern void init(void);
void Reset_Handler(void)
{
	/* Call the init function */
	init();

	/* Should never return */
	while (1)
		;
}
