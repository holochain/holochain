FILTER_FILE := $(wildcard *.lua)
PANDOC ?= pandoc
DIFF ?= diff

.PHONY: test
test: test-asymptote test-dot test-mermaid test-plantuml test-tikz \
	test-no-alt-or-caption

test-%: test/test-%.yaml test/input-%.md $(FILTER_FILE)
	@$(PANDOC) --defaults test/test-$*.yaml | \
	  $(DIFF) test/expected-$*.html -

sample.html: sample.md diagram.lua
	@$(PANDOC) --self-contained \
	    --lua-filter=diagram.lua \
	    --metadata=pythonPath:"python3" \
	    --metadata=title:"README" \
	    --output=$@ $<

clean:
	@rm -f sample.html
	@rm -rf tmp-latex
