---
diagram:
  cache: false
  engine:
    tikz:
      execpath: pdflatex
      header-includes:
        - '\usetikzlibrary{arrows, shapes}'
---

### Ti*k*Z

Example usage from [TikZ
examples](http://www.texample.net/tikz/examples/parallelepiped/) by
[Kjell Magne Fauske](http://www.texample.net/tikz/examples/nav1d/):

```{.tikz
    caption="Tetrahedron inscribed in a parallelepiped."
    filename="parallelepiped"
    opt-additional-packages="\usepackage{adjustbox}"}
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

Diagram showing how the delta-graph relates to the other graphs.
Note that this diagram does not have a caption, so it will be
rendered as a plain image instead of a figure.

``` {.tikz}
%%| label: delta-graph
%%| filename: delta-graph.pdf
%%| alt: Diagram showing how the delta-graph relates to the other graphs.
\tikzset{cat object/.style=   {node distance=4em}}

\begin{tikzpicture}[]
\node [cat object] (Del)                {$D$};
\node [cat object] (L)   [below of=Del] {$X$};
\node [cat object] (I)   [right of=L]   {$I$};
\node [cat object] (F)   [left of=L]    {$F$};

\draw [->] (Del) to node [left,near end]{$\scriptstyle{d_X}$}     (L);
\draw [->] (I)   to node [below]        {$\scriptstyle{x}$}       (L);
\draw [->] (Del) to node [above left]   {$\scriptstyle{d_{F}}$} (F);

\draw [->,dashed] (Del) to node {/}(I);
\end{tikzpicture}
```
