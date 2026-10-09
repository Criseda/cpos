/*
 * SPDX-License-Identifier: MIT
 * Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
 *
 * Author: Laurentiu Cristian Preda (criseda)
 * GitHub: https://github.com/criseda
 *
 * User-mode tasks for CPOS
 *
 * Everything here runs unprivileged. The console server is the only task
 * the MPU lets reach the UART; every other task prints by sending it
 * messages (SYS_WRITE is a message to the console). The test tasks check
 * the scheduler, the privilege split, memory protection and IPC, and
 * report "name: OK" or "name: FAILED" lines.
 *
 * No globals or static locals: kernel RAM is off limits to tasks.
 */

#include <stdint.h>
#include "user.h"

#define UART0_DR  (*(volatile uint32_t *)0x4000C000)
#define UART0_FR  (*(volatile uint32_t *)0x4000C018)
#define UART0_IMR (*(volatile uint32_t *)0x4000C038)
#define UART0_ICR (*(volatile uint32_t *)0x4000C044)
#define UART_FR_RXFE (1u << 4)
#define UART_FR_TXFF (1u << 5)
#define UART_INT_RX  (1u << 4) /* a character arrived */
#define UART_INT_RT  (1u << 6) /* receive timeout (FIFO mode) */

#define MSG_SIZE    64
#define MAX_READERS 8

#define SYST_CSR   (*(volatile uint32_t *)0xE000E010)
#define KERNEL_RAM 0x20000000u
#define TASK_SLOTS 0x20003000u
#define SLOT_SIZE  2048u

/* ---- small helpers (stack only) --------------------------------------- */

typedef struct {
	char buf[96];
	uint32_t len;
} line_t;

static void line_init(line_t *l)
{
	l->len = 0;
}

static void line_str(line_t *l, const char *s)
{
	while (*s && l->len < sizeof(l->buf)) {
		l->buf[l->len++] = *s++;
	}
}

static void line_dec(line_t *l, uint32_t n)
{
	char digits[10];
	int i = 0;
	do {
		digits[i++] = (char)('0' + n % 10);
		n /= 10;
	} while (n && i < 10);
	while (i && l->len < sizeof(l->buf)) {
		l->buf[l->len++] = digits[--i];
	}
}

/* SYS_WRITE takes at most one message per call; keep going until done */
static void line_send(line_t *l)
{
	uint32_t done = 0;
	while (done < l->len) {
		int32_t n = sys_write(l->buf + done, l->len - done);
		if (n <= 0) {
			return;
		}
		done += (uint32_t)n;
	}
}

static void report(const char *name, int ok)
{
	line_t l;
	line_init(&l);
	line_str(&l, "  - ");
	line_str(&l, name);
	line_str(&l, ok ? ": OK\n" : ": FAILED\n");
	line_send(&l);
}

static void line_bytes(line_t *l, const char *s, uint32_t n)
{
	for (uint32_t i = 0; i < n && l->len < sizeof(l->buf); i++) {
		l->buf[l->len++] = s[i];
	}
}

static int same(const char *a, const char *b, uint32_t n)
{
	for (uint32_t i = 0; i < n; i++) {
		if (a[i] != b[i]) {
			return 0;
		}
	}
	return 1;
}

static uint32_t own_slot(void)
{
	uint32_t marker;
	return (uint32_t)&marker & ~(SLOT_SIZE - 1);
}

/* ---- system tasks ------------------------------------------------------- */

void idle_task(uint32_t arg)
{
	(void)arg;
	while (1) {
		__asm volatile("wfi");
	}
}

static void put(char c)
{
	while (UART0_FR & UART_FR_TXFF) {
	}
	UART0_DR = (uint32_t)c;
}

/* Input line being typed; complete once it ends in '\n' */
typedef struct {
	char buf[MSG_SIZE];
	uint32_t len;
	int complete;
	char last;
} input_t;

