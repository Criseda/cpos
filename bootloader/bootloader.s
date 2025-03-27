.syntax unified
.cpu cortex-m3
.thumb

// Vector Table
.section .vectors, "ax"
.align 2
.global _vector_table
_vector_table:
	.word 0x20007F00      		// Initial stack pointer value
	.word _start + 1      		// Reset vector (+1 for thumb mode)
	.word exception_handler + 1  	// NMI
	.word exception_handler + 1  	// Hard Fault
	.word exception_handler + 1  	// MemManage Fault
	.word exception_handler + 1  	// Bus Fault
	.word exception_handler + 1 	// Usage Fault
	.word 0               		// Reserved
	.word 0               		// Reserved
	.word 0               		// Reserved
	.word 0               		// Reserved
	.word exception_handler + 1 	// SVCall
	.word exception_handler + 1  	// Debug Monitor
	.word 0               		// Reserved
	.word exception_handler + 1  	// PendSV
	.word exception_handler + 1  	// SysTick

.section .text
.thumb_func
_start:
	// Setup RAM pattern to verify execution
	ldr r0, =0x20000000
	ldr r1, =0xDEADBEEF
	str r1, [r0]

	// Setup stack
	ldr r0, =0x20007F00
	mov sp, r0

	// Call init function
	bl init

	// Infinite loop if init returns
	b .

.thumb_func
exception_handler:
	b exception_handler    // Loop forever
