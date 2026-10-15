# Design baseline

Prepared 2026-10-14. Product: **raipixel**. Proposed contract versions: recipe 1, snapshot 1, JSON schema 1, engine semantics 1.0. This is a documentation-only design proposal; the commands shown are proposed interfaces, not installed software.

## Read in this order

1. [Capability research and user needs](01-research.md)
2. [Product vision and scope](02-vision-scope.md)
3. [Technical domain model](03-domain-model.md)
4. [Architecture](04-architecture.md)
5. [Functional specification](05-functional-specification.md)
6. [Agent interface specification](06-agent-interface.md)
7. [Implementation plan](07-implementation-plan.md)
8. [Architectural decision records](08-decisions.md)
9. [Risk assessment](09-risks.md)
10. [Final recommended design](10-recommended-design.md)

Supporting material: [ten interface examples](examples/README.md), [scenario acceptance catalogue](acceptance-scenarios.md), and [independent adversarial review](review/README.md).

## Authority and interpretation

Documents 3, 5 and 6 are normative. Document 3 owns state invariants; 5 owns algorithm and feature behavior; 6 owns syntax, serialization and process behavior. Other documents explain decisions and implementation order. If an apparent contradiction cannot be resolved through that ownership, stop the affected implementation and correct the specification in a separate commit; do not silently choose a meaning.

**Qualitative research** means relevant capabilities and firsthand reports were assessed; source identifiers are intentionally omitted, so individual observations cannot be verified from these documents. **User report** means a first-person report, not a controlled experiment. **Judgment** means a design choice. **Estimate** means an unmeasured planning range. A historical report may motivate a workflow test but cannot establish a current external product defect.

The full selected release is milestones M1–M6; M1 is an early usable slice, not the promised complete suite. No implementation may weaken the dependency constraint to shorten the plan. No feature is implicitly included merely because it exists elsewhere.