/* Line editing with echo: Enter ends the line, backspace erases */
static void input_char(input_t *in, char c)
{
	char last = in->last;
	in->last = c;
	if (in->complete) {
		return; /* one line is held until a task reads it */
	}
	if (c == '\r' || c == '\n') {
		if (c == '\n' && last == '\r') {
			return; /* CR LF is one line end */
		}
		in->buf[in->len++] = '\n';
		in->complete = 1;
		put('\n');
	} else if (c == '\b' || c == 0x7f) {
		if (in->len) {
			in->len--;
			put('\b');
			put(' ');
			put('\b');
		}
	} else if (c >= ' ' && in->len < MSG_SIZE - 1) {
		in->buf[in->len++] = c;
		put(c);
	}
}

/* Owns the UART. Prints whatever it is sent (task output and the kernel
 * log), reads input when the kernel says the UART interrupt fired, and
 * answers SYS_READ requests (empty messages from tasks) with a line. */
void console_task(uint32_t arg)
{
	(void)arg;
	char buf[MSG_SIZE];
	uint32_t from;
	input_t in = { .len = 0, .complete = 0, .last = 0 };
	uint32_t readers[MAX_READERS];
	uint32_t nreaders = 0;

	UART0_IMR |= UART_INT_RX | UART_INT_RT;

	while (1) {
		int32_t n = sys_recv(buf, sizeof(buf), &from);
		if (n < 0) {
			continue;
		}
		if (from == SENDER_IRQ) {
			UART0_ICR = UART_INT_RX | UART_INT_RT;
			while (!(UART0_FR & UART_FR_RXFE)) {
				input_char(&in, (char)UART0_DR);
			}
		} else if (n == 0 && from != SENDER_KERNEL) {
			if (nreaders < MAX_READERS) {
				readers[nreaders++] = from;
			}
		} else {
			for (int32_t i = 0; i < n; i++) {
				put(buf[i]);
			}
		}

		/* Oldest reader gets the next line; a reader that has gone
		 * away (-ERROR_NO_TASK) leaves the line for the next one */
		while (in.complete && nreaders) {
			int32_t sent = sys_send(readers[0], in.buf, in.len);
			for (uint32_t i = 1; i < nreaders; i++) {
				readers[i - 1] = readers[i];
			}
			nreaders--;
			if (sent >= 0) {
				in.len = 0;
				in.complete = 0;
			}
		}
	}
}

/* Reads console lines through SYS_READ and prints them back */
static void echo_task(uint32_t arg)
{
	(void)arg;
	char buf[MSG_SIZE];
	line_t l;

	while (1) {
		int32_t n = sys_read(buf, sizeof(buf));
		if (n <= 0) {
			sys_sleep(100);
			continue;
		}
		line_init(&l);
		line_str(&l, "[echo] task read: ");
		line_bytes(&l, buf, (uint32_t)n);
		if (buf[n - 1] != '\n') {
			line_str(&l, "\n");
		}
		line_send(&l);
	}
}

/* ---- step 1: scheduling ------------------------------------------------- */

/* Busy-loops without yielding for 30 ticks. If the tick count ever jumps
 * by more than one, another task ran in between: preemption works. */
static void spinner_task(uint32_t arg)
{
	(void)arg;
	uint32_t start = sys_ticks();
	uint32_t last = start;
	uint32_t preempted = 0;

	while (sys_ticks() - start < 30) {
		uint32_t now = sys_ticks();
		if (now > last + 1) {
			preempted++;
		}
		last = now;
	}

	line_t l;
	line_init(&l);
	line_str(&l, "  - Preemptive round robin (spinner preempted ");
	line_dec(&l, preempted);
	line_str(&l, preempted >= 3 ? " times): OK\n" : " times): FAILED\n");
	line_send(&l);
}

