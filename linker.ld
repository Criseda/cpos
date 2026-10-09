/*
 * CPOS Linker Script
 *
 * Author: Laurentiu Cristian Preda (criseda)
 *
 * RAM layout (MPU regions must be power-of-two sized and size-aligned):
 *
 *   0x20000000 - 0x20001FFF  kernel .data and .bss (8K)
 *   0x20002000 - 0x20002FFF  kernel heap (4K)
 *   0x20003000 - 0x20006FFF  task slots, 8 x 2K (heap low, stack high)
 *   0x20007000 - 0x20007FFF  kernel stack (MSP)
 */

ENTRY(Reset_Handler)

MEMORY
{
	FLASH (rx)    : ORIGIN = 0x00000000, LENGTH = 512K
	KDATA (rwx)   : ORIGIN = 0x20000000, LENGTH = 8K
}

_heap_start  = 0x20002000;
_heap_size   = 0x1000;
_task_slots  = 0x20003000;
_stack_top   = 0x20008000;

SECTIONS
{
	.vectors :
	{
		KEEP(*(.vectors))
	} > FLASH

	.text :
	{
		*(.text)
		*(.text*)
		*(.rodata)
		*(.rodata*)
	} > FLASH

	.data :
	{
		. = ALIGN(4);
		_sdata = .;
		*(.data)
		*(.data*)
		. = ALIGN(4);
		_edata = .;
	} > KDATA AT > FLASH

	_sidata = LOADADDR(.data);
	_flash_end = LOADADDR(.data) + SIZEOF(.data);

	.bss (NOLOAD) :
	{
		. = ALIGN(4);
		_sbss = .;
		*(.bss)
		*(.bss*)
		*(COMMON)
		. = ALIGN(4);
		_ebss = .;
	} > KDATA

	/DISCARD/ :
	{
		*(.ARM.exidx*)
		*(.ARM.extab*)
	}
}

ASSERT(_ebss <= 0x20002000, "kernel data overflows into the heap")
