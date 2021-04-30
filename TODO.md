# To do

Phase 0:

* Codegen to x86_64 Mach-O exec without stack. (DONE)
* Bump allocator for closures, no deallocation. (DONE)
* Hardcoded closure size, function in header (Not needed)
* Closure & 64 bit register type. (DONE)
* `add` `sub` `mul` `iszero` builtins (DONE)
* Calling conventions: closure in r0, args in r1..r15, fail when >13 args. (DONE)
* Deduplicate literals (DONE)
* Fully functional closure analysis (DONE)
* Create multiple closures (DONE)
* Support multipage code and rom (DONE)
* Only allocate const closures when closure is empty

Phase 0.5:

* Solve state transition overlap edge cases
  => Use Dijkstra's algorithm to find the optimal transition between two states.
* Elf64 output
* More examples:
  * Bottles of beer example <http://99-bottles-of-beer.net/>
  * Binary trees from <https://benchmarksgame-team.pages.debian.net/benchmarksgame/description/binarytrees.html#binarytrees>
  * Select examples from
    * <https://rosettacode.org/wiki/Category:Programming_Tasks>
      * <https://rosettacode.org/wiki/Y_combinator>
    * <http://mlton.org/Performance>
* More intrinsics:
  * Read stdin
  * Closure compare
  * fork

Phase 1:

* Constant time reference counting.
* Computed fixed closure size.
* SLAB allocator
* Enumerate possible procedures at call sites
* Inlining
* A way to split source over multiple files

Future:

* Deduplicate procedures
* Break up >16 arg functions into closures.
* Closure lifetime, multiplicity analysis.
* Calling convention without closure, allowing 16 args.
* Dynamic calling conventions, allowing different register ordering
* Calling convention involving non-general purpose registers:
  * XMM registers
  * Flags
* Thread creationg (Linux clone, BSD bsdthread_create)

Prover core:

* Prover core (Metamask ish)
  See MM0 <https://arxiv.org/pdf/1910.10703.pdf>


Reading list:

* <https://arxiv.org/pdf/1910.10703.pdf>
* <https://bootstrappable.org/>
* <http://www.gii.upv.es/tlsf/files/ecrts04_tlsf.pdf>
* <https://paperhub.s3.amazonaws.com/24842c95fb1bc5d7c5da2ec735e106f0.pdf>
* <http://compilers.cs.uni-saarland.de/papers/lkh15_cgo.pdf>
* <https://os.phil-opp.com/allocator-designs/>
* <https://news.ycombinator.com/item?id=22372847>
