#!/bin/bash
sed -e '$s/$/\n/' -s hwp_*.md > hwp.md
pandoc -L diagram-1.0.0/diagram.lua --extract-media media/ -f markdown -t latex hwp.md --template ./pandoc-template.latex > hwp.tex
pdflatex --shell-escape hwp.tex
pdflatex --shell-escape hwp_alpha.tex
pandoc -f markdown -t latex holochain-players-of-ludos.md --template ./pandoc-template.latex > holochain-players-of-ludos.tex
pdflatex --shell-escape holochain-players-of-ludos.tex
