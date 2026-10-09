# Running CPOS on Hardware

CPOS is developed and tested on QEMU's `lm3s6965evb` machine. It has not
been run on a physical board yet. This page lists what QEMU lets the code
skip, what to change for real silicon, and what to check once it runs.

## Boards

| Board | Fit |
|-------|-----|
| TI EK-LM3S6965 evaluation kit | The chip QEMU emulates. Same memory map, UART0 and MPU; needs only clock and UART setup (below). Its on-board debug interface gives JTAG and a USB serial port wired to UART0. Discontinued, but available second-hand. |
| Other Stellaris LM3S boards with a Cortex-M3 and MPU | Same peripherals; check flash and RAM sizes against `linker.ld`. |
| STM32F103 ("Blue Pill" and similar) | Cheap Cortex-M3 with an MPU, but a port: different UART (USART1 at `0x40013800`, different registers), clock tree and 20 KB of RAM, so the RAM layout and the console server's register access change. |

## What QEMU lets the code skip

These all work on QEMU without any setup and need code on a real chip:

1. **System clock.** After reset the LM3S6965 runs from its internal
   oscillator, which is only accurate to about 30%: too loose for a UART.
   Switch to the board's 8 MHz crystal (EK-LM3S6965) in `SYSCTL_RCC`
   (`0x400FE060`): select the main oscillator, set the crystal value, and
   bypass the PLL (or configure the PLL for a faster clock).
2. **SysTick reload.** `kernel/kernel.c` computes the reload for 12 MHz
   (`SYSTICK_RELOAD`). Use the real clock: 80,000 for a 10 ms tick at 8 MHz.
3. **UART0 setup.** `uart_init` in `lib/uart.c` does nothing because
   QEMU's UART works from reset. On the chip:
   - enable the UART0 and GPIO port A clocks (`RCGC1` bit 0, `RCGC2` bit 0);
   - give PA0 (U0Rx) and PA1 (U0Tx) to the UART (`GPIO_PORTA_AFSEL`) and
     enable them as digital pins (`GPIO_PORTA_DEN`);
   - with the UART disabled, set the baud rate divisors (`UARTIBRD`,
     `UARTFBRD`; for 115200 baud at 8 MHz, 4 and 22), 8N1 in `UARTLCRH`,
     then enable it with transmit and receive in `UARTCTL`.

   This runs in the kernel before the scheduler starts, so the
   "kernel UART writes after scheduler start = 0" check is unaffected.

## Flashing

The EK-LM3S6965 works with OpenOCD's board file:

```bash
make
openocd -f board/ek-lm3s6965.cfg -c "program cpos.elf verify reset exit"
```

Then open the board's USB serial port at 115200 baud, 8N1, for example
`picocom -b 115200 /dev/ttyUSB1` (the second of the two ports the debug
interface creates is usually the UART).

## What to expect

The boot tests should print the same results as on QEMU, with one known
difference:

- **SysTick write from a task.** On QEMU 7.2 the write is ignored and the
  task reports `Task write to SysTick had no effect: OK`. A Cortex-M3
  raises a BusFault instead, so expect
  `[fault] systick-poke: BusFault ..., task killed` followed by
  `systick-poke stopped by fault: OK`. The test accepts either
  (`TASK_MAY_FAULT`).

Timing checks (sleep length, preemption counts) depend on the SysTick
reload matching the real clock; if they fail, check step 2.

Running on hardware would also settle the one documented QEMU gap: an
unprivileged task can read System Control Space registers on QEMU, while
the chip should fault. Checking that needs a test that expects the fault
on hardware only, so it is left until CPOS runs on a board.
