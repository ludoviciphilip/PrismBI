# Conformidade DAX medida (corpus)

> Gerado por `scripts/dax-conformance-report.mjs` a partir de `tests/dax-corpus/`. Não edite à mão.
> Cada caso tem resultado esperado conferido de forma independente dos motores.

**Casos:** 49 · **Rust (principal):** 47/49 (95.9%) · **TypeScript (legado):** 48/49 (98.0%)

| Categoria | Casos | Rust | TS |
| :--- | ---: | ---: | ---: |
| core | 49 | 47 (95.9%) | 48 (98.0%) |

## Casos com falha (3)

| Caso | Rust | TS |
| :--- | :--- | :--- |
| `core.iter.maxx_category` | ❌ 320 | ✅ |
| `core.rankx.client` | ✅ | ❌ 1 |
| `core.switch_true` | ❌ erro: Esperado ')' após argumentos de SWITCH | ✅ |
