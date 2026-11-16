# Rule bindings

A binding puts a Rule id next to code. An Implementation binding marks the
primary production code of a Rule, and a Verification binding marks a test.
The binding does not define the Rule; a Rule may exist before any code is bound
to it. The scanner reads source files line by line and does not parse a syntax
tree.

Production code never needs Provenance as a dependency. A test dependency is a
development dependency, so test code can import the helpers below.

## Production code: a comment

In each of the six supported languages, put a comment directly above the
function, method, or `const` that implements the Rule:

```ts
// @provenance rule: rule_overtime
export const paysOvertime = (hours: number) => hours > 38;
```

```rust
// @provenance rule: rule_overtime
pub fn pays_overtime(hours: u32) -> bool {
    hours > 38
}
```

```python
# @provenance rule: rule_overtime
def pays_overtime(hours: int) -> bool:
    return hours > 38
```

The comment needs no import and no package. An install that omits development
dependencies, such as `npm ci --omit=dev`, loads the application. The comment
does not move with its function the way a compiled symbol does, so keep the two
together when you move code.

The scanner reads the marker only on a comment line: after leading whitespace,
the line starts with `//`, `/*`, or `*` (or `#` in Python), or the line is
inside an open block comment. A marker after code on the same line does not
bind. The comment binds the Rule whatever item comes below it.

The item name of the comment is the name of the declaration that starts
directly below it. Comments, empty lines, attributes, decorators, and
annotations between the comment and the declaration do not change the name,
and a declaration can take any number of lines. Bindings above one declaration
all get the name of that declaration, so in

```rust
// @provenance rule: rule_overtime
#[inline]
#[rule("rule_hours")]
pub fn pays_overtime(hours: u32) -> bool {
```

the comment and the attribute both get `pays_overtime`.

A binding gets a name only when it is at the start of a statement: at the
start of the file, or after `;`, `{`, or `}`. In Go, JavaScript, and
TypeScript, the end of a line also ends a statement when the line ends with a
word, a string, `)`, or `]`, outside parentheses and square brackets. In
Python, a statement starts at the start of a logical line. A binding inside a
statement has no name: for example a binding inside the arguments of a call,
after an operator, or between the parts of a conditional expression
`ready ? start(a) : stop`.

The scanner does not parse a syntax tree. The scanner removes each comment
span, from `/*` to `*/` or from `//` (or `#` in Python) to the end of the line,
and reads the text that is left as code tokens. The first tokens of the
statement below the binding must make one of the declaration forms below, or
the binding has no name:

- Rust: `fn`, `struct`, `enum`, `union`, `trait`, or `type` and the name, after
  `pub` or `pub(...)`, `default`, `async`, `unsafe`, `extern` with its ABI
  string, and `const` before `fn`. A raw identifier `r#match` gives `match`.
- Python: `def` or `class` and the name, after `async`.
- Go: `type` and the name; `func`, the name, and `(` or `[`; or `func`, a
  receiver, the name, and `(`. A function literal such as
  `func() error { ... }()` has no name.
- JavaScript, TypeScript: after decorators and the modifiers `export`,
  `default`, `declare`, `abstract`, `async`, `public`, `private`, `protected`,
  `static`, `readonly`, `override`, and `accessor`:
  - `function` or `function*`, `class`, `interface`, `type`, `enum`,
    `const enum`, `namespace`, `const`, `let`, or `var`, and the name. `let
    first, second` gives `first`.
  - A method: an optional `*`, `get`, or `set`, the name, type parameters, and
    a parameter list that a return type or a `{` on the line of its `)`
    follows. So `public async total(): Promise<number> {` gives `total`.
  - A property after a class modifier such as `private` or `readonly`: the
    name, and a type, a value, or `;`. So `private count = 0;` gives `count`.
  - These words in the place of a method name or a property name give no
    name: `if`, `for`, `while`, `switch`, `catch`, `return`, `throw`, `new`,
    `await`, `yield`, `typeof`, `delete`, `void`, `do`, `else`, `try`,
    `finally`, `with`, `super`, `this`, `import`, `case`, and `async`.
- Java: after annotations and modifiers:
  - `class`, `interface`, `enum`, `record`, or `@interface`, and the name.
  - A method: type parameters, a return type, the name, and `(`. A return type
    can be a qualified name with type arguments, so
    `public java.util.List<String> names() {` gives `names`.
  - A constructor, or an enum constant with a body: the name, its arguments,
    and `{` or `throws`. So `ONE(1) {` in an enum gives `ONE`.
  - A field after `public`, `protected`, `private`, `static`, `transient`, or
    `volatile`: a type, the name, and `=`, `;`, or `,`.
  - A Java keyword such as `if`, `return`, `new`, `throw`, `yield`, or `var`
    in the place of the type or the name gives no name.

