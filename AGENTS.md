# AGENTS — Project Entry Point

**Level 1 — Constitution (SOUL) v4.1 hardened:**

→ **[AGENTS Constitution/AGENTS.md](AGENTS%20Constitution/AGENTS.md)**

**Level 2 — Operating law (process):**

→ **[AGENTS Constitution/SOP.md](AGENTS%20Constitution/SOP.md)**

**Governance (pack integrity):**

→ [RULE-REGISTRY.md](AGENTS%20Constitution/RULE-REGISTRY.md) · [MODULE-INDEX.md](AGENTS%20Constitution/MODULE-INDEX.md) · [INTEGRITY.md](AGENTS%20Constitution/INTEGRITY.md)  
→ Verify: `pwsh -File "AGENTS Constitution/tools/verify-pack.ps1"`

**Level 3 — Modules (load by task):**

```
AGENTS Constitution/
├── constitution/     Core law, authority, human–AI contract, DoD
├── standards/        Engineering, testing, security, docs, perf, deps, observability
├── operations/       Delivery, VCS, releases, refactor, migrations, lifecycle
├── collaboration/    Multi-agent, context switch, review packaging, memory
├── specialist/       Novel R&D, IP, networking, low-level, constrained HW
├── templates/        MANIFEST, ADR, audit, delivery report, project override
└── tools/            verify-pack.ps1
```

**Always load (any contribution):** `AGENTS.md` · `SOP.md` · `constitution/03-DEFINITION-OF-DONE.md` · `standards/ENGINEERING.md` · `standards/TESTING.md` · `standards/DOCUMENTATION.md`

**Level 4 — Project law:** this repo’s local docs, PRDs, ADRs, task plans (may tighten, not silently weaken `CONST-*` / `ENG-WARN-001` / `TEST-BEHAVIOR-001` / `SEC-INPUT-001` without `PROJECT-OVERRIDE`).

This root file exists so tools and new AIs discover the law immediately.  
Do not duplicate the constitution here.

**Evolution**

| Artifact | Method |
|----------|--------|
| SOUL | Complete replace `AGENTS Constitution/AGENTS.md` |
| Process | Complete replace `AGENTS Constitution/SOP.md` |
| Modules | Complete replace the affected module file |
| Rule IDs | Update `RULE-REGISTRY.md` in the same delivery |
| Pack structure | Update `MODULE-INDEX.md` + `INTEGRITY.md`; run verify-pack |
