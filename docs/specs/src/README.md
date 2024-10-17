To build the TEX whitepaper:

1. Make sure you have the following installed and available on the command line:
    * [`pandoc`](https://pandoc.org/) for converting the Markdown to LaTeX.
    * [`tex`](https://tug.org/) for converting the LaTeX to PDF. Hint: the easiest route is to install texlive, but most Linux distros have outdated packages, so do the vanilla install. [These are the best instructions](https://tex.stackexchange.com/a/95373) I've found.
    * [`inkscape`](https://inkscape.org) for converting the SVG diagrams to embeddable PDFs.
    * [`dot` from Graphviz](https://www.graphviz.org/) for converting various diagrams written in inline DOT code blocks to embeddable PDFs.
2. Type: `./build.sh`

