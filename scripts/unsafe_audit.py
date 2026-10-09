#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
"""Count `unsafe` in the Rust kernel and check that each use is justified.

Every `unsafe` block or `unsafe impl` needs a `// SAFETY:` comment on the
same line or in the comment lines just above it; every `unsafe fn` needs a
`# Safety` section in its doc comment. Test modules are counted separately
and not required to comply.

Exits non-zero if any non-test `unsafe` lacks a justification.

Usage: python3 scripts/unsafe_audit.py [rust_kernel]
"""

import pathlib
import re
import sys

# String contents are blanked before matching, so `extern "C"` reads `extern ""`
UNSAFE = re.compile(r"\bunsafe\b\s*(fn|impl|extern\s+\"\"\s+fn|\{)?")


def strip_strings_and_comments(line):
    """The code part of a line, without string contents or // comments."""
    line = re.sub(r'b?"(\\.|[^"\\])*"', '""', line)
    return line.split("//", 1)[0]


def justified(lines, i, kind):
    """Look for the justification above line `i`."""
    if "SAFETY:" in lines[i]:
        return True
    j = i - 1
    # Walk up over comments, attributes and the start of the statement
    while j >= 0 and i - j <= 12:
        text = lines[j].strip()
        if kind == "fn" and text.startswith("///") and "# Safety" in text:
            return True
        if "SAFETY" in text and text.startswith("//"):
            return True
        if text.startswith(("//", "#[", "///")) or text == "":
            j -= 1
            continue
        # Code: allow a statement that spans a few lines before the block
        if kind == "block" and not text.endswith((";", "}")):
            j -= 1
            continue
        break
    return False


def audit(root):
    results = {"code": [], "tests": []}
    for path in sorted(root.rglob("*.rs")):
        if "target" in path.parts or "fuzz" in path.parts:
            continue
        lines = path.read_text(encoding="utf-8").splitlines()
        in_tests = False
        for i, raw in enumerate(lines):
            if re.match(r"\s*#\[cfg\(test\)\]", raw) and i + 1 < len(lines) and "mod tests" in lines[i + 1]:
                in_tests = True
            code = strip_strings_and_comments(raw)
            for m in UNSAFE.finditer(code):
                what = m.group(1) or ""
                if what == "{" or what == "":
                    kind = "block"
                elif what == "impl":
                    kind = "impl"
                else:
                    kind = "fn"
                ok = justified(lines, i, kind)
                rel = path.relative_to(root.parent)
                results["tests" if in_tests else "code"].append((str(rel), i + 1, kind, ok))
    return results


def main():
    root = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "rust_kernel").resolve()
    results = audit(root)
    code = results["code"]
    missing = [r for r in code if not r[3]]

    for kind in ("block", "fn", "impl"):
        n = sum(1 for r in code if r[2] == kind)
        print(f"unsafe {kind:5}: {n}")
    print(f"total (kernel code): {len(code)}, justified: {len(code) - len(missing)}")
    print(f"in test modules (not audited): {len(results['tests'])}")
    files = sorted({r[0] for r in code})
    for f in files:
        print(f"  {f}: {sum(1 for r in code if r[0] == f)}")

    for path, line, kind, _ in missing:
        print(f"MISSING SAFETY: {path}:{line} (unsafe {kind})")
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
