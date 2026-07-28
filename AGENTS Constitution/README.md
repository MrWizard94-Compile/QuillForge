# AGENTS Constitution — Universal Pack

**Universal · reusable · movable** quality + process law for human–AI engineering empires.

| | |
|--|--|
| **Pack version** | **5.0.0** (`VERSION`) |
| **SOUL** | [AGENTS.md](AGENTS.md) |
| **Process** | [SOP.md](SOP.md) |
| **Identity / portability** | [PACK.md](PACK.md) |
| **How to adopt or move** | [ADOPT.md](ADOPT.md) |

This folder is self-contained. **Move or rename it freely.** Re-run verify after every copy/move.

---

## Start here

1. [PACK.md](PACK.md) — what this pack is  
2. [ADOPT.md](ADOPT.md) — install / point / move  
3. [AGENTS.md](AGENTS.md) — Level 1 constitution  
4. [SOP.md](SOP.md) — Level 2 delivery process  
5. [RULE-REGISTRY.md](RULE-REGISTRY.md) · [MODULE-INDEX.md](MODULE-INDEX.md) · [INTEGRITY.md](INTEGRITY.md)  

---

## Always load (AI)

* `AGENTS.md`  
* `SOP.md`  
* `constitution/03-DEFINITION-OF-DONE.md`  
* `standards/ENGINEERING.md`  
* `standards/TESTING.md`  
* `standards/DOCUMENTATION.md`  

Then load modules per the applicability matrix in `AGENTS.md`.

---

## Verify (path-independent)

From any cwd:

```powershell
pwsh -File tools/verify-pack.ps1
pwsh -File tools/self-audit.ps1
pwsh -File tools/run-full-constitution-self.ps1
```

Or:

```powershell
pwsh -File tools/verify-pack.ps1 -PackRoot "D:\anywhere\AGENTS-Constitution"
```

| Tool | Writes | Gate |
|------|--------|------|
| `verify-pack.ps1` | console | `GOV-INT-001` |
| `self-audit.ps1` | `reports/SELF-AUDIT-latest.md` | `CONST-GATE-001` meta |
| `run-full-constitution-self.ps1` | `reports/FULL-CONSTITUTION-SELF-RUN-latest.md` | Full law + SOP |
| `write-checksums.ps1` | `reports/CHECKSUMS.sha256` | Optional release aid |

All must be runnable after the pack is moved (`GOV-PORT-001`).

---

## Layout

```text
VERSION  PACK.md  ADOPT.md  README.md
AGENTS.md  SOP.md  RULE-REGISTRY.md  MODULE-INDEX.md  INTEGRITY.md
constitution/  standards/  operations/  collaboration/  specialist/
templates/  tools/  docs/  reports/
```

---

## Four levels (reminder)

1. **Constitution** — this pack’s `AGENTS.md` + `constitution/`  
2. **Operating law** — `SOP.md`  
3. **Modules** — standards / operations / collaboration / specialist  
4. **Project law** — consumer repo docs + optional pointer (`templates/PROJECT-POINTER.template.md`)  

---

## Evolution

* SOUL → complete replace `AGENTS.md`  
* Process → complete replace `SOP.md`  
* Modules → complete replace the affected file  
* New Rule IDs → update `RULE-REGISTRY.md` same delivery  
* Structure change → update `MODULE-INDEX.md`, `INTEGRITY.md`, `PACK.md` / `VERSION`  
* After change → `verify-pack.ps1` exit 0  

Design history: [Modularize.md](Modularize.md).

---

*Drop this folder anywhere. Point projects at it. Enforce it. Move it without rewriting the law.*
