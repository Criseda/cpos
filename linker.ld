ENTRY(Reset_Handler)

MEMORY
{
	FLASH (rx) : ORIGIN = 0x00000000, LENGTH = 512K
	RAM (rwx)  : ORIGIN = 0x20000000, LENGTH = 32K
}

SECTIONS
{
	.vectors :
	{
		*(.vectors)
	} > FLASH

	.text :
	{
		*(.text)
		*(.text*)
	} > FLASH

	.data :
	{
		*(.data)
	} > RAM AT > FLASH

	.bss :
	{
		*(.bss)
	} > RAM
}