Limits: these forms give no item name on purpose, because the tokens at the
start of the statement do not make a declaration form above, or make the same
tokens as a statement:

- Rust: an `impl` block, a `const`, a `static`, a `mod`, a `macro_rules!`, a
  struct field, and an enum variant.
- Python: an assignment.
- Go: a `type (` group, a `var`, a `const`, and a function literal.
- JavaScript, TypeScript: an arrow function, also in a `const` with the name of
  the `const`, `export default function () {}`, an optional method
  `pays?(hours: number): boolean;`, a property with no class modifier such as
  `total = () => {`, a method named `async` or a statement keyword above, and
  a method whose `{` is on a line below its `)`: the tokens are the same as a
  call statement that a block follows. A test call such as
  `test("hours above 38", () => {` and a `describe(...)` call give no name.
- Java: a field with no modifier or with only `final`, because a local
  variable has the same tokens; an enum constant with no body, such as
  `TWO(2),`, because a call statement has the same tokens; a statement such as
  a call, a qualified call `Collections.<String>emptyList();`, or
  `synchronized (lock) {`.
- A binding inside a statement, as above.
- A Rust attribute directly below a binding comment is not scanned as a
  binding: the comment takes in the attribute line.

A marker in a block comment that code follows on the same line, such as
`/* @provenance rule: rule_overtime */ function pays() {}`, does not bind
correctly: the scanner takes the rest of the line into the Rule id. Put the
comment on its own line.

A Rust `#[rule]` or `#[verifies]` attribute and a Python `@rule` decorator take
their item name by the same rules. A call does not:

- In JavaScript and TypeScript, `rule("id", ...)` and `verifies("id",
  "method")` take the name of the assignment on their line, or on the line
  above when that line ends with `=`. A `verifies` call that has no assignment
  takes the name of the function around it.
- In Go and Java, `rule("id", ...)` takes the name of the assignment on its
  line, or on the line above when that line ends with `=`. So Go
  `x := rule("rule_overtime", 1)` and Java
  `int x = rule("rule_overtime", 1);` give `x`.

These rules read words on lines, not the declaration forms, so they can give a
word that is not the name of a declaration.

## Test code

A test can use the same comment, with its verification method on the line
above the Rule line:

```python
# @provenance verification: examples
# @provenance rule: rule_overtime
def test_overtime_examples() -> None:
    ...
```

The method is one of `exhaustion`, `property`, `examples`, `conformance`,
`construction`, or `proof`. Put a `construction` comment directly above the type
that it verifies, not above a test. The comment needs no package in the
application; the item names of a type follow the list above.

In TypeScript and JavaScript, a test can call `verifies` from the SDK's `rules`
subpath. Install the SDK as a development dependency (the TypeScript
initializer does this):

```sh
npm install --save-dev @quality-sh/provenance
```

```ts
import { verifies } from "@quality-sh/provenance/rules";
import { paysOvertime } from "./pay.js";

test("hours above 38 attract overtime", function overtimeExamples() {
  verifies("rule_overtime", "examples");
  expect(paysOvertime(39)).toBe(true);
});
```

`verifies` returns nothing, registers no state, and does not change the test.
Put `verifies("id", "method")` in a named function or in a function-valued
`const`; the scanner takes the name of that function. Import `verifies` only in
test code. A free `verifies(` call that does not read as a quoted id and one
method word gives the warning ``malformed verifies binding `<line>`: expected a
quoted rule id and one method word: exhaustion, property, examples, conformance,
construction, or proof``. A member call such as `x.verifies(` that does not read
that way gives no warning and does not bind.

In Rust, a test takes `#[verifies]` from `provenance-macros` under
`[dev-dependencies]`. `cargo provenance init` adds `provenance-sdk` under
`[dev-dependencies]`, and it re-exports the same attribute as
`provenance_sdk::verifies`:

```sh
cargo add --dev provenance-macros
```

```rust
use provenance_macros::verifies;

#[test]
#[verifies("rule_overtime", examples)]
fn hours_above_38_attract_overtime() {
    assert!(pays_overtime(39));
}
```

## Rust attribute alternative

Rust production code can use `#[rule("id")]` in place of the comment. The
attribute comes from `provenance-macros`, which then goes under
`[dependencies]`:

```sh
cargo add provenance-macros
```

