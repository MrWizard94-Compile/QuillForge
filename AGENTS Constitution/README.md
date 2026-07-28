# AGENTS Constitution — Human Workspace

Reusable **empire-wide** quality + process pack for any project under the AI Empire model.

**Pack version:** aligned with `AGENTS.md` **v4.1.0** (hardened)

## Start here

1. **[AGENTS.md](AGENTS.md)** — Level 1 constitution (SOUL)  
2. **[SOP.md](SOP.md)** — Level 2 project-to-product process  
3. **[RULE-REGISTRY.md](RULE-REGISTRY.md)** — all Rule IDs  
4. **[MODULE-INDEX.md](MODULE-INDEX.md)** — all modules  
5. **[INTEGRITY.md](INTEGRITY.md)** — pack integrity law  
6. **[Modularize.md](Modularize.md)** — design rationale  

## Always load (AI)

* `AGENTS.md`  
* `SOP.md`  
* `constitution/03-DEFINITION-OF-DONE.md`  
* `standards/ENGINEERING.md`  
* `standards/TESTING.md`  
* `standards/DOCUMENTATION.md`  

Then load modules per the applicability matrix in `AGENTS.md`.

## Verify pack integrity

```powershell
pwsh -File tools/verify-pack.ps1
pwsh -File tools/self-audit.ps1   # run the system against itself
```

`verify-pack` must exit `0` before publishing pack amendments (`GOV-INT-001`).  
`self-audit` applies Section 0 to the pack and writes `reports/SELF-AUDIT-latest.md`.

## Layout

```
AGENTS.md  SOP.md  RULE-REGISTRY.md  MODULE-INDEX.md  INTEGRITY.md
constitution/  standards/  operations/  collaboration/  specialist/
templates/  tools/
```

## Archive

Pre-modular monolith and old workflow: `_archive_pre_v4/`

## Evolution

* SOUL → complete replace `AGENTS.md`  
* Process → complete replace `SOP.md`  
* Modules → complete replace the affected file  
* New Rule IDs → update `RULE-REGISTRY.md` in the same delivery  
* New modules → update `MODULE-INDEX.md` + `INTEGRITY.md` required list  

Drop this folder into any project (or keep as the shared empire pack) and point the project root `AGENTS.md` at it.
