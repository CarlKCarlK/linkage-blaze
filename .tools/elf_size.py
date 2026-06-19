#!/usr/bin/env python3
"""Print flash and RAM usage for an ESP32 ELF via xtensa-esp32-elf-readelf."""
import sys, re, subprocess

elf = sys.argv[1]
out = subprocess.check_output(["xtensa-esp32-elf-readelf", "-S", elf], text=True)

flash_code = flash_ro = ram = 0
for line in out.splitlines():
    m = re.search(r'\]\s+(\S+)\s+(PROGBITS|NOBITS)\s+([0-9a-f]{8})\s+\S+\s+([0-9a-f]{6})', line)
    if not m:
        continue
    name, kind, addr, size_hex = m.group(1), m.group(2), m.group(3), m.group(4)
    size = int(size_hex, 16)
    if size == 0:
        continue
    executable = 'X' in line
    if addr.startswith('40') or addr.startswith('3f4'):
        if executable:
            flash_code += size
        else:
            flash_ro += size
    elif addr.startswith('3ff'):
        ram += size

total = flash_code + flash_ro
print(f"  Flash code:   {flash_code:7,} B  ({flash_code/1024:.1f} KB)")
print(f"  Flash rodata: {flash_ro:7,} B  ({flash_ro/1024:.1f} KB)")
print(f"  Flash total:  {total:7,} B  ({total/1024:.1f} KB)  of 4096 KB")
print(f"  RAM total:    {ram:7,} B  ({ram/1024:.1f} KB)  of 320 KB")
