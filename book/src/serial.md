# The Serial Oluś language

Serial Oluś loves unicode. It follows the XIC spec for identifiers.

$$
\gdef\code#1{\mathtt{#1}}
$$

As this language is functional, the most obvious example is the factorial function (as opposed to the 'hello world' for imperative languages):

```
fact n return: isZero n base recurse
    base: return 1
    recurse: sub n 1 step1
        step1 m: fact m step2
        step2 f: mul n f return
```

Don't worry about the verbosity, this will get fixed when we add sugar to the syntax. I haven't used any here to drive home an important point: every line has a very specific shape. Take the first line for example

$$
\code{fact}\ \code{n}\ \code{ret}\ \code{:}\ \code{isZero}\ \code{n}\ \code{base}\ \code{recurse}
$$

this is read as

$$
\overbrace{
\overbrace{
    \underbrace{
        \code{fact}
    } _ {\text{name}}
    {\ }
    \underbrace{
        \code{n}
        {\ }
        \code{ret}
    } _ {\text{parameters}}
}^{\text{procedure}}
\overbrace{
    {\ }
    \code{:}
    {\ }
}^{\text{maplet}}
\overbrace{
    \underbrace{
        \code{isZero}
    } _ {\text{closure}}
    {\ }
    \underbrace{
        \code{n}
        {\ }
        \code{base}
        {\ }
        \code{recurse}
    } _ {\text{arguments}}
}^{\text{call}}
}^{\text{declaration}}
$$

In other languages you would write it as

*Python*:
```python
def fact(n, ret):
    isZero(n, base, recurse)
```

*Javascript*:
```js
function fact(n, ret) {
    isZero(n, base, recurse);
}
```

In contrast to other languages, the Oluś language is strictly continuation passing style. This means that:

* Every procedure has exactly one call in its function body.
* There are no return values.
* Every call is a tail call.
* There is no stack.

Oh, and the indentation and line order are completely arbitrary. The following is an identical implementation:

```python
base: ret 1
fact n ret: isZero n base recurse
recurse: sub n 1 step1
step1 m: fact m step2
step2 f: mul n f ret
```

## Functions and return values

The language does not have a return statement, but we can add one by adhering to a simple convention:

**Definition**: A procedure is *function* if its last parameter is 'eventually' called. The arguments it is called with are the *return values*.

In Oluś this last parameter is conventionally called `return`. In CPS literature one often finds `k`.

**Definition**: A *function* is *total* if it always eventually calls its last argument.

Our factorial above is an example of a total function. The following is not total:

```
div n m fail return: isZero m fail step1
    # Division algorithm
```

This division algorithm




After adding a bit of syntax sugar this will become:

```
fact n return:
    isZero n (: return 1) (:)
    return (mul n (fact (sub n 1)))
```

Or, using lisp style names for the build in operations:

```
fact n return:
    if (= n 0) (: return 1) (:)
    return (* n (fact (- n 1)))
```

And with Church numerals it becomes a nice one-liner:

```
fact n return: n (: return 1) (m: return (* n (fact m)))
```
