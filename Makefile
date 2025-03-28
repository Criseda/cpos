# SPDX-License-Identifier: MIT
# Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
#
# Author: Laurentiu Cristian Preda (criseda)
# GitHub: https://github.com/criseda
#
# Makefile for CPOS
# 
# This Makefile builds the CPOS operating system, including both C and Rust components.
# It handles compilation, linking, and provides targets for running the system in QEMU.

# Target binary name
TARGET = cpos.elf

# Toolchain
CC = arm-none-eabi-gcc
AS = arm-none-eabi-as
LD = arm-none-eabi-ld
OBJCOPY = arm-none-eabi-objcopy

# Directories
BOOTLOADER_DIR = bootloader
KERNEL_DIR = kernel
INCLUDE_DIR = include
LIB_DIR = lib
INIT_DIR = init
RUST_DIR = rust_kernel

# Rust settings
RUST_TARGET = thumbv7m-none-eabi
RUST_LIB = $(RUST_DIR)/target/$(RUST_TARGET)/release/librust_kernel.a

# Source files
BOOTLOADER_SRC = $(BOOTLOADER_DIR)/bootloader.s
KERNEL_SRC = $(KERNEL_DIR)/kernel.c $(KERNEL_DIR)/vectors.c
INIT_SRC = $(INIT_DIR)/init.c
LIB_SRC = $(wildcard $(LIB_DIR)/*.c)

# Object files
BOOTLOADER_OBJ = $(BOOTLOADER_SRC:.s=.o)
KERNEL_OBJ = $(KERNEL_SRC:.c=.o)
INIT_OBJ = $(INIT_SRC:.c=.o)
LIB_OBJ = $(LIB_SRC:.c=.o)

# Linker script
LD_SCRIPT = linker.ld

# Assembler flags
ASFLAGS = -mcpu=cortex-m3 -mthumb

# Compiler flags
CFLAGS = $(ASFLAGS) -nostdlib -ffreestanding -Wall -Wextra -I$(INCLUDE_DIR) -Os
LDFLAGS = -T $(LD_SCRIPT) -nostdlib

# QEMU settings
QEMU = qemu-system-arm
QEMU_MCU = cortex-m3
QEMU_MACHINE = lm3s6965evb
QEMU_ARGS = -machine $(QEMU_MACHINE) -cpu $(QEMU_MCU) -nographic -monitor null -serial stdio
QEMU_DEBUG = -d in_asm,cpu_reset > qemu_debug.log 2>&1
QEMU_GDB_DEBUG = -nographic -s -S

# Build Rules
all: $(TARGET)

# Pattern rules for building object files
%.o: %.c
	$(CC) $(CFLAGS) -c -o $@ $<

%.o: %.s
	$(AS) $(ASFLAGS) -o $@ $<

# Rule to build the Rust library
$(RUST_LIB):
	@echo "Building Rust components..."
	@cd $(RUST_DIR) && rustup target add $(RUST_TARGET) 2>/dev/null || true
	@cd $(RUST_DIR) && cargo build --release --target $(RUST_TARGET)

$(TARGET): $(BOOTLOADER_OBJ) $(KERNEL_OBJ) $(INIT_OBJ) $(LIB_OBJ) $(RUST_LIB)
	$(CC) $(ASFLAGS) -nostdlib -ffreestanding -T $(LD_SCRIPT) -Wl,--no-warn-mismatch -o $@ $^
	$(OBJCOPY) -O binary $@ cpos.bin

# QEMU target
qemu:
	@echo "Starting QEMU. To exit, press Ctrl+C"
	$(QEMU) $(QEMU_ARGS) -kernel $(TARGET)

qemu-gdb:
	@echo "Starting QEMU with GDB server. Connect with 'arm-none-eabi-gdb -ex \"target remote localhost:1234\"'"
	$(QEMU) $(QEMU_ARGS) -kernel $(TARGET) $(QEMU_GDB_DEBUG)

qemu-debug:
	@echo "Starting QEMU with debug logging. Check qemu_debug.log for details."
	$(QEMU) $(QEMU_ARGS) -kernel $(TARGET) $(QEMU_DEBUG)

clean:
	rm -f $(BOOTLOADER_OBJ) $(KERNEL_OBJ) $(INIT_OBJ) $(LIB_OBJ) $(TARGET) cpos.bin

.PHONY: all clean qemu qemu-gdb qemu-debug 
