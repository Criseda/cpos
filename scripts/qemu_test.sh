#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
#
# Boot CPOS in QEMU and check the on-target tests: wait for the task tests
# to finish, type a line on the serial port, and wait for the echo task to
# read it back. Fails if any check prints FAILED, fewer than EXPECTED_OK
# checks pass, the tests never finish, or the input never arrives.
#
# Usage: scripts/qemu_test.sh [cpos.elf]

set -u

ELF=${1:-cpos.elf}
EXPECTED_OK=${EXPECTED_OK:-41}
INPUT="hello cpos"

work=$(mktemp -d)
out="$work/out.txt"
fifo="$work/stdin"
mkfifo "$fifo"
trap 'rm -rf "$work"' EXIT

timeout 60 qemu-system-arm -machine lm3s6965evb -cpu cortex-m3 -nographic \
	-monitor null -serial stdio -kernel "$ELF" <"$fifo" >"$out" 2>&1 &
qemu=$!
exec 3>"$fifo"

# wait_for TEXT SECONDS
wait_for() {
	for _ in $(seq 1 $(($2 * 10))); do
		grep -q "$1" "$out" && return 0
		kill -0 "$qemu" 2>/dev/null || return 1
		sleep 0.1
	done
	return 1
}

finished=0
echoed=0
if wait_for "Task tests complete" 30; then
	finished=1
	# Enter sends CR, as a terminal does
	printf '%s\r' "$INPUT" >&3
	wait_for "task read: $INPUT" 10 && echoed=1
fi

kill "$qemu" 2>/dev/null
wait "$qemu" 2>/dev/null
exec 3>&-

cat "$out"

ok=$(grep -c ': OK' "$out")
failed=$(grep -c 'FAILED' "$out")
echo "=== OK: $ok (expected at least $EXPECTED_OK)  FAILED: $failed  tests finished: $finished  input echoed: $echoed"

status=0
[ "$finished" -eq 1 ] || { echo "error: task tests did not finish"; status=1; }
[ "$failed" -eq 0 ] || { echo "error: $failed checks failed"; status=1; }
[ "$ok" -ge "$EXPECTED_OK" ] || { echo "error: only $ok checks passed"; status=1; }
[ "$echoed" -eq 1 ] || { echo "error: console input did not reach the echo task"; status=1; }
exit $status
