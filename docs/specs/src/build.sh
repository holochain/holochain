#!/bin/bash
sed -e '$s/$/\n/' -s hwp_*.md > hwp.md
pandoc -f markdown -t latex hwp.md --template ./pandoc-template.latex > hwp.tex