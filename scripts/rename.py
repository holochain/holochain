#!/usr/bin/python3

import re
import sys

hashes = dict()
hexes = dict()
others = dict()

def replace_hashes(line):
    pat_hash = re.compile("uhC(.)[a-zA-Z0-9_-]+")
    m = pat_hash.search(line)
    if m is None:
        return line, False
    item = m.group(0)
    id = m.group(1)
    rep = ""
    if item in hashes:
        rep = hashes[item]
    else:
        rep = f'<H{str(len(hashes))}>'
        hashes[item] = rep
    line = pat_hash.sub(rep, line)
    return line, True

def replace_hexes(line):
    pat_hex = re.compile("0x[a-zA-Z0-9]{8}[a-zA-Z0-9]*")
    m = pat_hex.search(line)
    if m is None:
        return line, False
    item = m.group(0)
    rep = ""
    if item in hexes:
        rep = hexes[item]
    else:
        rep = f'<x{str(len(hexes))}>'
        hexes[item] = rep
    line = pat_hex.sub(rep, line)
    return line, True

def replace_others(line):
    pat_other = re.compile("[a-zA-Z0-9-]{16}[a-zA-Z0-9-]*")
    pat_num = re.compile("[0-9]+")
    pat_alpha = re.compile("[a-zA-Z]+")
    m = pat_other.search(line)
    if m is None:
        return line, False
    item = m.group(0)
    rep = None
    if item in others:
        rep = others[item]
    elif pat_num.search(item) is not None and pat_alpha.search(item) is not None:
        rep = f'<o{str(len(others))}>'
        others[item] = rep
    else:
        return line, False

    if rep:
        line = pat_other.sub(rep, line)
    return line, True

def main():
    lines = sys.stdin.readlines()
    for i, line in enumerate(lines):
        changed = True
        while changed:
            line, changed = replace_hashes(line)
        changed = True
        while changed:
            line, changed = replace_hexes(line)
        changed = True
        while changed:
            line, changed = replace_others(line)
        lines[i] = line

    print("REPLACEMENTS")
    print("============")
    print()
    print("Hashes:")
    for k, v in hashes.items():
        print(f'{k} -> {v}')
    print()
    print("Hexes:")
    for k, v in hexes.items():
        print(f'{k} -> {v}')
    print()
    print("Other:")
    for k, v in others.items():
        print(f'{k} -> {v}')

    print()
    print("ACTUAL LOG")
    print("==========")

    for line in lines:
        print(line, end='')

if __name__ == '__main__':
    main()