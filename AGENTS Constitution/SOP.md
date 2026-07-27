# Standard Operating Procedure: Project-to-Product Delivery

**Document ID:** SOP-PROD-001  
**Version:** 1.0.1  
**Effective Date:** 2026-07-27  
**Owner:** Project Lead  
**Approved By:** Management / Product Owner  
**Status:** Binding (with AGENTS.md)  
**Supersedes:** WORKFLOW.md  

This SOP is process law. Quality law remains **[AGENTS.md](AGENTS.md)** (Section 0 gate, zero-warning, complete deliveries).

---

## Purpose

Define the standard process for researching reference projects, designing a new system, engineering the solution, auditing and hardening the codebase, and delivering the final product—so execution is consistent and quality-controlled.

## Scope

Applies to all teams and contributors in this workspace (product, design, engineering, QA, security, delivery). Covers research through final delivery and handoff. Post-launch support is out of scope unless separately assigned.

## References

- Secure SDLC: requirements, planning, design, implementation, testing/deployment, maintenance as connected phases  
- Effective SOPs: structured, step-based, roles, responsibilities, revision history  
- Quillforge product specs: `docs/`  
- Empire constitution: `AGENTS.md` in this folder  

## Definitions

| Term | Meaning |
|------|---------|
| **Reference projects** | External projects studied for product direction |
| **Core components** | Building blocks: UI, domain logic, data, integrations, security |
| **Hardening** | Reducing risk via security, reliability, validation before release |
| **Final audit** | Last formal review of code, docs, and release readiness before delivery |

## Roles and Responsibilities

| Role | Responsibility |
|------|----------------|
| **Project Lead** | Scope, priorities, approvals, final delivery |
| **Product Owner** | User value, feature priorities, acceptance criteria |
| **Architect / Lead Engineer** | Research → technical design and engineering direction |
| **Engineers** | Implement approved plan; fix issues |
| **QA / Reviewer** | Functionality, fixes, release readiness |
| **Security Reviewer** | Vulnerabilities, access, hardening |

For AI contributors under AGENTS.md: AI owns implementation detail; human is sovereign director.

---

## Procedure

### 1. Research 10 Well-Known Projects

1.1 Identify 10 projects relevant to product direction.  
1.2 Record purpose, target users, key features, platform, strengths.  
1.3 Document relevance to this workspace.  
1.4 Store findings in a shared research log (`docs/research/`).

### 2. Decompose Projects to Core Components

2.1 Break each project into comparable categories.  
2.2 At minimum: UI, workflow logic, data model, integrations, permissions, analytics, admin.  
2.3 Note visible dependencies between components.  
2.4 Use a standard format for direct comparison.

### 3. Study Each Project in Depth

3.1 Review user flows, architecture clues, feature behavior.  
3.2 Identify strengths, weaknesses, friction, unique design choices.  
3.3 Capture cross-project patterns.  
3.4 Record insights for new system design.

### 4. Design a New System

4.1 Synthesize strongest features into a coherent product.  
4.2 Define vision, target users, core use cases.  
4.3 Prioritized feature set.  
4.4 Initial user journeys and system/trust boundaries.  
4.5 Confirm design direction before full documentation.

### 5. Write Foundational Documentation

5.1 Core docs for implementation (PRD, functional, architecture, data model, API, security, test strategy).  
5.2 Consistency with approved design.  
5.3 Review/signoff before engineering (or incremental milestone approval for green-lit slices).

### 6. Plan Engineering

6.1 Convert docs into implementation plan.  
6.2 Milestones, dependencies, delivery phases.  
6.3 Definition of done per milestone.  
6.4 Risks, unknowns, required decisions.  
6.5 Plan approval before coding (or green-light per milestone).

### 7. Implement Engineering Plan

7.1 Build in prioritized increments.  
7.2 Core data model and primary workflows first.  
7.3 Auth, integrations, reporting, admin in planned phases.  
7.4 Maintain coding standards, tests, and docs during implementation (**AGENTS.md Section 0**).  
7.5 Track milestones; update stakeholders.

**Quillforge note:** Honor canonical path `Command → Operations → Journal → Kernel → Derived`.

### 8. Audit Codebase and Apply Fixes

8.1 Functional defects, security issues, architecture drift.  
8.2 Automated tests, linting, dependency checks, security scans.  
8.3 Triage by severity.  
8.4 Fix and re-test.  
8.5 Document audit results and remediation.

### 9. Harden Codebase

9.1 Reduce attack surface.  
9.2 Validation, error handling, logging, observability.  
9.3 Authorization, secrets, deployment controls.  
9.4 Performance and reliability weak points.  
9.5 Confirm release-ready stability.

### 10. Final Audit, Fix Pass, and Delivery

10.1 Final review of code, docs, release package.  
10.2 Critical findings resolved or formally accepted.  
10.3 Release readiness, rollback, handoff materials.  
10.4 Deliver to approved channel.  
10.5 Archive documentation; record completion.

---

## Inputs and Outputs

### Inputs

- Project goals and scope  
- Research sources  
- Stakeholder requirements  
- Technical constraints  
- Security and compliance requirements  

### Outputs

- Research summary  
- Component decomposition matrix  
- Deep-dive notes  
- System design proposal  
- Foundational documentation set  
- Engineering plan  
- Implemented codebase  
- Audit and hardening reports  
- Final release package (MANIFEST)  

---

## Quality Controls

- Major steps produce written outputs.  
- Reviews before phase advance (or explicit green-light for a milestone).  
- Security and quality checks required before delivery.  
- Changes traceable to findings, decisions, or requirements.  
- **AGENTS.md** zero-warning / complete-delivery rules apply to every increment.  
- **Git:** commit after discrete changes; push at end of work turn when directed.  

---

## Quillforge baseline (resumption)

| Complete | Next |
|----------|------|
| Research + design + plan (`docs/research/`) | M9 plugin host |
| M0–M8 (session, package, layout, editor, export, render, CRDT) | M9 → M10 ship |

---

## Revision History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-07-27 | Initial SOP (as WORKFLOW.md content). |
| 1.0.1 | 2026-07-27 | Renamed to SOP.md; cleaned structure; linked AGENTS.md; Quillforge baseline. |

## Approval

- **Prepared By:** Project Lead  
- **Reviewed By:** Architect / Lead Engineer  
- **Approved By:** Product Owner / Management  

---

*End of SOP-PROD-001*
