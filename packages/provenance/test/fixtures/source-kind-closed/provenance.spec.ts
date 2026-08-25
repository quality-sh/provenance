import { defineSpec, requirement, source } from "../../../dist/index.js";

// `integration` is not one of the canonical source types. Both fluent
// builder surfaces refuse it: the top-level `source(key)` and the
// spec-scoped `defineSpec(key).source(key)`. The low-level
// `source(key, options)` overload keeps a string kind.

const topLevel = source("brief").kind("integration");

const migration = defineSpec("bound");
const bound = migration.source("brief").kind("integration");

requirement("intake").statement("The catalogue records every citation").from(topLevel);
migration.requirement("intake").statement("The catalogue records every citation").from(bound);
