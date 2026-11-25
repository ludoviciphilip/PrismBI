# Conformidade DAX medida (corpus)

> Gerado por `scripts/dax-conformance-report.mjs` a partir de `tests/dax-corpus/`. Não edite à mão.
> Cada caso tem resultado esperado conferido de forma independente dos motores.

**Casos:** 366 · **Rust (principal):** 300/366 (82.0%) · **TypeScript (legado):** 350/366 (95.6%)

| Categoria | Casos | Rust | TS |
| :--- | ---: | ---: | ---: |
| core | 49 | 47 (95.9%) | 48 (98.0%) |
| filter | 67 | 47 (70.1%) | 56 (83.6%) |
| scalar | 110 | 93 (84.5%) | 106 (96.4%) |
| stats | 71 | 60 (84.5%) | 71 (100.0%) |
| time | 69 | 53 (76.8%) | 69 (100.0%) |

## Casos com falha (73)

| Caso | Rust | TS |
| :--- | :--- | :--- |
| `core.iter.maxx_category` | ❌ 320 | ✅ |
| `core.rankx.client` | ✅ | ❌ 1 |
| `core.switch_true` | ❌ erro: Esperado ')' após argumentos de SWITCH | ✅ |
| `filter.allselected.share_of_selected_caneta` | ❌ 0.08370044052863436 | ❌ 1 |
| `filter.allselected.share_of_selected_mouse` | ❌ 0.28193832599118945 | ❌ 1 |
| `filter.bidir.crossfilter_none_blocks_propagation` | ✅ | ❌ 2 |
| `filter.earlier.rank_sum_via_filter` | ❌ erro: Função DAX 'EARLIER' ainda não implementada no motor nativo. | ✅ |
| `filter.iscrossfiltered.no_bidirectional_propagation_false` | ✅ | ❌ true |
| `filter.iscrossfiltered.table_via_other_column_true` | ❌ false | ✅ |
| `filter.keepfilters.intersect_empty_is_blank` | ❌ 215 | ✅ |
| `filter.lookupvalue.multikey_ambiguous_is_error` | ❌ null | ✅ |
| `filter.rel.blankrow.values_column_includes` | ❌ 2 | ❌ 2 |
| `filter.rel.blankrow.values_table_includes` | ❌ 2 | ❌ 0 |
| `filter.relatedtable.sumx_countrows_by_client` | ❌ erro: Expressão de tabela não suportada neste iterador | ✅ |
| `filter.relatedtable.sumx_countrows_by_product` | ❌ erro: Expressão de tabela não suportada neste iterador | ✅ |
| `filter.table.addcolumns_countrows` | ❌ erro: Expressão de tabela não suportada neste iterador | ✅ |
| `filter.table.calculatetable_countrows` | ❌ erro: Expressão de tabela não suportada neste iterador | ❌ 4 |
| `filter.table.crossjoin_countrows` | ❌ erro: Expressão de tabela não suportada neste iterador | ✅ |
| `filter.table.except_countrows_removes_papelaria` | ❌ erro: Expressão de tabela não suportada neste iterador | ✅ |
| `filter.table.intersect_countrows_no_overlap` | ❌ erro: Expressão de tabela não suportada neste iterador | ✅ |
| `filter.table.selectcolumns_countrows_no_dedup` | ❌ erro: Expressão de tabela não suportada neste iterador | ✅ |
| `filter.table.summarize_countrows_categories` | ❌ erro: Expressão de tabela não suportada neste iterador | ❌ 1 |
| `filter.table.union_countrows_no_dedup` | ❌ erro: Expressão de tabela não suportada neste iterador | ✅ |
| `filter.topn.ties_expand_rows` | ❌ 3 | ❌ 3 |
| `filter.treatas.virtual_relationship_category` | ❌ null | ❌ null |
| `filter.values.countrows_table_no_dedup` | ✅ | ❌ 0 |
| `scalar.date.arithmetic.subtract_one_crosses_month_end` | ❌ -1 | ❌ erro: Não foi possível converter o texto "2024-03-01" em número |
| `scalar.info.isblank.empty_string_is_false` | ✅ | ❌ true |
| `scalar.info.iserror.div_by_zero_is_true` | ❌ false | ✅ |
| `scalar.info.islogical.true` | ❌ erro: Esperado ')' após argumentos de ISLOGICAL | ✅ |
| `scalar.logical.and_operator` | ❌ erro: Token inesperado após expressão: LParen | ✅ |
| `scalar.logical.if_eager.same_as_if` | ❌ erro: Função DAX 'IF.EAGER' ainda não implementada no motor nativo | ✅ |
| `scalar.logical.iferror.intercepts_div_by_zero` | ❌ null | ✅ |
| `scalar.logical.not_function` | ❌ erro: Parêntese ')' esperado | ✅ |
| `scalar.logical.or_function` | ❌ erro: Esperado ')' após argumentos de OR | ✅ |
| `scalar.logical.or_operator` | ❌ erro: Token inesperado após expressão: LParen | ✅ |
| `scalar.logical.switch_true_form` | ❌ erro: Esperado ')' após argumentos de SWITCH | ✅ |
| `scalar.math.ceiling.negative_significance` | ❌ -18 | ✅ |
| `scalar.math.floor.negative_significance` | ❌ -21 | ✅ |
| `scalar.math.sqrt.negative_error` | ❌ null | ✅ |
| `scalar.text.find.not_found_error` | ❌ null | ✅ |
| `scalar.text.format.iso_date_pattern` | ❌ "2024-mm-05" | ✅ |
| `scalar.text.format.month_abbrev` | ✅ | ❌ "2024-03-05" |
| `scalar.text.format.percent_pattern` | ❌ "25.00%" | ✅ |
| `scalar.text.format.thousands_pattern` | ✅ | ❌ "1.234.567" |
| `scalar.text.search.wildcard` | ❌ null | ✅ |
| `stats.rankx.asc.min_value` | ❌ erro: Expressão inesperada: Comma | ✅ |
| `stats.rankx.asc.tie_30` | ❌ erro: Expressão inesperada: Comma | ✅ |
| `stats.rankx.dense.desc.tie_20` | ❌ erro: Expressão inesperada: Comma | ✅ |
| `stats.rankx.dense.desc.tie_30` | ❌ erro: Expressão inesperada: Comma | ✅ |
| `stats.rankx.desc.mid_value` | ❌ 11 | ✅ |
| `stats.rankx.desc.min_value` | ❌ 11 | ✅ |
| `stats.rankx.desc.tie_20` | ❌ 11 | ✅ |
| `stats.rankx.desc.tie_30` | ❌ 11 | ✅ |
| `stats.rankx.desc.value_15` | ❌ 11 | ✅ |
| `stats.xirr.cashflow` | ❌ erro: Função DAX 'XIRR' ainda não implementada no motor nativo. | ✅ |
| `stats.xnpv.cashflow_10pct` | ❌ erro: Função DAX 'XNPV' ainda não implementada no motor nativo. | ✅ |
| `time.endofmonth.from_leap_day` | ❌ null | ✅ |
| `time.endofmonth.leap_february_2024` | ❌ null | ✅ |
| `time.endofmonth.march_2024` | ❌ null | ✅ |
| `time.endofmonth.nonleap_february_2025` | ❌ null | ✅ |
| `time.endofquarter.q1_2025` | ❌ null | ✅ |
| `time.endofyear.2025` | ❌ null | ✅ |
| `time.firstdate.march_2024` | ❌ null | ✅ |
| `time.firstnonblank.quarter_single_sale` | ❌ null | ✅ |
| `time.firstnonblank.year_2024` | ❌ null | ✅ |
| `time.lastdate.march_2024` | ❌ null | ✅ |
| `time.lastnonblank.year_2024` | ❌ null | ✅ |
| `time.startofmonth.march_2024` | ❌ null | ✅ |
| `time.startofquarter.q1_2025` | ❌ null | ✅ |
| `time.startofyear.2024` | ❌ null | ✅ |
| `time.totalytd.fiscal_year_end_0630` | ❌ 574 | ✅ |
| `time.totalytd.fiscal_year_end_0630_second_half` | ❌ 501 | ✅ |
