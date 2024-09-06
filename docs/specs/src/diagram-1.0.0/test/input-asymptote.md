## Asymptote

```{.asymptote
    caption="This is an image, created by **Asymptote**."
    filename='triangle'}
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
