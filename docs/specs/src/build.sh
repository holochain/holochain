#!/bin/bash
sed -e '$s/$/\n/' -s hwp_*.md > holochain-white-paper-2.0.md
pandoc -L diagram-1.0.0/diagram.lua --extract-media media/ -f markdown -t latex holochain-white-paper-2.1.md --template ./pandoc-template.latex > holochain-white-paper-2.1.tex
pdflatex --shell-escape holochain-white-paper-2.1.tex
pdflatex --shell-escape holochain-white-paper-alpha.tex
pandoc -f markdown -t latex holochain-players-of-ludos.md --template ./pandoc-template.latex > holochain-players-of-ludos.tex
pdflatex --shell-escape holochain-players-of-ludos.tex
