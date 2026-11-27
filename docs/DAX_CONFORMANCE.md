# Conformidade DAX medida (corpus)

> Gerado por `scripts/dax-conformance-report.mjs` a partir de `tests/dax-corpus/`. Não edite à mão.
> Cada caso tem resultado esperado conferido de forma independente dos motores.

**Casos:** 412 · **Rust (principal):** 412/412 (100.0%) · **TypeScript (legado):** 361/412 (87.6%)

| Categoria | Casos | Rust | TS |
| :--- | ---: | ---: | ---: |
| calculated | 9 | 9 (100.0%) | 0 (0.0%) |
| core | 49 | 49 (100.0%) | 48 (98.0%) |
| filter | 82 | 82 (100.0%) | 60 (73.2%) |
| scalar | 110 | 110 (100.0%) | 106 (96.4%) |
| stats | 73 | 73 (100.0%) | 71 (97.3%) |
| time | 69 | 69 (100.0%) | 69 (100.0%) |
| window | 20 | 20 (100.0%) | 7 (35.0%) |

## Casos com falha (51)

| Caso | Rust | TS |
| :--- | :--- | :--- |
| `calculated.column.earlier_cheaper_in_category` | ✅ | ❌ null |
| `calculated.column.related_category_filters_sales` | ✅ | ❌ null |
| `calculated.measure.sum_over_calculated_table` | ✅ | ❌ null |
| `calculated.table.addcolumns_margin_sum` | ✅ | ❌ null |
| `calculated.table.calendar_is_flagged_for_calendarauto` | ✅ | ❌ null |
| `calculated.table.calendar_row_count` | ✅ | ❌ 0 |
| `calculated.table.summarize_category_avg_sum` | ✅ | ❌ null |
| `calculated.table.summarize_category_count` | ✅ | ❌ 0 |
| `calculated.table.union_keeps_duplicates` | ✅ | ❌ 0 |
| `core.rankx.client` | ✅ | ❌ 1 |
| `filter.allselected.column_not_in_shadow_clears` | ✅ | ❌ 45 |
| `filter.allselected.divide_share_three_selected` | ✅ | ❌ 1 |
| `filter.allselected.noargs_restores_whole_shadow` | ✅ | ❌ 84 |
| `filter.allselected.share_of_selected_caneta` | ✅ | ❌ 1 |
| `filter.allselected.share_of_selected_mouse` | ✅ | ❌ 1 |
| `filter.allselected.table_arg_restores_slicer` | ✅ | ❌ 95 |
| `filter.bidir.crossfilter_none_blocks_propagation` | ✅ | ❌ 2 |
| `filter.iscrossfiltered.no_bidirectional_propagation_false` | ✅ | ❌ true |
| `filter.keepfilters.calculatetable_arg_intersects` | ✅ | ❌ 1135 |
| `filter.rel.blankrow.addcolumns_over_values_includes` | ✅ | ❌ 0 |
| `filter.rel.blankrow.direct_filter_excludes_values_table` | ✅ | ❌ 0 |
| `filter.rel.blankrow.sumx_constant_counts_blank` | ✅ | ❌ 2 |
| `filter.rel.blankrow.values_column_categoria_includes` | ✅ | ❌ 2 |
| `filter.rel.blankrow.values_column_includes` | ✅ | ❌ 2 |
| `filter.rel.blankrow.values_table_includes` | ✅ | ❌ 0 |
| `filter.table.calculatetable_countrows` | ✅ | ❌ 4 |
| `filter.table.summarize_countrows_categories` | ✅ | ❌ 1 |
| `filter.topn.ties_expand_rows` | ✅ | ❌ 3 |
| `filter.treatas.virtual_relationship_category` | ✅ | ❌ null |
| `filter.tuple.summarize_bare_as_filter` | ✅ | ❌ 215 |
| `filter.tuple.treatas_pair_restricts_combo` | ✅ | ❌ 215 |
| `filter.values.countrows_table_no_dedup` | ✅ | ❌ 0 |
| `scalar.date.arithmetic.subtract_one_crosses_month_end` | ✅ | ❌ erro: Não foi possível converter o texto "2024-03-01" em número |
| `scalar.info.isblank.empty_string_is_false` | ✅ | ❌ true |
| `scalar.text.format.month_abbrev` | ✅ | ❌ "2024-03-05" |
| `scalar.text.format.thousands_pattern` | ✅ | ❌ "1.234.567" |
| `stats.rankx.blank_target_ranks_as_zero` | ✅ | ❌ null |
| `stats.rankx.sum_without_calculate_ties_all` | ✅ | ❌ 8 |
| `window.calendarauto.fiscal_july_days` | ✅ | ❌ 365 |
| `window.calendarauto.fiscal_july_start_year` | ✅ | ❌ null |
| `window.index.last_in_partition` | ✅ | ❌ 215 |
| `window.index.top_price_desc` | ✅ | ❌ 1135 |
| `window.offset.first_row_previous_is_blank` | ✅ | ❌ 95 |
| `window.offset.partitioned_boundary_is_blank` | ✅ | ❌ 320 |
| `window.offset.partitioned_next` | ✅ | ❌ 320 |
| `window.offset.previous_product_by_price` | ✅ | ❌ 320 |
| `window.rank.dense_with_ties` | ✅ | ❌ 1 |
| `window.rank.skip_with_ties` | ✅ | ❌ 1 |
| `window.rownumber.tie_broken_by_client_name` | ✅ | ❌ {"__isDaxTable":true,"columns":["Value","RowNumber"],"rows":[{"Value":{"__isModifier":true,"type":"ALL","tableId":"d_Cliente","tableName":"d_Cliente","columns":["Cliente","Cidade"]},"RowNumber":1}]} |
| `window.window.moving_two_items` | ✅ | ❌ erro: Não foi possível converter o texto "[object Object]" em núme |
| `window.window.running_total` | ✅ | ❌ erro: Não foi possível converter o texto "[object Object]" em núme |