static void sleeper_task(uint32_t arg)
{
	(void)arg;
	uint32_t before = sys_ticks();
	int32_t r = sys_sleep(100);
	uint32_t slept = sys_ticks() - before;

	line_t l;
	line_init(&l);
	line_str(&l, "  - SYS_SLEEP(100 ms) under load woke after ");
	line_dec(&l, slept);
	line_str(&l, " ticks of ");
	line_dec(&l, TICK_MS);
	line_str(&l, " ms");
	line_str(&l, r == 0 && slept >= 10 && slept <= 12 ? ": OK\n" : ": FAILED\n");
	line_send(&l);
}

static void exit_task(uint32_t arg)
{
	(void)arg;
	sys_exit(0);
	report("Code after SYS_EXIT never runs", 0);
}

/* ---- step 2: unprivileged mode ------------------------------------------ */

static uint32_t read_control(void)
{
	uint32_t control;
	__asm volatile("mrs %0, control" : "=r"(control));
	return control;
}

/* Privileged operations from a task must have no effect */
static void unprivileged_task(uint32_t arg)
{
	(void)arg;
	report("Task runs unprivileged (CONTROL.nPRIV = 1)", read_control() & 1);

	/* Try to clear nPRIV; the write is ignored in unprivileged mode */
	__asm volatile("msr control, %0\nisb" : : "r"(0u) : "memory");
	report("Task cannot make itself privileged", read_control() & 1);
}

/* Try to stop the system timer. Unprivileged access to the System Control
 * Space must not take effect: a Cortex-M3 raises a BusFault, which kills
 * the task; QEMU 7.2 ignores the write instead, and ticks keep coming.
 * Either outcome passes (TASK_MAY_FAULT). */
static void systick_poke_task(uint32_t arg)
{
	(void)arg;
	SYST_CSR = 0;
	uint32_t before = sys_ticks();
	sys_sleep(50);
	report("Task write to SysTick had no effect", sys_ticks() - before >= 5);
}

/* ---- step 3: memory protection ------------------------------------------ */

static void bad_pointer_task(uint32_t arg)
{
	(void)arg;
	uint32_t slot = own_slot();
	char buf[8];
	uint32_t from;

	report("SYS_WRITE from a kernel address rejected",
	       sys_write((const void *)KERNEL_RAM, 4) == -ERROR_BAD_ADDRESS);
	report("SYS_WRITE from another task's slot rejected",
	       sys_write((const void *)TASK_SLOTS, 4) == -ERROR_BAD_ADDRESS);
	report("SYS_WRITE running past the task's slot rejected",
	       sys_write((const void *)(slot + SLOT_SIZE - 2), 8) ==
		       -ERROR_BAD_ADDRESS);
	report("SYS_RECV into flash rejected",
	       sys_recv((void *)"flash", 4, &from) == -ERROR_BAD_ADDRESS);
	report("SYS_RECV sender into kernel memory rejected",
	       sys_recv(buf, sizeof(buf), (uint32_t *)KERNEL_RAM) ==
		       -ERROR_BAD_ADDRESS);
	report("SYS_FREE of a kernel heap pointer rejected",
	       sys_free(0x20002010) == -ERROR_INVALID_ARGUMENT);
	report("SYS_WRITE from flash allowed", sys_write("", 0) == 0);
}

/* Each of these must be killed by the MPU or the bus */
static void other_slot_task(uint32_t arg)
{
	(void)arg;
	/* Slot 0 belongs to the idle task */
	volatile uint32_t *p = (volatile uint32_t *)(TASK_SLOTS + SLOT_SIZE - 16);
	(void)*p;
	report("Task read another task's stack", 0);
}

static void kernel_data_task(uint32_t arg)
{
	(void)arg;
	volatile uint32_t *p = (volatile uint32_t *)KERNEL_RAM;
	*p = 0;
	report("Task wrote kernel memory", 0);
}

static void uart_task(uint32_t arg)
{
	(void)arg;
	UART0_DR = '!';
	report("Non-console task wrote the UART", 0);
}

