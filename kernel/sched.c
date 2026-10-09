/*
 * SPDX-License-Identifier: MIT
 * Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
 *
 * Author: Laurentiu Cristian Preda (criseda)
 * GitHub: https://github.com/criseda
 *
 * Exception entry points for the scheduler
 *
 * The scheduling decisions live in Rust (rust_kernel/task); this file only
 * holds the parts that must be written in assembly: saving and restoring
 * the registers the hardware does not stack, and finding the exception
 * frame.
 */

#include <stdint.h>
#include "rust_interface.h"

/*
 * Context switch. The hardware already pushed r0-r3, r12, lr, pc and xPSR
 * onto the outgoing task's PSP; we push r4-r11 below them, let Rust pick
 * the next task (and program its MPU regions), then pop that task's
 * r4-r11 and return to it in unprivileged thread mode on PSP.
 *
 * Interrupts are masked while Rust runs so SysTick cannot preempt us
 * while we hold the kernel state.
 */
void __attribute__((naked)) PendSV_Handler(void)
{
	__asm volatile(
		"cpsid i\n"
		"mrs r0, psp\n"
		"stmdb r0!, {r4-r11}\n"
		"bl rust_switch_context\n" /* r0 = next task's saved sp */
		"ldmia r0!, {r4-r11}\n"
		"msr psp, r0\n"
		"movs r1, #1\n" /* CONTROL.nPRIV: thread mode is unprivileged */
		"msr control, r1\n"
		"isb\n"
		"ldr lr, =0xFFFFFFFD\n" /* return to thread mode, PSP */
		"cpsie i\n"
		"bx lr\n");
}

/* Console input arrived. The kernel does not read the UART: Rust masks
 * the interrupt and wakes the console server, which does. */
void UART0_Handler(void)
{
	rust_uart_irq();
}

void SysTick_Handler(void)
{
	rust_systick();
}

/*
 * All faults share one entry. EXC_RETURN (lr) says whether a task or the
 * kernel faulted; IPSR says which fault it was. rust_fault returns to the
 * EXC_RETURN we pass through in lr, and by then PendSV is pending, so the
 * dead task never resumes.
 */
#define FAULT_ENTRY(name)                                                   \
	void __attribute__((naked)) name(void)                              \
	{                                                                   \
		__asm volatile("mov r0, lr\n"                               \
			       "mrs r1, ipsr\n"                             \
			       "b rust_fault\n");                           \
	}

FAULT_ENTRY(HardFault_Handler)
FAULT_ENTRY(MemManage_Handler)
FAULT_ENTRY(BusFault_Handler)
FAULT_ENTRY(UsageFault_Handler)

/* A task's entry function returns here; finish the task cleanly */
void task_exit_trampoline(void)
{
	register uint32_t r0 __asm__("r0") = SYS_EXIT;
	register uint32_t r1 __asm__("r1") = 0;
	__asm volatile("svc #0" : "+r"(r0) : "r"(r1) : "memory");
	while (1) {
	}
}
