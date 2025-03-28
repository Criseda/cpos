.syntax unified
.cpu cortex-m3
.thumb

.section .text
.thumb_func
_start:
	/* Setup RAM pattern to verify execution */ 
	ldr r0, =0x20000000
	ldr r1, =0xDEADBEEF
	str r1, [r0]

	/* Setup stack */ 
	ldr r0, =0x20007F00
	mov sp, r0

	/* Call init function */
	bl init

	/* Infinite loop if init returns */
	b .

.thumb_func
exception_handler:
	b exception_handler    /* Loop forever */
