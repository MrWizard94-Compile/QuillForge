# AGENTS — Project Entry Point (QuillForge)

This file is **Level 4 project law entry only**. It points at the universal AGENTS Constitution pack. It does not duplicate the constitution.

**Pack path (relative):** [`AGENTS Constitution/`](AGENTS%20Constitution/)  
**Pack version:** **5.0.1** — see [`AGENTS Constitution/VERSION`](AGENTS%20Constitution/VERSION)

---

## Binding pack

| Role | Path |
|------|------|
| **Constitution (SOUL)** | [AGENTS Constitution/AGENTS.md](AGENTS%20Constitution/AGENTS.md) |
| **Process (SOP)** | [AGENTS Constitution/SOP.md](AGENTS%20Constitution/SOP.md) |
| **Pack identity / portability** | [AGENTS Constitution/PACK.md](AGENTS%20Constitution/PACK.md) |
| **Lock** | [AGENTS Constitution/LOCK.md](AGENTS%20Constitution/LOCK.md) |
| **Adopt / move** | [AGENTS Constitution/ADOPT.md](AGENTS%20Constitution/ADOPT.md) |
| **Rule registry** | [AGENTS Constitution/RULE-REGISTRY.md](AGENTS%20Constitution/RULE-REGISTRY.md) |
| **Module index** | [AGENTS Constitution/MODULE-INDEX.md](AGENTS%20Constitution/MODULE-INDEX.md) |
| **Integrity** | [AGENTS Constitution/INTEGRITY.md](AGENTS%20Constitution/INTEGRITY.md) |

---

## Always load (via pack)

* Pack `AGENTS.md`  
* Pack `SOP.md`  
* Pack `constitution/03-DEFINITION-OF-DONE.md`  
* Pack `standards/ENGINEERING.md`  
* Pack `standards/TESTING.md`  
* Pack `standards/DOCUMENTATION.md`  

Then load pack modules per the applicability matrix in pack `AGENTS.md`.

---

## Layout (pack)

```
AGENTS Constitution/
├── VERSION · PACK.md · ADOPT.md · README.md · LOCK.md
├── AGENTS.md · SOP.md
├── RULE-REGISTRY.md · MODULE-INDEX.md · INTEGRITY.md
├── constitution/ · standards/ · operations/
├── collaboration/ · specialist/ · templates/
├── tools/ · docs/ · reports/
```

---

## Verify pack

```powershell
pwsh -File "AGENTS Constitution/tools/verify-pack.ps1"
pwsh -File "AGENTS Constitution/tools/self-audit.ps1"
```

Must exit `0` after pack install, move, or update (`GOV-INT-001`).

---

## Level 4 — QuillForge project law

* Product specs / research: `docs/` (and related project docs)  
* Architecture / ADRs: project-local as maintained  
* Product surface priority: `docs/28-product-surface.md` (Word-class editor, WPAI brand only)  
* Overrides: pack `templates/PROJECT-OVERRIDE.template.md` (store under this project if used)  
* **New Python projects:** bootstrap with sibling [RepoForge](../RepoForge/) using  
  `--constitution "AGENTS Constitution"` (or absolute pack path) so Level-4 `AGENTS.md` is attached  

**QuillForge note:** Honor canonical path `Command → Operations → Journal → Kernel → Derived` where applicable.

Project law may **tighten** standards. It may not weaken `CONST-*`, `ENG-WARN-001`, `TEST-BEHAVIOR-001`, or `SEC-INPUT-001` without a documented override.

---

## Evolution

| Artifact | Method |
|----------|--------|
| SOUL | Complete replace `AGENTS Constitution/AGENTS.md` |
| Process | Complete replace `AGENTS Constitution/SOP.md` |
| Modules | Complete replace the affected module file |
| Rule IDs | Update `RULE-REGISTRY.md` in the same delivery |
| Pack structure | Update `MODULE-INDEX.md` + `INTEGRITY.md` + `VERSION` / `PACK.md`; run verify-pack |
| This pointer | Update if the relative pack path changes |
