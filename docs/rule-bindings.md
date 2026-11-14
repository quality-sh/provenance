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
dependencies, such as `npm ci --omit=dev`, still loads the application. The
comment binds to the first function declared in the six lines below it. It does
not move with its function the way a compiled symbol does, so keep the two
together when you move code.

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
`construction`, or `proof`.

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
test code. The scanner does not read a `rule(` call in TypeScript or
JavaScript: production code uses the comment.

In Rust, a test takes `#[verifies]` from `provenance-macros` under
`[dev-dependencies]`:

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

The scanner also reads a helper that a Python, Go, or Java project defines in
its own code. Such a helper is project code and adds no dependency. The comment
stays the default form.

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
