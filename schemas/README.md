# Versioned JSON contracts

The files in this directory are the frozen JSON Schema 2020-12 contracts for
the compatibility release. Schema identifiers in serialized documents map as
follows:

| Document identifier | Schema file |
|---|---|
| `underprint.capabilities/v1` | `capabilities-v1.schema.json` |
| `underprint.detection/v1` | `detection-v1.schema.json` |
| `underprint.embedding/v1` | `embedding-v1.schema.json` |
| `underprint.error/v1` | `error-v1.schema.json` |

Additive or breaking changes require a new document identifier and schema file.
Existing versioned files are immutable after a tagged compatibility release.
JSON numbers are never used for identifiers or payload bytes.
