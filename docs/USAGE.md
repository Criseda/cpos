# Using CPOS

This document explains how to build, run, and develop with CPOS.

## Building the Project

To build CPOS, run:

```bash
make
```

This will:

1. Compile the bootloader assembly
2. Compile C files for the kernel, initialization, and library code
3. Link everything together
4. Generate both ELF and binary files

### Running in QEMU

To run CPOS in the QEMU emulator:

```bash
make qemu
```

This will start QEMU with the appropriate machine configuration (lm3s6965evb) and load CPOS.

### Hardware Considerations

The default configuration targets the lm3s6965evb machine in QEMU, which emulates:

- ARM Cortex-M3 CPU
- 64KB RAM
- UART at base address 0x4000C000

If porting to different hardware, update:

- UART base address in [uart.c](../lib/uart.c)
- Memory layout in [linker.ld](../linker.ld)
- CPU flags in the [Makefile](../Makefile)
