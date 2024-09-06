# Diagram Generator Lua Filter

## Introduction
This Lua filter is used to create images with or without captions from code
blocks. Currently PlantUML, Graphviz, Ti*k*Z, Asymptote, and Python can be
processed. This document also serves as a test document, which is why the
subsequent test diagrams are integrated in every supported language.

## Prerequisites
To be able to use this Lua filter, the respective external tools must be
installed. However, it is sufficient if the tools to be used are installed.
If you only want to use PlantUML, you don't need LaTeX or Python, etc.

### PlantUML
To use PlantUML, you must install PlantUML itself. See the
[PlantUML website](http://plantuml.com/) for more details. It should be
noted that PlantUML is a Java program and therefore Java must also
be installed.

By default, this filter expects the plantuml.jar file to be in the
working directory. Alternatively, the environment variable
`PLANTUML` can be set with a path. If, for example, a specific
PlantUML version is to be used per pandoc document, the
`plantuml_path` meta variable can be set.

Furthermore, this filter assumes that Java is located in the
system or user path. This means that from any place of the system
the `java` command is understood. Alternatively, the `JAVA_HOME`
environment variable gets used. To use a specific Java version per
pandoc document, use the `java_path` meta variable. Please notice
that `JAVA_HOME` must be set to the java's home directory e.g.
`c:\Program Files\Java\jre1.8.0_201\` whereas `java_path` must be
set to the absolute path of `java.exe` e.g.
`c:\Program Files\Java\jre1.8.0_201\bin\java.exe`.

Example usage:

```{.plantuml caption="This is an image, created by **PlantUML**." width=50%}
@startuml
Alice -> Bob: Authentication Request Bob --> Alice: Authentication Response
Alice -> Bob: Another authentication Request Alice <-- Bob: another Response
@enduml
```

### Graphviz
To use Graphviz you only need to install Graphviz, as you can read
on its [website](http://www.graphviz.org/). There are no other
dependencies.

This filter assumes that the `dot` command is located in the path
and therefore can be used from any location. Alternatively, you can
set the environment variable `DOT` or use the pandoc's meta variable
`dot_path`.

Example usage from [the Graphviz
gallery](https://graphviz.gitlab.io/_pages/Gallery/directed/fsm.html):

```{.graphviz caption="This is an image, created by **Graphviz**'s dot."}
digraph finite_state_machine {
	rankdir=LR;
	node [shape = doublecircle]; LR_0 LR_3 LR_4 LR_8;
	node [shape = circle];
	LR_0 -> LR_2 [ label = "SS(B)" ];
	LR_0 -> LR_1 [ label = "SS(S)" ];
	LR_1 -> LR_3 [ label = "S($end)" ];
	LR_2 -> LR_6 [ label = "SS(b)" ];
	LR_2 -> LR_5 [ label = "SS(a)" ];
	LR_2 -> LR_4 [ label = "S(A)" ];
	LR_5 -> LR_7 [ label = "S(b)" ];
	LR_5 -> LR_5 [ label = "S(a)" ];
	LR_6 -> LR_6 [ label = "S(b)" ];
	LR_6 -> LR_5 [ label = "S(a)" ];
	LR_7 -> LR_8 [ label = "S(b)" ];
	LR_7 -> LR_5 [ label = "S(a)" ];
	LR_8 -> LR_6 [ label = "S(b)" ];
	LR_8 -> LR_5 [ label = "S(a)" ];
}
```

### Ti*k*Z
Ti*k*Z (cf. [Wikipedia](https://en.wikipedia.org/wiki/PGF/TikZ)) is a
description language for graphics of any kind that can be used within
LaTeX (cf. [Wikipedia](https://en.wikipedia.org/wiki/LaTeX)).

Therefore a LaTeX system must be installed on the system. The Ti*k*Z code is
embedded into a dynamic LaTeX document. This temporary document gets
translated into a PDF document using LaTeX (`pdflatex`). Finally,
Inkscape is used to convert the PDF file to the desired format.

Note: We are using Inkscape here to use a stable solution for the
convertion. Formerly ImageMagick was used instead. ImageMagick is
not able to convert PDF files. Hence, it uses Ghostscript to do
so, cf. [1](https://stackoverflow.com/a/6599718/2258393).
Unfortunately, Ghostscript behaves unpredictable during Windows and
Linux tests cases, cf. [2](https://stackoverflow.com/questions/21774561/some-pdfs-are-converted-improperly-using-imagemagick),
[3](https://stackoverflow.com/questions/9064706/imagemagic-convert-command-pdf-convertion-with-bad-size-orientation), [4](https://stackoverflow.com/questions/18837093/imagemagic-renders-image-with-black-background),
[5](https://stackoverflow.com/questions/37392798/pdf-to-svg-is-not-perfect),
[6](https://stackoverflow.com/q/10288065/2258393), etc. By using Inkscape,
we need one dependency less and get rid of unexpected Ghostscript issues.

Due to this more complicated process, the use of Ti*k*Z is also more
complicated overall. The process is error-prone: An insufficiently
configured LaTeX installation or an insufficiently configured
Inkscape installation can lead to errors. Overall, this results in
the following dependencies:

- Any LaTeX installation. This should be configured so that
missing packages are installed automatically. This filter uses the
`pdflatex` command which is available by the system's path. Alternatively,
you can set the `PDFLATEX` environment variable. In case you have to use
a specific LaTeX version on a pandoc document basis, you might set the
`pdflatex_path` meta variable.

- An installation of [Inkscape](https://inkscape.org/).
It is assumed that the `inkscape` command is in the path and can be
executed from any location. Alternatively, the environment
variable `INKSCAPE` can be set with a path. If a specific
version per pandoc document is to be used, the `inkscape_path`
meta-variable can be set.

In order to use additional LaTeX packages, use the optional
`additionalPackages` attribute in your document, as in the
example below.

Example usage from [TikZ
examples](http://www.texample.net/tikz/examples/parallelepiped/) by
[Kjell Magne Fauske](http://www.texample.net/tikz/examples/nav1d/):

```{.tikz caption="This is an image, created by **TikZ i.e. LaTeX**."
     additionalPackages="\usepackage{adjustbox}"}
\usetikzlibrary{arrows}
\tikzstyle{int}=[draw, fill=blue!20, minimum size=2em]
\tikzstyle{init} = [pin edge={to-,thin,black}]

\resizebox{16cm}{!}{%
  \trimbox{3.5cm 0cm 0cm 0cm}{
    \begin{tikzpicture}[node distance=2.5cm,auto,>=latex']
      \node [int, pin={[init]above:$v_0$}] (a) {$\frac{1}{s}$};
      \node (b) [left of=a,node distance=2cm, coordinate] {a};
      \node [int, pin={[init]above:$p_0$}] at (0,0) (c)
        [right of=a] {$\frac{1}{s}$};
      \node [coordinate] (end) [right of=c, node distance=2cm]{};
      \path[->] (b) edge node {$a$} (a);
      \path[->] (a) edge node {$v$} (c);
      \draw[->] (c) edge node {$p$} (end) ;
    \end{tikzpicture}
  }
}
```

### Python
In order to use Python to generate an diagram, your Python code must store the
final image data in a temporary file with the correct format. In case you use
matplotlib for a diagram, add the following line to do so:

```python
plt.savefig("$DESTINATION$", dpi=300, format="$FORMAT$")
```

The placeholder `$FORMAT$` gets replace by the necessary format. Most of the
time, this will be `png` or `svg`. The second placeholder, `$DESTINATION$`
gets replaced by the path and file name of the destination. Both placeholders
can be used as many times as you want. Example usage from the [Matplotlib
examples](https://matplotlib.org/gallery/lines_bars_and_markers/cohere.html#sphx-glr-gallery-lines-bars-and-markers-cohere-py):

```{.py2image caption="This is an image, created by **Python**."}
import matplotlib
matplotlib.use('Agg')

import sys
import numpy as np
import matplotlib.pyplot as plt

# Fixing random state for reproducibility
np.random.seed(19680801)

dt = 0.01
t = np.arange(0, 30, dt)
nse1 = np.random.randn(len(t))                 # white noise 1
nse2 = np.random.randn(len(t))                 # white noise 2

# Two signals with a coherent part at 10Hz and a random part
s1 = np.sin(2 * np.pi * 10 * t) + nse1
s2 = np.sin(2 * np.pi * 10 * t) + nse2

fig, axs = plt.subplots(2, 1)
axs[0].plot(t, s1, t, s2)
axs[0].set_xlim(0, 2)
axs[0].set_xlabel('time')
axs[0].set_ylabel('s1 and s2')
axs[0].grid(True)

cxy, f = axs[1].cohere(s1, s2, 256, 1. / dt)
axs[1].set_ylabel('coherence')

fig.tight_layout()
plt.savefig("$DESTINATION$", dpi=300, format="$FORMAT$")
```

Precondition to use Python is a Python environment which contains all
necessary libraries you want to use. To use, for example, the standard
[Anaconda Python](https://www.anaconda.com/distribution/) environment
on a Microsoft Windows system ...

- set the environment variable `PYTHON` or the meta key `pythonPath`
to `c:\ProgramData\Anaconda3\python.exe`

- set the environment variable `PYTHON_ACTIVATE` or the meta
key `activate_python_path` to `c:\ProgramData\Anaconda3\Scripts\activate.bat`.

Pandoc will activate this Python environment and starts Python with your code.

## Asymptote
[Asymptote](https://asymptote.sourceforge.io/) is a graphics
language inspired by Metapost. To use Asymptote, you will need to
install the software itself, a TeX distribution such as
[TeX Live](https://www.tug.org/texlive/), and
[dvisvgm](https://dvisvgm.de/), which may be included in the TeX
distribution.

If png output is required (such as for the `docx`, `pptx` and `rtf`
output formats) Inkscape must be installed. See the Ti*k*Z section
for details.

Ensure that the Asymptote `asy` binary is in the path, or point
the environment variable `ASYMPTOTE` or the metadata variable
`asymptotePath` to the full path name. Asymptote calls the various
TeX utilities and dvipdfm, so you will need to configure Asymptote
so that it finds them.

```{.asymptote caption="This is an image, created by **Asymptote**."}
size(5cm);
include graph;

pair circumcenter(pair A, pair B, pair C)
{
  pair P, Q, R, S;
  P = (A+B)/2;
  Q = (B+C)/2;
  R = rotate(90, P) * A;
  S = rotate(90, Q) * B;
  return extension(P, R, Q, S);
}

pair incenter(pair A, pair B, pair C)
{
  real a = abs(angle(C-A)-angle(B-A)),
       b = abs(angle(C-B)-angle(A-B)),
       c = abs(angle(A-C)-angle(B-C));
  return (sin(a)*A + sin(b)*B + sin(c)*C) / (sin(a)+sin(b)+sin(c));
}

real dist_A_BC(pair A, pair B, pair C)
{
  real det = cross(B-A, C-A);
  return abs(det/abs(B-C));
}

pair A = (0, 0), B = (5, 0), C = (3.5, 4),
     O = circumcenter(A, B, C),
     I = incenter(A, B, C);
dot(A); dot(B); dot(C); dot(O, blue); dot(I, magenta);
draw(A--B--C--cycle, linewidth(2));
draw(Circle(O, abs(A-O)), blue+linewidth(1.5));
draw(Circle(I, dist_A_BC(I, A, B)), magenta+linewidth(1.5));
label("$A$", A, SW);
label("$B$", B, SE);
label("$C$", C, NE);
label("$O$", O, W);
label("$I$", I, E);
```

## How to run pandoc
This section will show, how to call Pandoc in order to use this filter with
meta keys. The following command assume, that the filters are stored in the
subdirectory `filters`. Further, this is a example for a Microsoft Windows
system.

Command to use PlantUML (a single line):

```
pandoc.exe README.md -f markdown -t docx --self-contained --standalone --lua-filter=filters\diagram-generator.lua --metadata=plantumlPath:"c:\ProgramData\chocolatey\lib\plantuml\tools\plantuml.jar" --metadata=javaPath:"c:\Program Files\Java\jre1.8.0_201\bin\java.exe" -o README.docx
```

All available environment variables:

- `PLANTUML` e.g. `c:\ProgramData\chocolatey\lib\plantuml\tools\plantuml.jar`; Default: `plantuml.jar`
- `INKSCAPE` e.g. `c:\Program Files\Inkscape\inkscape.exe`; Default: `inkscape`
- `PYTHON` e.g. `c:\ProgramData\Anaconda3\python.exe`; Default: n/a
- `PYTHON_ACTIVATE` e.g. `c:\ProgramData\Anaconda3\Scripts\activate.bat`; Default: n/a
- `JAVA_HOME` e.g. `c:\Program Files\Java\jre1.8.0_201`; Default: n/a
- `DOT` e.g. `c:\ProgramData\chocolatey\bin\dot.exe`; Default: `dot`
- `PDFLATEX` e.g. `c:\Program Files\MiKTeX 2.9\miktex\bin\x64\pdflatex.exe`; Default: `pdflatex`
- `ASYMPTOTE` e.g. `c:\Program Files\Asymptote\asy`; Default: `asy`

All available meta keys:

- `plantuml_path`
- `inkscape_path`
- `python_path`
- `activate_python_path`
- `java_path`
- `dot_path`
- `pdflatex_path`
- `asymptote_path`
