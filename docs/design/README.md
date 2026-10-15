# raipixel design

Baseline revised 2026-10-15. **Proposed contracts, not implemented functionality.** Interface version 1, project format 1, export schema 1, engine semantics `1.0`. In this specification MUST/MUST NOT are requirements; SHOULD is a default requiring a documented reason to depart; research, judgments, estimates, and future candidates are labeled separately.

| Document | Responsibility |
|---|---|
| [Research](01-research.md) | Verified capability categories, evidence limitations, standards and format evaluation |
| [Vision and scope](02-vision-scope.md) | Product promise, priorities, exclusions and admission criteria |
| [Domain](03-domain-model.md) | Normative state, identity, coordinates, preservation and limits |
| [Architecture](04-architecture.md) | Normative module boundaries, codecs, rendering and storage design |
| [Functional specification](05-functional-specification.md) | Normative behavior and reclassification of every F01–F25 requirement |
| [Agent interface](06-agent-interface.md) | Normative commands, JSON requests/results and persistence/export schemas |
| [Roadmap](07-implementation-plan.md) | Capability dependencies and end-to-end acceptance gates |
| [Decisions](08-decisions.md) | ADRs, alternatives and consequences |
| [Risks](09-risks.md) | Residual uncertainty and required evidence |
| [Recommendation](10-recommended-design.md) | Significant changes, removed complexity and open questions |

Supporting contracts: [usage examples](examples/README.md), [acceptance and comparative validation](acceptance-scenarios.md), [four-perspective adversarial review](review/README.md), and [command JSON Schema](command-schema.json).

Domain owns state invariants; architecture owns codec/render/publication policy; functional specification owns algorithms and prioritization; interface owns wire shapes and field defaults. The JSON Schema checks structural command syntax; cross-field, reference, and pixel semantics remain normative in the prose. Resolve contradictions before implementing the affected behavior.

**Foundation** completes the first static creation/import loop. **Core** completes useful structured production, animation and reuse. **Advanced** is valuable later scope. **Deferred** needs evidence or disproportionate engineering. **Excluded** is outside the product. The first useful toolkit comprises the Foundation and Core gates A–E, not every feature in this design. No dated release, LOC quota or executable-size promise is made.
