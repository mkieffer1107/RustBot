# RustBot vendored patch

Based on fancy-regex 0.16.2 from crates.io; upstream license is in LICENSE.

The internal delegate compiler shares identical live regex-automata programs using a bounded thread-local cache of weak references. Keys include the delegate source, all regex-automata syntax configuration fields, and both delegate size limits. No matching algorithm, builder defaults, validation limits, or prefilter settings are changed. Entries never keep programs alive; no-std builds compile without the cache.
