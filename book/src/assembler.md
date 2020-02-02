# Assembler

The x86_64 processor's state can be described as follows

* the instruction pointer
* 16 general purpose registers
* a number of flags
* the contents of the memory

There are more state variables to the x86_64 processor, but these are only relevant for operating systems. For use processes like Oluś generates, we can ignore these. I have also omitted the MM registers, used for floating point and SIMD instructions. These can easily be added later as the language gains support for them.

A unique feature of Oluś is that all general purpose registers are treated equal. In conventional languages, the `rbp` and `rsp` are dedicated to managing the stack, but since Oluś has no stack, there is no need for this.

To further drive home the point that all registers are treated equally, they will be revered to by number as `r0` ... `r15` instead of their more common nicknames (`rax`, `rbx`, ...). The numbering used is the one used by the [dynasm crate][da-regs].

[da-regs]: https://censoredusername.github.io/dynasm-rs/language/langref_x64.html#register

## Calling convention

'Calling convention' is not really accurate, since we never use `call` or `ret` but instead `jmp` to the new procedure. Still, the name is most conventional for what is described here.

At the start of each procedure, a pointer to its current closure is in `r0`, arguments to the procedure are in `r1`, ... `r15`. The closure contains a pointer to the start of the procedure code followed by any closure variable values. If there are no closure variables it is allocated in read only memory.

If the machine state is set up as described, the procedure can be initiated using and indirect jump to the value pointed to be `r0`:

```asm
jmp [r0]
```

At the start of each procedure, all the machine state belongs to the procedure and it can do with it as it sees fit. This is a consequence there being no returns in CPS. The sole job of a procedure is to convert the current machine state in to that for its call.

### Ideas

The basic calling convention is good enough for a working implementation, but can be improved upon:

* No closures in `r0` when there are no closure variables.
* Pass current closure variables in registers.
* Different ordering of registers to avoid shuffling.
* Pass other closure variables in registers.
* Pass boolean values in flags.
* Use static direct jumps when the target is known compile time.
* Lay out code to use fall-through instead of static jumps.
* Pass through unclobbered values

With highly flexible calling conventions, we can implement zero-overhead intrinsics for most instructions. For example `adcx` takes reads and writes from the carry flag and `mulx` expects one of it's arguments in `r2`.

## Closure allocation

Closures are immutable and can reference previous closures. This creates a direccted acyclic graph of closures. Currently these are bumb-allocated and never freed.

### Ideas

* Use reference counting for the generic directed acyclic graph case.
* Make the language real-time using deferred free reference counting.
* Statically allocate closures where at most one instance can exist (these can now also be roots).
* Use stack allocation where the closure graph is strictly linear.
* Use slab allocation for closures of the same size (allocation sizes are always know compile time).
* Research special allocation algorithms for tree-like and dag-like data.
  (Observe bump allocation makes all pointers point exclusively backward.)

## Appendix: Apple Darwin-XNU system calls

> The fastest program is the shortest path between the ideal syscalls.

Darwin follows the [x64-abi][x86-64 ABI] appendix A.2.1 for system calls. The system call is selected using `r0`, the arguments are passed in `r7`, `r6`, `r2`, `r10`, `r8`, and `r9`. The return value is in `r0` and as an extension additional return values are passed in `r1`. In the process the values of `r11`. The system call is initiated with the `syscall` instruction.

[x64-abi]: https://github.com/hjl-tools/x86-psABI/wiki/X86-psABI

Since Apple does not support using the system calls directly, there is no official documentation. The Darwin source code however, contains a [good overview][dar-sys]. A value of `0x0200_0000` needs to be added to the call numbers. Besides the offset, they are POSIX inspired and correspond closely to, for example, the [Linux syscalls][lin-sys].

[dar-sys]: https://github.com/apple/darwin-xnu/blob/master/bsd/kern/syscalls.master
[lin-sys]: https://syscalls.kernelgrok.com/

Currently, Oluś doesn't offer much beyond reading and writing to standard input/output, which is done with the `read` and `write` system calls. Simple file IO can also be implemented this way. Async IO, sockets and threads look like they can work, will require some research. More complex operations like graphics will likely need dynamic linking and a substanitally more complex design. Hopefully this can be implemented as a library in the language.

## Resources

https://www.felixcloutier.com/x86/

https://www.agner.org/optimize/
