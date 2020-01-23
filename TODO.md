Phase 0:
* Codegen to x86_64 Mach-O exec without stack.
* Bump allocator for closures, no deallocation.
* Hardcoded closure size, function in header
* Closure & 64 bit register type.
* `add` `sub` `mul` `iszero` builtins
* Calling conventions: closure in r0, args in r1..r15, fail when >15 args.
* Deduplicate literals

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
