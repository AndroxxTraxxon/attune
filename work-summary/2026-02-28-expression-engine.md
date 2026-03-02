# Expression Engine for Workflow Templates

**Date**: 2026-02-28
**Scope**: `crates/common/src/workflow/expression/`, `crates/executor/src/workflow/context.rs`

## Summary

Implemented a complete expression evaluation engine for workflow templates, replacing the previous simple dot-path variable lookup and string-based condition evaluation with a full recursive-descent parser and evaluator operating over JSON values.

## Motivation

Workflows previously had no way to perform calculations, comparisons, or transformations within template expressions (`{{ }}`). The `evaluate_condition()` method could only check if a rendered string was truthy — it couldn't evaluate expressions like `result().code == 200 and succeeded()` or `length(items) > 3`. Publish directives and task inputs had no way to compute derived values.

## Architecture

The engine follows a classic three-phase interpreter design, located in `crates/common/src/workflow/expression/`:

1. **Lexer** (`tokenizer.rs`) — converts expression strings into tokens
2. **Parser** (`parser.rs`) — recursive-descent parser producing an AST
3. **Evaluator** (`evaluator.rs`) — walks the AST against an `EvalContext` trait to produce `JsonValue` results

The `EvalContext` trait decouples the expression engine from the `WorkflowContext`, allowing it to be used in other contexts (e.g., rule condition evaluation, template resolver) in the future.

## Operator Precedence (lowest to highest)

1. `or`
2. `and`
3. `not` (unary)
4. `==`, `!=`, `<`, `>`, `<=`, `>=`, `in`
5. `+`, `-`
6. `*`, `/`, `%`
7. Unary `-`
8. Postfix: `.field`, `[index]`, `(args)`

## Supported Operators

### Arithmetic
- `+` — number addition, string concatenation, array concatenation
- `-`, `*`, `/`, `%` — standard numeric operations
- Unary `-` — negation
- Integer division returns float when not evenly divisible (e.g., `10 / 4` → `2.5`, `10 / 5` → `2`)

### Comparison
- `==`, `!=` — deep equality (recursive for objects/arrays, cross-type for int/float)
- `>`, `<`, `>=`, `<=` — ordering for numbers, strings, and lists
- No implicit type coercion: `"3" == 3` → `false`

### Boolean
- `and`, `or` — short-circuit evaluation
- `not` — logical negation

### Membership & Access
- `.` — object property access
- `[n]` — array index (supports negative indexing), object bracket access, string character access
- `in` — membership test (item in array, key in object, substring in string)

## Built-in Functions

### Type Conversion
`string(v)`, `number(v)`, `int(v)`, `bool(v)`

### Introspection
`type_of(v)`, `length(v)`, `keys(obj)`, `values(obj)`

### Math
`abs(n)`, `floor(n)`, `ceil(n)`, `round(n)`, `min(a,b)`, `max(a,b)`, `sum(arr)`

### String
`lower(s)`, `upper(s)`, `trim(s)`, `split(s, sep)`, `join(arr, sep)`, `replace(s, old, new)`, `starts_with(s, prefix)`, `ends_with(s, suffix)`, `match(pattern, s)` (regex)

### Collection
`contains(haystack, needle)`, `reversed(v)` (arrays and strings), `sort(arr)`, `unique(arr)`, `flat(arr)`, `zip(a, b)`, `range(n)` / `range(start, end)`, `slice(v, start, end)`, `index_of(haystack, needle)`, `count(haystack, needle)`, `merge(obj_a, obj_b)`, `chunks(arr, size)`

### Workflow-specific (via `EvalContext`)
`result()`, `succeeded()`, `failed()`, `timed_out()`

## Design Decisions

- **No implicit type coercion**: Arithmetic between strings and numbers is an error. Only int/float cross-comparison is allowed (e.g., `3 == 3.0` → `true`).
- **Python-like truthiness**: `null`, `false`, `0`, `""`, `[]`, `{}` are falsy; everything else is truthy.
- **Integer preservation**: Operations on two integers produce integers; operations involving any float produce floats. Integer division that isn't evenly divisible promotes to float.
- **Array literals**: Supported in expressions: `[1, 2, 3]` (with optional trailing comma).
- **Negative array indexing**: `arr[-1]` returns the last element.

## Integration with WorkflowContext

- `WorkflowContext` implements the `EvalContext` trait, bridging variable resolution (`parameters`, `item`, `index`, `system`, `task`, `variables`, and direct variable names) and workflow functions (`result()`, `succeeded()`, `failed()`, `timed_out()`)
- `evaluate_expression()` now delegates entirely to the expression engine
- `evaluate_condition()` tries the expression engine first (for bare expressions like `x > 5`), with fallback to template rendering for backward compatibility with `{{ }}` wrapper syntax
- All existing tests pass without modification — backward compatibility is fully preserved

## Test Coverage

- **63 tests** in `crates/common/src/workflow/expression/` covering tokenizer, parser, and evaluator
- **28 tests** in `crates/executor/src/workflow/context.rs` (17 existing + 11 new) covering integration with WorkflowContext
- New tests cover: condition comparisons, boolean operators, `in` operator, function calls in conditions, `length()` in conditions, arithmetic in templates, string concatenation, nested function calls, bracket access, and type conversion

## Files Changed

- **Created**: `crates/common/src/workflow/expression/mod.rs` — module entry point and integration tests
- **Created**: `crates/common/src/workflow/expression/ast.rs` — AST node types
- **Created**: `crates/common/src/workflow/expression/tokenizer.rs` — lexer
- **Created**: `crates/common/src/workflow/expression/parser.rs` — recursive-descent parser
- **Created**: `crates/common/src/workflow/expression/evaluator.rs` — AST evaluator with all built-in functions
- **Modified**: `crates/common/src/workflow/mod.rs` — added `expression` module
- **Modified**: `crates/executor/src/workflow/context.rs` — integrated expression engine, replaced old evaluate_expression/evaluate_condition, implemented EvalContext trait, removed unused get_nested_value, added new tests