# Conformidade DAX medida (corpus)

> Gerado por `scripts/dax-conformance-report.mjs` a partir de `tests/dax-corpus/`. Não edite à mão.
> Cada caso tem resultado esperado conferido de forma independente dos motores.

**Casos:** 366 · **Rust (principal):** 354/366 (96.7%) · **TypeScript (legado):** 350/366 (95.6%)

| Categoria | Casos | Rust | TS |
| :--- | ---: | ---: | ---: |
| core | 49 | 49 (100.0%) | 48 (98.0%) |
| filter | 67 | 57 (85.1%) | 56 (83.6%) |
| scalar | 110 | 110 (100.0%) | 106 (96.4%) |
| stats | 71 | 69 (97.2%) | 71 (100.0%) |
| time | 69 | 69 (100.0%) | 69 (100.0%) |

## Casos com falha (22)

| Caso | Rust | TS |
| :--- | :--- | :--- |
| `core.rankx.client` | ✅ | ❌ 1 |
| `filter.allselected.share_of_selected_caneta` | ❌ 0.08370044052863436 | ❌ 1 |
| `filter.allselected.share_of_selected_mouse` | ❌ 0.28193832599118945 | ❌ 1 |
| `filter.bidir.crossfilter_none_blocks_propagation` | ✅ | ❌ 2 |
| `filter.earlier.rank_sum_via_filter` | ❌ erro: Função DAX 'EARLIER' ainda não implementada no motor nativo. | ✅ |
| `filter.iscrossfiltered.no_bidirectional_propagation_false` | ✅ | ❌ true |
| `filter.rel.blankrow.values_column_includes` | ❌ 2 | ❌ 2 |
| `filter.rel.blankrow.values_table_includes` | ❌ 2 | ❌ 0 |
| `filter.table.addcolumns_countrows` | ❌ erro: Expressão de tabela não suportada neste iterador | ✅ |
| `filter.table.calculatetable_countrows` | ✅ | ❌ 4 |
| `filter.table.except_countrows_removes_papelaria` | ❌ 2 | ✅ |
| `filter.table.selectcolumns_countrows_no_dedup` | ❌ erro: Expressão de tabela não suportada neste iterador | ✅ |
| `filter.table.summarize_countrows_categories` | ❌ erro: Expressão de tabela não suportada neste iterador | ❌ 1 |
| `filter.topn.ties_expand_rows` | ✅ | ❌ 3 |
| `filter.treatas.virtual_relationship_category` | ❌ null | ❌ null |
| `filter.values.countrows_table_no_dedup` | ✅ | ❌ 0 |
| `scalar.date.arithmetic.subtract_one_crosses_month_end` | ✅ | ❌ erro: Não foi possível converter o texto "2024-03-01" em número |
| `scalar.info.isblank.empty_string_is_false` | ✅ | ❌ true |
| `scalar.text.format.month_abbrev` | ✅ | ❌ "2024-03-05" |
| `scalar.text.format.thousands_pattern` | ✅ | ❌ "1.234.567" |
| `stats.xirr.cashflow` | ❌ erro: Função DAX 'XIRR' ainda não implementada no motor nativo. | ✅ |
| `stats.xnpv.cashflow_10pct` | ❌ erro: Função DAX 'XNPV' ainda não implementada no motor nativo. | ✅ |