```rust
use provenance_macros::rule;

#[rule("rule_overtime")]
pub fn pays_overtime(hours: u32) -> bool {
    hours > 38
}
```

`provenance-macros` is a procedural macro crate. Cargo builds it, with `syn`,
`quote`, and `proc-macro2`, for the build machine and runs it at compile time.
The attribute gives back its item unchanged, so the crate adds no code and no
run-time dependency to the binary. In return, the attribute is a compiled
symbol: it moves with its item, it goes when the item is deleted, and the build
stops with "cannot find attribute `rule`" when the `use` line is missing.
`#[verifies("id", construction)]` sits on a type, not on a test, so it also
needs `provenance-macros` under `[dependencies]`, or the comment form.

`provenance-sdk` re-exports both attributes for test code, but it depends on
the Provenance store. Do not add `provenance-sdk` under `[dependencies]` to get
`#[rule]`.

`#[rule("id")]` and `#[verifies("id", method)]` bind in three spellings:
plain, qualified through a crate path
(`#[provenance_macros::rule("id")]`, and any `path::rule` or `path::verifies`
name), and wrapped across lines the way rustfmt formats long arguments:

```rust
use provenance_macros::verifies;

#[verifies(
    "rule_this_is_a_long_id_after_rustfmt",
    examples
)]
fn wrapped() {}
```

The wrapped form closes within sixteen lines and holds no comments. A
string, a raw string, or a block comment that contains attribute text never
binds. The scanner does not read `cfg_attr` rewrites.

## Helpers in the project's own code

The scanner also reads a helper that a Python, TypeScript, JavaScript, Go, or
Java project defines in its own code. Such a helper is project code and adds no
dependency. The comment stays the default form.

### Python decorator

```python
from collections.abc import Callable
from typing import ParamSpec, TypeVar

P = ParamSpec("P")
R = TypeVar("R")


def rule(_rule_id: str) -> Callable[[Callable[P, R]], Callable[P, R]]:
    def bind(function: Callable[P, R]) -> Callable[P, R]:
        return function

    return bind


@rule("rule_overtime")
def pays_overtime(hours: int) -> bool:
    return hours > 38
```

The scanner recognizes `@rule("id")` directly above a `def`. The scanner does
not read a Python verification decorator; a Python test uses the comment.

### TypeScript wrapper

```ts
// src/rules.ts
export function rule<Implementation>(
  _ruleId: string,
  implementation: Implementation,
): Implementation {
  return implementation;
}
```

```ts
import { rule } from "./rules.js";

export const paysOvertime = rule("rule_overtime", (hours: number) => hours > 38);
```

The scanner recognizes a same-line `rule("id",` call, free or through a member
such as `rules.rule(`, and takes the `const` on the left of `=` as the item
name. A call that has an object after the id, such as
`requirement.rule("key", { statement })` in a spec, declares a Rule and does
not bind. The same reading applies to JavaScript.

### Go wrapper

```go
func rule[Function any](_ string, function Function) Function {
	return function
}

var paysOvertime = rule("rule_overtime", func(hours int) bool {
	return hours > 38
})
```

The scanner recognizes a same-line `rule("id",` call and takes the variable on
the left of `=` as the item name. Verification calls are not recognized in Go.

### Java static helper

```java
import java.util.function.IntPredicate;

final class ProvenanceRules {
    private ProvenanceRules() {}

    static <Function> Function rule(String ruleId, Function function) {
        return function;
    }
}

final class PayrollRules {
    private static final IntPredicate PAYS_OVERTIME =
        ProvenanceRules.rule("rule_overtime", hours -> hours > 38);
}
```

The scanner recognizes `rule("id",` after a plain or qualified helper name and
uses the assigned field as the item name. Java verification helpers are not
recognized.

## What the repository scan covers

A scan of a directory tree covers the files a repository tracks. Three
directory names always stay out: `.git`, `node_modules`, and `target`. The
scan also prunes the generated output trees that the repository itself
declares: a `.gitignore` file in the scanned root, or in any directory below
it, keeps its ignored directories out of the scan. The scan reads
`.gitignore` rules itself, and it implements a working subset of the
gitignore grammar: comments, blank lines, `!` negation, a trailing `/`, and
the `*`, `**`, and `?` globs.

The walk prunes directories only. A `.gitignore` rule that names one file
does not hide that file from the scan. Escaped characters, character
classes, and `.gitignore` files above the scanned root have no effect.
A directory name alone never marks output: an unignored `dist` directory of
hand-written source stays in the scan. Naming the generated tree as the scan
root (`--path dist`) overrides the ignore rules and scans it.
