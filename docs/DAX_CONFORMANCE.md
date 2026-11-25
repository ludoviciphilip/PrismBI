# Conformidade DAX medida (corpus)

> Gerado por `scripts/dax-conformance-report.mjs` a partir de `tests/dax-corpus/`. Não edite à mão.
> Cada caso tem resultado esperado conferido de forma independente dos motores.

**Casos:** 368 · **Rust (principal):** 364/368 (98.9%) · **TypeScript (legado):** 350/368 (95.1%)

| Categoria | Casos | Rust | TS |
| :--- | ---: | ---: | ---: |
| core | 49 | 49 (100.0%) | 48 (98.0%) |
| filter | 67 | 63 (94.0%) | 56 (83.6%) |
| scalar | 110 | 110 (100.0%) | 106 (96.4%) |
| stats | 73 | 73 (100.0%) | 71 (97.3%) |
| time | 69 | 69 (100.0%) | 69 (100.0%) |

## Casos com falha (18)

| Caso | Rust | TS |
| :--- | :--- | :--- |
| `core.rankx.client` | ✅ | ❌ 1 |
| `filter.allselected.share_of_selected_caneta` | ❌ 0.08370044052863436 | ❌ 1 |
| `filter.allselected.share_of_selected_mouse` | ❌ 0.28193832599118945 | ❌ 1 |
| `filter.bidir.crossfilter_none_blocks_propagation` | ✅ | ❌ 2 |
| `filter.iscrossfiltered.no_bidirectional_propagation_false` | ✅ | ❌ true |
| `filter.rel.blankrow.values_column_includes` | ❌ 2 | ❌ 2 |
| `filter.rel.blankrow.values_table_includes` | ❌ 2 | ❌ 0 |
| `filter.table.calculatetable_countrows` | ✅ | ❌ 4 |
| `filter.table.summarize_countrows_categories` | ✅ | ❌ 1 |
| `filter.topn.ties_expand_rows` | ✅ | ❌ 3 |
| `filter.treatas.virtual_relationship_category` | ✅ | ❌ null |
| `filter.values.countrows_table_no_dedup` | ✅ | ❌ 0 |
| `scalar.date.arithmetic.subtract_one_crosses_month_end` | ✅ | ❌ erro: Não foi possível converter o texto "2024-03-01" em número |
| `scalar.info.isblank.empty_string_is_false` | ✅ | ❌ true |
| `scalar.text.format.month_abbrev` | ✅ | ❌ "2024-03-05" |
| `scalar.text.format.thousands_pattern` | ✅ | ❌ "1.234.567" |
| `stats.rankx.blank_target_ranks_as_zero` | ✅ | ❌ null |
| `stats.rankx.sum_without_calculate_ties_all` | ✅ | ❌ 8 |
