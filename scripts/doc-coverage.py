#!/usr/bin/env python3
"""Measure documentation coverage of Rust source files.

Uses `cargo doc` with `--document-private-items` and the `missing_docs` lint
to count undocumented items, then compares against a total item count to
produce a coverage percentage.

Usage:
    python3 scripts/doc-coverage.py [--manifest-path <path>] [--min <pct>]
"""

import argparse
import os
import re
import subprocess
import sys
import tempfile


def get_total_items(src_dir):
    """Count total documentable items (excluding #[cfg(test)] blocks and impl methods)."""
    total = 0
    for root, dirs, files in os.walk(src_dir):
        dirs[:] = [d for d in dirs if not d.startswith('.')]
        for f in files:
            if not f.endswith('.rs'):
                continue
            filepath = os.path.join(root, f)
            total += count_items_in_file(filepath)
    return total


def count_items_in_file(filepath):
    """Count documentable items in a single file."""
    with open(filepath) as f:
        content = f.read()

    # Remove #[cfg(test)] blocks (including their content)
    no_test = re.sub(
        r'#\[cfg\(test\)\]\s*\n\s*mod\s+\w+\s*\{[^}]*\}',
        '',
        content,
    )

    # Remove #[cfg(test)] attribute blocks (for inline tests)
    # Match #[cfg(test)] followed by mod tests { ... } with balanced braces
    result = []
    i = 0
    depth = 0
    in_test_block = False
    lines = content.split('\n')
    filtered = []
    i = 0
    while i < len(lines):
        line = lines[i]
        if '#[cfg(test)]' in line:
            in_test_block = True
            # Skip the #[cfg(test)] line
            i += 1
            continue
        if in_test_block:
            # Check if this is a mod declaration
            if re.match(r'^\s*mod\s+\w+\s*\{', line):
                # Skip to matching closing brace
                depth = 1
                i += 1
                while i < len(lines) and depth > 0:
                    for ch in lines[i]:
                        if ch == '{':
                            depth += 1
                        elif ch == '}':
                            depth -= 1
                    i += 1
                in_test_block = False
                continue
            elif not line.strip() or line.strip().startswith('//'):
                i += 1
                continue
            else:
                in_test_block = False
        filtered.append(line)
        i += 1

    content = '\n'.join(filtered)

    # Remove impl blocks - methods inside impl don't need separate docs
    # (they inherit from the trait)
    content = re.sub(
        r'impl[^{]*\{[^}]*\}(?:\s+where[^{]*\{[^}]*\})*',
        '',
        content,
    )

    # Find all documentable items
    pattern = re.compile(r'^\s*(?:pub\s+)?(?:pub\(crate\)\s+)?(?:'
                         r'fn\s+\w+|'
                         r'struct\s+\w+|'
                         r'enum\s+\w+|'
                         r'trait\s+\w+|'
                         r'type\s+\w+|'
                         r'const\s+\w+|'
                         r'mod\s+\w+'
                         r')', re.MULTILINE)
    return len(pattern.findall(content))


def get_undocumented_items(manifest_path):
    """Run cargo doc with missing_docs warnings and count undocumented items."""
    env = os.environ.copy()
    env['RUSTDOCFLAGS'] = '--warn missing_docs'

    # Run cargo doc, capture stderr
    result = subprocess.run(
        ['cargo', 'doc', '--document-private-items', '--manifest-path', manifest_path],
        capture_output=True,
        text=True,
        env=env,
        timeout=180,
    )

    # Count "missing documentation for" lines
    count = 0
    for line in result.stderr.split('\n'):
        if 'missing documentation for' in line:
            count += 1
    for line in result.stdout.split('\n'):
        if 'missing documentation for' in line:
            count += 1

    return count


def main():
    parser = argparse.ArgumentParser(description="Measure doc coverage of Rust source files")
    parser.add_argument("--manifest-path", default="Cargo.toml",
                        help="Path to Cargo.toml")
    parser.add_argument("--min", type=float, default=100.0,
                        help="Minimum acceptable coverage percentage")
    parser.add_argument("--format", choices=["text", "markdown"], default="text")
    args = parser.parse_args()

    manifest_path = os.path.abspath(args.manifest_path)
    project_dir = os.path.dirname(manifest_path)
    src_dir = os.path.join(project_dir, 'src')

    total = get_total_items(src_dir)
    undocumented = get_undocumented_items(manifest_path)
    documented = total - undocumented
    coverage = (documented / total * 100) if total > 0 else 100.0

    if args.format == "markdown":
        print(f"| Metric | Value |")
        print(f"|--------|-------|")
        print(f"| Total items | {total} |")
        print(f"| Undocumented | {undocumented} |")
        print(f"| Documented | {documented} |")
        print(f"| Coverage | {coverage:.0f}% |")
    else:
        status = "✅" if coverage >= args.min else "❌"
        print(f"  Total items:       {total}")
        print(f"  Undocumented:       {undocumented}")
        print(f"  Documented:         {documented}")
        print(f"  Coverage:           {coverage:.0f}%")
        print(f"  Status:             {status}")

    if coverage < args.min:
        print(f"\nError: Doc coverage {coverage:.0f}% < min {args.min:.0f}%",
              file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
