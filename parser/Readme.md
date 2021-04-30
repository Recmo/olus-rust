# Oluś Parser
## Tokenization

Tokenization follows the recomendations in TR31. In particular regarding
identifiers, syntax and whitespace.

https://www.unicode.org/reports/tr31/

Identifiers are matched in NFKC normalized case folded form. If the
non-normalized forms are not identicial, this is an error.

## Simple Sugar conversion is done on the AST

## TODO

* Port to Logos
* Incremental parsing
* Benchmarking

## Resources

<https://arzg.github.io/lang/10/>
