# Questioning orthodoxy


## Functions

The idea of a 'function' as used in programming stems from mathematics and predates computers by centuries. Apparently $f(x)$ notation invented by Euler in 1734.

<https://en.wikipedia.org/wiki/Function_(mathematics)>
<https://en.wikipedia.org/wiki/History_of_the_function_concept>

## Stack

The idea of a computational stack is so pervasive that processors have dedicated instructions for them. As we will see however, it is completely unecessary for a rich computing environment. Rust, despite all it's safety measure, can not preclude stack overflows. Oluś will never stack overflow.

## Types

Invented by Alfred North Whitehead and Bertrand Russel to fix a zero-day remote code execution in Mathematics' kernel, types are substantially more recent than functions, but they still predate the modern concept of a computer. Furthermore, the Curry-Howard correspondence suggests an equivalence between proofs, types and programs. Part of the original motivation of this language was to create a minimal programming language, a minimal proof language (Metamath style) and unify the two.

TODO: Short modern CS perspective from Entsheidungsproblem till Turing completeness.

Also, proof systems.

## Sequence of symbols

Formal languages are based on a sequence of symbols. This is likely inherited from written versions of spoken languages, which in turn reflect the serial nature of sound.

In reality directed graphs are a much more generic representation of arbitrary structures. And graph fragments are the objects we want to reason about.

This is heavily inspired by [Penrose notation][penrose] and the [dataflow programming][dataflow] languages LabVIEW and Simulink. Reading Penrose's Road to Reality while working with LabVIEW made me realize the orthodoxy of formal languages.

[penrose]: https://en.wikipedia.org/wiki/Penrose_graphical_notation
[dataflow]: https://en.wikipedia.org/wiki/Dataflow_programming

The irony is that mathematicians are no strangers to non-linear notation. Formula in modern math can contain all sorts of subscripts, superscripts, matrices and other two-dimensional layouts. Most of that is an algamation of unstructured conventions. Gottlob Frege's Begriffsschrift is notably two dimensional and tree-like. Same with Gentzen's natural deduction and sequent calculus.

<https://en.wikipedia.org/wiki/Begriffsschrift>
<https://gallica.bnf.fr/ark:/12148/bpt6k65658c/f5.image>

TODO: <https://en.wikipedia.org/wiki/Cirquent_calculus>

Whitehead and Russel's Principia Mathematica is noticeably serial.

Generally, when you see parenthesis `(` and `)` being used in languages, this is just a poor attempt at forcing a tree structure to look serial. Usually a complicated system of infix notation and precedence rules is introduced to then avoid some of the parenthesis.

Even worse, parenthesis only allow you to express trees. To express directed acyclic structures, further trickery is required such as intermediate expressions that are then combined. Or worse, the structure is turned into a tree by repetition.

When forced to represent inherently cyclic structures as a sequence of symbols, 'bound variables' are introduced, followed by complex rules around free and bound variables, α-conversion, α-equivalence, variable shadowing, capture avoiding substitution, macro hygiene, etc.

<https://en.wikipedia.org/wiki/Hygienic_macro>


One way to solve all this is using Combinatory logic, similar to point-free style popular in Haskell <https://en.wikipedia.org/wiki/Combinatory_logic> <https://en.wikipedia.org/wiki/Tacit_programming>.

<https://en.wikipedia.org/wiki/De_Bruijn_index>

Oluś does not break away from this convention completely; it's source files are still sequences of symbols to facilitate editing using existing tooling. But the first step after parsing is to turn the source into a giant graph.

