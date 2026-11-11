"""The exported page-structure document's version, shared by its consumers.

The Rust contract is the only owner of the table; this module is the one
consumer-side pin of its identity, so a version bump cannot leave a reader
silently interpreting a newer export with an older vocabulary rule.
"""

PAGE_STRUCTURE_EXPORT_VERSION = 14
