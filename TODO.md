Phase 0:
* Codegen to x86_64 Mach-O exec without stack. (DONE)
* Bump allocator for closures, no deallocation. (DONE)
* Hardcoded closure size, function in header (Not needed)
* Closure & 64 bit register type. (DONE)
* `add` `sub` `mul` `iszero` builtins (DONE)
* Calling conventions: closure in r0, args in r1..r15, fail when >13 args. (DONE)
* Deduplicate literals (DONE)
* Fully functional closure analysis

Phase 0.5:
* Create multiple and recursive closures
* Solve state transition overlap edge cases

Phase 1:
* Constant time reference counting.
* Computed fixed closure size.
* Enumerate possible procedures at call sites
* Inlining

Future:
* Deduplicate procedures
* Break up >16 arg functions into closures.
* Closure lifetime, multiplicity analysis.
* Calling convention without closure, allowing 16 args.
* Dynamic calling conventions, allowing different register ordering
* Calling convention involving non-general purpose registers:
  * XMM registers
  * Flags

Prover core:
* Prover core (Metamask ish)
  <https://arxiv.org/pdf/1910.10703.pdf>


Reading list:
* <https://arxiv.org/pdf/1910.10703.pdf>
* <https://bootstrappable.org/>
* <http://www.gii.upv.es/tlsf/files/ecrts04_tlsf.pdf>