/* Unbounded recursion with small frames, as a runaway recursive function
 * would do: the stack reaches the guard above the heap and faults there */
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Winfinite-recursion"
static uint32_t __attribute__((noinline)) recurse(uint32_t depth)
{
	volatile uint32_t frame[2] = { depth, depth };
	return recurse(depth + 1) + frame[0] + frame[1];
}
#pragma GCC diagnostic pop

static void stack_overflow_task(uint32_t arg)
{
	report("Stack overflow went undetected", recurse(arg) == 0);
}

static void heap_task(uint32_t arg)
{
	(void)arg;
	uint32_t slot = own_slot();
	int32_t p = sys_alloc(100);
	int ok = p > 0 && (uint32_t)p >= slot && (uint32_t)p + 100 <= slot + SLOT_SIZE;
	report("SYS_ALLOC in a task returns memory in its own slot", ok);
	if (ok) {
		char *mem = (char *)p;
		for (int i = 0; i < 100; i++) {
			mem[i] = (char)i;
		}
		report("SYS_FREE in a task", sys_free(p) == 0);
		report("Task double free rejected", sys_free(p) == -ERROR_DOUBLE_FREE);
	}
	report("Task heap exhaustion reported",
	       sys_alloc(4096) == -ERROR_OUT_OF_MEMORY);
}

/* ---- step 4: message passing -------------------------------------------- */

static void pong_task(uint32_t arg)
{
	(void)arg;
	char buf[16];
	uint32_t from;
	for (int i = 0; i < 3; i++) {
		int32_t n = sys_recv(buf, sizeof(buf), &from);
		if (n == 4 && same(buf, "ping", 4)) {
			sys_send(from, "pong", 4);
		}
	}
}

static void ping_task(uint32_t pong)
{
	char buf[16];
	uint32_t from = 0;
	int ok = 1;
	for (int i = 0; i < 3; i++) {
		ok &= sys_send(pong, "ping", 4) == 4;
		int32_t n = sys_recv(buf, sizeof(buf), &from);
		ok &= n == 4 && from == pong && same(buf, "pong", 4);
	}
	report("IPC ping/pong, 3 round trips", ok);
	report("SYS_SEND to a missing task rejected",
	       sys_send(999, "x", 1) == -ERROR_NO_TASK);
	report("SYS_SEND over the message size rejected",
	       sys_send(pong, buf, 65) == -ERROR_INVALID_ARGUMENT);
}

/* ---- registration ------------------------------------------------------- */

void register_user_tasks(void)
{
	rust_task_register("idle", idle_task, 0, TASK_IDLE);
	rust_task_register("console", console_task, 0, TASK_CONSOLE);
	rust_task_register("echo", echo_task, 0, TASK_SERVICE);

	/* These run together: the sleeper and spinners load each other */
	int32_t pong = rust_task_register("pong", pong_task, 0, 0);
	rust_task_register("ping", ping_task, (uint32_t)pong, 0);
	rust_task_register("spinner-a", spinner_task, 0, 0);
	rust_task_register("spinner-b", spinner_task, 0, 0);
	rust_task_register("sleeper", sleeper_task, 0, 0);
	rust_task_register("unprivileged", unprivileged_task, 0, 0);
	rust_task_register("systick-poke", systick_poke_task, 0, TASK_MAY_FAULT);

	/* Queued until slots free up */
	rust_task_register("bad-pointers", bad_pointer_task, 0, 0);
	rust_task_register("other-slot", other_slot_task, 0, TASK_EXPECT_FAULT);
	rust_task_register("kernel-data", kernel_data_task, 0, TASK_EXPECT_FAULT);
	rust_task_register("uart-poke", uart_task, 0, TASK_EXPECT_FAULT);
	rust_task_register("stack-overflow", stack_overflow_task, 0,
			   TASK_EXPECT_FAULT);
	rust_task_register("heap", heap_task, 0, 0);
	rust_task_register("exit", exit_task, 0, 0);
}
