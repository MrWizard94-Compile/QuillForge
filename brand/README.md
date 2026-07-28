# Quillforge brand kit (WPAI)

**Studio:** Wizard Productions AI Studio (**WPAI** / **W.P.A.I**)  
**Product:** Quillforge (Software division)  
**Tagline:** Forging the future of creative media  
**Thesis:** AI is in the name. The wizard is in the work.

This folder is the **product-repo** brand surface. It mirrors the company kit under `C:\WPAI\Brand\` so Quillforge ships self-contained.

## Voice (do not dilute)

Authoritative voice copy: [BRAND-VOICE.md](BRAND-VOICE.md).

- Transparent, never confessional about AI assistance.  
- Unapologetic, not defensive.  
- Human-directed always — **Nothing ships untouched.**  
- Forge vernacular sparingly (fire, anvil, embers, forging).  
- Neurodivergence as arsenal (“weaponizing”), never obstacle.

## Visual law

| Token | Hex | Role |
|-------|-----|------|
| Ember | `#ff7a26` | Primary accent |
| Molten | `#e8451c` | Hot accent |
| Coal | `#0a0706` | Surface / background |
| Basalt | `#12100e` | Raised surface |
| Ash | `#e8e2dc` | Primary text on dark |
| Iron | `#6b635c` | Muted text |

- Dark cinematic forge aesthetic; **not** pastel SaaS or corporate blue.  
- No cartoon wizards or clip-art.  
- Code theme: `Theme::wpai_forge_dark()` in `qf-theme`.  
- Constants: `qf_core::brand`.

## Assets

| File | Use |
|------|-----|
| `assets/quillforge-product.png` | **Product mark** — GUI chrome, insert image/logo, demos (`PRODUCT_MARK_PNG`) |
| `assets/wpai-official-brand.jpg` | Studio mark with tagline |
| `assets/wpai-official-brand-alt.jpg` | Studio mark without tagline |
| `assets/wpai-official-backdrop.png` | Full backdrop |

The product mark is **compile-embedded** into `qf-app` via `include_bytes!` so the shell never requires a relative path at runtime.

## Product attribution lines

**CLI banner:** `Quillforge — WPAI · Forging the future of creative media`  
**Package app id:** `{semver} (WPAI Software)` via `qf_storage::APP_VERSION`  
**Disclosure (footer):** see `qf_core::STUDIO_DISCLOSURE`

## External

- Site: https://wpaistudio.net  
- Store: https://wpaistudio.gumroad.com  
- Contact: rob@wpaistudio.net  
- Founder: Rob Bulkley (Mrwizard94)
