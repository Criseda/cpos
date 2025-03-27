# Installation Requirements for CPOS

This document outlines all prerequisites needed to build and run CPOS, a bare-metal operating system for ARM Cortex-M3 processors.

## Prerequisites

### Required Software

- **ARM GCC Toolchain**: Cross-compilation toolchain for ARM processors
- **QEMU**: System emulator for ARM architecture
- **Make**: Build automation tool
- **Git**: Version control system (for obtaining the project)

### Installing on Ubuntu/Debian

```bash
# Update package lists
sudo apt update

# Install ARM GCC Toolchain
sudo apt install gcc-arm-none-eabi binutils-arm-none-eabi

# Install QEMU for ARM
sudo apt install qemu-system-arm

# Install Make and Git
sudo apt install make git
```

### Installing on Arch Linux

```bash
# Update package lists
sudo pacman -Syu

# Install ARM GCC Toolchain
sudo pacman -S arm-none-eabi-gcc arm-none-eabi-binutils

# Install QEMU for ARM
sudo pacman -S qemu-arch-extra

# Install Make and Git
sudo pacman -S make git
```

### Installing on MacOS (Using Homebrew)

```bash
# Install Homebrew if not already installed
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install ARM GCC Toolchain
brew install --cask gcc-arm-embedded

# Install QEMU for ARM
brew install qemu

# Install Make and Git (if not already installed)
brew install make git
```

### Installing on Windows

Download WSL2, then use the Ubuntu guide.

### Verifying Installation

Verify your installation by checking that the following commands work:

```bash
arm-none-eabi-gcc --version
qemu-system-arm --version
make --version
git --version
```

### Next Steps

Once you have installed all prerequisites, see [USAGE.md](USAGE.md) for instructions on building and running CPOS.
