# WORKFLOW.md — Mandatory Engineering Stages

**Status:** Binding process law (read with `AGENTS.md`)  
**Scope:** Every Quillforge (and empire) delivery cycle  

When the human authorizes work without a narrower scope, execute these stages in order.  
When a milestone green-light is given mid-cycle, resume at **stage 7** for that milestone, then complete **8–10**.

---

## Stages

| # | Stage | Output |
|---|--------|--------|
| 1 | Research 10 well-known related projects | Comparable product/system notes |
| 2 | Decompose each into core components | Uniform component matrix |
| 3 | Study each project in depth | Journeys, patterns, pains, constraints |
| 4 | Synthesize a new system design | Vision, priorities, trust, threats, success criteria |
| 5 | Write foundational documentation | PRD, architecture, data, API, security, test plan |
| 6 | Plan engineering | Stack, milestones, DoD, tests, risks |
| 7 | Implement the plan | Complete, zero-warning, tested increments |
| 8 | Audit the codebase | Functional, security, deps, tests, ops |
| 9 | Harden the codebase | Validation, least privilege, evidence, rollback |
| 10 | Final audit and delivery | Verify, docs current, MANIFEST, handoff |

---

## 1. Research Projects

Select 10 mature projects that solve problems close to the one you are building.

For each project, record:

- Name and purpose  
- Target users  
- Core workflows  
- Key differentiators  
- Platform or architecture style  
- Strengths, weaknesses, and gaps  

---

## 2. Decompose Components

Break every project into comparable pieces:

- Frontend or user-facing experience  
- Core domain objects and rules  
- APIs and integrations  
- Persistence and state handling  
- Security, auth, and permissions  
- Automation and background processing  
- Observability and admin tools  

---

## 3. Deep Study

For each project document:

- Main user journey  
- Important screens or entry points  
- Repeated patterns and system behaviors  
- Performance or scalability clues  
- Security or compliance characteristics  
- Missing features or pain points  

---

## 4. Synthesize New Design

Outputs:

- Product vision  
- Feature priorities  
- User stories  
- Trust boundaries  
- Data classification  
- Threat model notes  
- Success criteria  

Design must address trust boundaries, dependencies, data sensitivity, and auditability.

---

## 5. Foundational Documentation

Minimum docs (project may already have these; keep synchronized):

- Product requirements document  
- Functional specifications  
- UX/workflow maps  
- Architecture overview  
- Data model and schema notes  
- API documentation  
- Security and access-control policy  
- Test and release plan  

---

## 6. Engineering Plan

Include:

- Tech stack  
- Repository and branching strategy  
- Milestone plan  
- Definition of done  
- Test strategy  
- Dependency and release controls  
- Risk register  

---

## 7. Implement Plan

Build in increments starting with the smallest usable core.

Recommended order for application products:

1. Core data model  
2. Authentication and authorization (or local trust model)  
3. Primary user workflow  
4. Integrations and automation  
5. Reporting and admin tools  
6. Logging, monitoring, and release readiness  

For Quillforge specifically, honor the product roadmap: canonical path → recovery → layout → input → export/search → collab → plugins → AI.

Every increment must satisfy `AGENTS.md` Section 0 (complete, zero-warning, tested, documented).

---

## 8. Codebase Audit

Checklist:

- Functional correctness  
- Security  
- Dependency and supply-chain  
- Test coverage  
- Logging and observability  
- Performance and reliability  

---

## 9. Harden Codebase

- Remove known vulnerabilities  
- Tighten permissions / least privilege  
- Improve input validation and error handling  
- Add abuse protection where network exists  
- Strengthen logging and evidence capture  
- Improve deployment and rollback controls  

---

## 10. Final Audit and Delivery

- Re-run tests and static analysis  
- Verify documentation is current  
- Confirm release and rollback readiness  
- Validate operational signals  
- Package MANIFEST + handoff materials  

---

## Quillforge current baseline (for resumption)

| Complete | Next green-light targets |
|----------|---------------------------|
| Research + design + plan (`docs/research/`) | **M3** layout goldens |
| M1 session + package path | **M4** input↔session wiring |
| M2 untrusted ZIP + layout hash + citations | Then M5+ per engineering plan |

---

*End of WORKFLOW.md*
