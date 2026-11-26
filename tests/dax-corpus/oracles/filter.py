#!/usr/bin/env python3
"""Oraculo independente para a categoria 'filter' do corpus de conformidade DAX.

Carrega os modelos JSON em tests/dax-corpus/models/ (vendas, filter_rel,
filter_bidir) e recalcula, com Python puro e logica explicita (sem reusar
nenhum dos dois motores), os valores esperados usados em
tests/dax-corpus/cases/filter.json, cases/filter_rel.json e
cases/filter_bidir.json.

Nao e um motor DAX generico: cada funcao abaixo implementa literalmente a
semantica documentada (dax.guide / SQLBI / Microsoft Learn) so para o caso
especifico que ela testa. Fontes citadas em cada funcao e no campo "source"
dos casos correspondentes.

Rodar: /usr/bin/python3 tests/dax-corpus/oracles/filter.py
Sai com codigo != 0 se algum valor recalculado nao bater com o "expected"
gravado no JSON de casos (exceto os marcados como {"error": true}).
"""
from __future__ import annotations

import json
import os
import sys
from typing import Any

CORPUS_DIR = os.path.normpath(os.path.join(os.path.dirname(__file__), ".."))


def load_json(*parts: str) -> Any:
    with open(os.path.join(CORPUS_DIR, *parts), "r", encoding="utf-8") as f:
        return json.load(f)


def table_rows(model: dict, name: str) -> list[dict]:
    for t in model["tables"]:
        if t["name"] == name:
            cols = [c["name"] for c in t["columns"]]
            return [dict(zip(cols, row)) for row in t.get("rows", [])]
    raise KeyError(name)


# ---------------------------------------------------------------------------
# Modelo "vendas" (tests/dax-corpus/models/vendas.json)
# ---------------------------------------------------------------------------
vendas = load_json("models", "vendas.json")
f_vendas = table_rows(vendas, "f_Vendas")
d_produto = table_rows(vendas, "d_Produto")
d_cliente = table_rows(vendas, "d_Cliente")
prod_by_id = {p["ProdutoID"]: p for p in d_produto}
cli_by_id = {c["ClienteID"]: c for c in d_cliente}


def total_valor(rows: list[dict]) -> float | None:
    """SUM(f_Vendas[Valor]) sobre um subconjunto de linhas; BLANK se vazio."""
    return sum(r["Valor"] for r in rows) if rows else None


def filt(rows=None, prod_cat=None, prod_nome=None, cli_cidade=None, prod_ids=None, cli_ids=None):
    """Filtro explicito equivalente a um CALCULATE com os predicados dados
    combinados por AND (cada argumento, quando usado, restringe mais)."""
    rows = f_vendas if rows is None else rows
    out = []
    for r in rows:
        p = prod_by_id[r["ProdutoID"]]
        c = cli_by_id[r["ClienteID"]]
        if prod_cat is not None and p["Categoria"] not in prod_cat:
            continue
        if prod_nome is not None and p["Produto"] not in prod_nome:
            continue
        if cli_cidade is not None and c["Cidade"] not in cli_cidade:
            continue
        if prod_ids is not None and r["ProdutoID"] not in prod_ids:
            continue
        if cli_ids is not None and r["ClienteID"] not in cli_ids:
            continue
        out.append(r)
    return out


def per_client_total(cliente_id: int) -> float | None:
    return total_valor(filt(cli_ids={cliente_id}))


def per_product_total(produto_id: int) -> float | None:
    return total_valor(filt(prod_ids={produto_id}))


EXPECTED: dict[str, Any] = {}

# -- CALCULATE com multiplos filtros (AND) ---------------------------------
EXPECTED["filter.calculate.multi_and"] = total_valor(filt(prod_cat={"Informatica"}, cli_cidade={"Curitiba"}))
EXPECTED["filter.calculate.multi_and_narrow"] = total_valor(filt(prod_cat={"Papelaria"}, cli_cidade={"Londrina"}))

# -- Filtro em coluna diferente (intersecta) vs mesma coluna (substitui) ---
# dax.guide/calculate: um novo filtro de CALCULATE substitui qualquer filtro
# existente NA MESMA coluna, mas colunas diferentes da mesma tabela continuam
# se combinando por AND.
EXPECTED["filter.calculate.diffcolumn_intersect"] = total_valor(filt(prod_ids={1, 3}, prod_cat={"Papelaria"}))
EXPECTED["filter.calculate.samecolumn_replaces_outer"] = per_product_total(3)  # Mouse

# -- ALL(tabela) vs ALL(coluna) vs ALLEXCEPT --------------------------------
EXPECTED["filter.all.column_only_keeps_other_filters"] = total_valor(filt(prod_ids={1}))
EXPECTED["filter.all.whole_table_clears_all_columns"] = total_valor(f_vendas)
EXPECTED["filter.allexcept.keep_one_column"] = total_valor(filt(prod_cat={"Papelaria"}))
EXPECTED["filter.allexcept.keep_multiple_columns_noop"] = total_valor(filt(prod_ids={1}))

# -- KEEPFILTERS intersecta em vez de substituir ----------------------------
# dax.guide/keepfilters
outer_informatica = {r["VendaID"] for r in filt(prod_cat={"Informatica"})}
outer_papelaria = {r["VendaID"] for r in filt(prod_cat={"Papelaria"})}
outer_both = {r["VendaID"] for r in filt(prod_cat={"Papelaria", "Informatica"})}
inner_papelaria = {r["VendaID"] for r in filt(prod_cat={"Papelaria"})}


def total_by_ids(ids: set[int]) -> float | None:
    return total_valor([r for r in f_vendas if r["VendaID"] in ids])


EXPECTED["filter.keepfilters.intersect_empty_is_blank"] = total_by_ids(outer_informatica & inner_papelaria)
EXPECTED["filter.keepfilters.intersect_same_value"] = total_by_ids(outer_papelaria & inner_papelaria)
EXPECTED["filter.keepfilters.intersect_subset"] = total_by_ids(outer_both & inner_papelaria)

# -- FILTER com predicado de medida (transicao de contexto por linha) ------
clients_gt300 = {c["ClienteID"] for c in d_cliente if (per_client_total(c["ClienteID"]) or 0) > 300}
clients_gt400 = {c["ClienteID"] for c in d_cliente if (per_client_total(c["ClienteID"]) or 0) > 400}
EXPECTED["filter.filter.measure_predicate_gt300"] = total_valor(filt(cli_ids=clients_gt300))
EXPECTED["filter.filter.measure_predicate_gt400"] = total_valor(filt(cli_ids=clients_gt400))
EXPECTED["filter.filter.related_predicate_on_fact"] = total_valor(filt(prod_cat={"Informatica"}))

# -- Transicao de contexto em iteradores ------------------------------------
EXPECTED["filter.context.sumx_over_fact_rows_is_identity"] = sum(r["Valor"] for r in f_vendas)
EXPECTED["filter.context.sumx_dim_calculate_explicit_noop"] = sum(per_product_total(p["ProdutoID"]) for p in d_produto)
EXPECTED["filter.context.maxx_product_total"] = max(per_product_total(p["ProdutoID"]) for p in d_produto)

# -- RELATEDTABLE ------------------------------------------------------------
EXPECTED["filter.relatedtable.sumx_countrows_by_product"] = sum(
    sum(1 for r in f_vendas if r["ProdutoID"] == p["ProdutoID"]) for p in d_produto
)
EXPECTED["filter.relatedtable.sumx_countrows_by_client"] = sum(
    sum(1 for r in f_vendas if r["ClienteID"] == c["ClienteID"]) for c in d_cliente
)

# -- LOOKUPVALUE multi-chave --------------------------------------------------
def lookupvalue_matches(prod_id: int, cli_id: int) -> list[dict]:
    return [r for r in f_vendas if r["ProdutoID"] == prod_id and r["ClienteID"] == cli_id]


_m1 = lookupvalue_matches(2, 1)
assert len(_m1) == 1
EXPECTED["filter.lookupvalue.multikey_unique_match"] = _m1[0]["Valor"]

_m2 = lookupvalue_matches(1, 3)
assert len(_m2) == 0
EXPECTED["filter.lookupvalue.multikey_no_match_uses_alternate"] = 999

_m3 = lookupvalue_matches(3, 3)
assert len(_m3) == 2 and len({r["Valor"] for r in _m3}) > 1
EXPECTED["filter.lookupvalue.multikey_ambiguous_is_error"] = {"error": True}

# -- TREATAS ------------------------------------------------------------------
EXPECTED["filter.treatas.virtual_relationship_category"] = total_valor(filt(prod_cat={"Papelaria"}))

# -- ISFILTERED / ISCROSSFILTERED / HASONEVALUE ------------------------------
EXPECTED["filter.isfiltered.direct_column_true"] = True
EXPECTED["filter.isfiltered.indirect_column_false"] = False
EXPECTED["filter.iscrossfiltered.table_via_other_column_true"] = True
EXPECTED["filter.iscrossfiltered.no_bidirectional_propagation_false"] = False
EXPECTED["filter.hasonevalue.city_true"] = True
EXPECTED["filter.hasonevalue.no_filter_false"] = False

# -- ALLSELECTED --------------------------------------------------------------
# dax.guide/allselected: remove os filtros adicionados por CALCULATE/FILTER
# dentro da consulta, mas preserva o contexto de filtro "externo" (aqui,
# o array `filters` do caso, que representa o filtro de visual/slicer).
selected_total = per_product_total(1) + per_product_total(3)  # Caneta + Mouse selecionados no slicer
EXPECTED["filter.allselected.share_of_selected_caneta"] = per_product_total(1) / selected_total
EXPECTED["filter.allselected.share_of_selected_mouse"] = per_product_total(3) / selected_total

# ALLSELECTED: table arg, no-args, column not covered by the shadow, and a
# 3-item-slicer variant of the DIVIDE/share pattern above.
EXPECTED["filter.allselected.table_arg_restores_slicer"] = per_product_total(1) + per_product_total(3)
shadow_cal_cid = filt(prod_ids={1, 3}, cli_ids={1, 3})
EXPECTED["filter.allselected.noargs_restores_whole_shadow"] = total_valor(shadow_cal_cid)
shadow_cidade_only = filt(cli_ids={1, 3})
EXPECTED["filter.allselected.column_not_in_shadow_clears"] = total_valor(shadow_cidade_only)
den3 = per_product_total(1) + per_product_total(2) + per_product_total(3)
EXPECTED["filter.allselected.divide_share_three_selected"] = per_product_total(2) / den3

# -- Tuple filters (§2.3): FILTER/SUMMARIZE over 2+ columns, multi-column
# TREATAS, KEEPFILTERS intersecting tuples instead of replacing them. ---------
pair_11_22 = {r["VendaID"] for r in f_vendas if (r["ProdutoID"], r["ClienteID"]) in {(1, 1), (2, 2)}}
EXPECTED["filter.tuple.treatas_pair_restricts_combo"] = total_by_ids(pair_11_22)
EXPECTED["filter.tuple.filter_multi_col_predicate_on_physical"] = total_by_ids(pair_11_22)
EXPECTED["filter.tuple.summarize_bare_as_filter"] = total_by_ids(pair_11_22)
EXPECTED["filter.tuple.summarize_dim_pairs_as_filter"] = per_product_total(1) + per_product_total(3)
outer_pairs_345 = {r["VendaID"] for r in f_vendas if (r["ProdutoID"], r["ClienteID"]) in {(1, 1), (2, 2), (3, 3)}}
inner_pairs_11_41 = {r["VendaID"] for r in f_vendas if (r["ProdutoID"], r["ClienteID"]) in {(1, 1), (4, 1)}}
EXPECTED["filter.tuple.keepfilters_intersects_pairs"] = total_by_ids(outer_pairs_345 & inner_pairs_11_41)

# -- KEEPFILTERS wrapping a table expression (FILTER/CALCULATETABLE), not just
# a bare predicate: must still intersect, not replace. -----------------------
EXPECTED["filter.keepfilters.filter_table_arg_intersects"] = total_valor(filt(prod_cat={"Papelaria"}))
EXPECTED["filter.keepfilters.calculatetable_arg_intersects"] = total_valor(filt(prod_cat={"Informatica"}))

# -- VALUES vs DISTINCT (sem violacao de integridade referencial) ------------
EXPECTED["filter.values.countrows_table_no_dedup"] = len(d_produto)
EXPECTED["filter.distinct.countrows_column"] = len({p["Categoria"] for p in d_produto})
EXPECTED["filter.values.countrows_column_with_real_blank"] = len({r["Desconto"] for r in f_vendas})
EXPECTED["filter.distinct.countrows_column_with_real_blank"] = len({r["Desconto"] for r in f_vendas})

# -- Funcoes de tabela encapsuladas em COUNTROWS -----------------------------
EXPECTED["filter.table.summarize_countrows_categories"] = len({prod_by_id[r["ProdutoID"]]["Categoria"] for r in f_vendas})
EXPECTED["filter.table.addcolumns_countrows"] = len({p["Categoria"] for p in d_produto})
EXPECTED["filter.table.selectcolumns_countrows_no_dedup"] = len(d_produto)
EXPECTED["filter.table.union_countrows_no_dedup"] = len({p["Categoria"] for p in d_produto}) + len({c["Cidade"] for c in d_cliente})
EXPECTED["filter.table.intersect_countrows_no_overlap"] = len(
    {p["Categoria"] for p in d_produto} & {c["Cidade"] for c in d_cliente}
)
EXPECTED["filter.table.except_countrows_removes_papelaria"] = len({p["Categoria"] for p in d_produto} - {"Papelaria"})
EXPECTED["filter.table.crossjoin_countrows"] = len(d_produto) * len(d_cliente)
EXPECTED["filter.table.generateseries_countrows"] = len(list(range(1, 11)))
EXPECTED["filter.table.generateseries_countrows_step"] = len(list(range(0, 21, 5)))
EXPECTED["filter.table.calculatetable_countrows"] = sum(1 for p in d_produto if p["Categoria"] == "Papelaria")

# -- TOPN / MAXX(TOPN) --------------------------------------------------------
EXPECTED["filter.topn.no_ties_countrows"] = 2
# dax.guide/topn: empates na fronteira de N sao todos incluidos (resultado
# pode ter mais que N linhas). Papelaria(2) + Informatica(2) empatando no
# 3o lugar -> as 4 linhas aparecem.
EXPECTED["filter.topn.ties_expand_rows"] = len(d_produto)
top2_clients_by_total = sorted(d_cliente, key=lambda c: -(per_client_total(c["ClienteID"]) or 0))[:2]
EXPECTED["filter.topn.maxx_combo"] = max(per_client_total(c["ClienteID"]) for c in top2_clients_by_total)

# -- EARLIER ------------------------------------------------------------------
# dax.guide/earlier: dentro de FILTER aninhado em SUMX(d_Produto, ...), EARLIER
# aponta para a linha do SUMX (contexto externo).
EXPECTED["filter.earlier.rank_sum_via_filter"] = sum(
    sum(1 for q in d_produto if q["Preco"] <= p["Preco"]) for p in d_produto
)

# ---------------------------------------------------------------------------
# Modelo "filter_rel": USERELATIONSHIP + blank row (fonte:
# sqlbi.com/articles/blank-row-in-dax/)
# ---------------------------------------------------------------------------
filter_rel = load_json("models", "filter_rel.json")
f_pedido = table_rows(filter_rel, "f_Pedido")
d_produto_rel = table_rows(filter_rel, "d_Produto")


def ym(date_str: str) -> str:
    return date_str[:7]


def total_pedido(rows: list[dict]) -> float:
    return sum(r["Valor"] for r in rows)


EXPECTED["filter.rel.active_default_jan"] = total_pedido([r for r in f_pedido if ym(r["OrderDate"]) == "2024-01"])
EXPECTED["filter.rel.active_default_feb"] = total_pedido([r for r in f_pedido if ym(r["OrderDate"]) == "2024-02"])
EXPECTED["filter.rel.userelationship_shipdate_jan"] = total_pedido([r for r in f_pedido if ym(r["ShipDate"]) == "2024-01"])
EXPECTED["filter.rel.userelationship_shipdate_feb"] = total_pedido([r for r in f_pedido if ym(r["ShipDate"]) == "2024-02"])
EXPECTED["filter.rel.userelationship_no_filter_noop"] = total_pedido(f_pedido)
escritorio_ids = {p["ProdutoID"] for p in d_produto_rel if p["Categoria"] == "Escritorio"}
EXPECTED["filter.rel.category_filter_excludes_orphan"] = total_pedido([r for r in f_pedido if r["ProdutoID"] in escritorio_ids])
EXPECTED["filter.rel.unfiltered_includes_orphan"] = total_pedido(f_pedido)

# Blank row: f_Pedido tem uma linha com ProdutoID=9, que nao existe em
# d_Produto (so tem 1 e 2) -> relacionamento invalido -> VertiPaq adiciona
# uma blank row virtual em d_Produto (SQLBI, "Blank row in DAX").
EXPECTED["filter.rel.blankrow.countrows_table_excludes"] = len(d_produto_rel)  # COUNTROWS direto NAO inclui a blank row
EXPECTED["filter.rel.blankrow.values_table_includes"] = len(d_produto_rel) + 1  # VALUES(tabela) inclui
EXPECTED["filter.rel.blankrow.distinct_column_excludes"] = len({p["Produto"] for p in d_produto_rel})  # DISTINCT nunca inclui
EXPECTED["filter.rel.blankrow.values_column_includes"] = len({p["Produto"] for p in d_produto_rel}) + 1  # VALUES(coluna) inclui

# Blank member extras: direct filter excludes it; ADDCOLUMNS/SUMX over
# VALUES keep it; another column (Categoria) also surfaces it.
EXPECTED["filter.rel.blankrow.direct_filter_excludes_values_table"] = sum(1 for p in d_produto_rel if p["Categoria"] == "Escritorio")
EXPECTED["filter.rel.blankrow.addcolumns_over_values_includes"] = len(d_produto_rel) + 1
EXPECTED["filter.rel.blankrow.sumx_constant_counts_blank"] = len({p["Produto"] for p in d_produto_rel}) + 1
EXPECTED["filter.rel.blankrow.values_column_categoria_includes"] = len({p["Categoria"] for p in d_produto_rel}) + 1

# ---------------------------------------------------------------------------
# Modelo "filter_bidir": relacionamento bidirecional (crossFilter both) e
# CROSSFILTER() em tempo de consulta.
# ---------------------------------------------------------------------------
filter_bidir = load_json("models", "filter_bidir.json")
f_vendas_b = table_rows(filter_bidir, "f_Vendas")
d_cliente_b = table_rows(filter_bidir, "d_Cliente")
d_produto_b = table_rows(filter_bidir, "d_Produto")
prod_b_by_id = {p["ProdutoID"]: p for p in d_produto_b}

EXPECTED["filter.bidir.countrows_no_filter"] = len(d_cliente_b)


def clients_for_category(categoria: str) -> set[int]:
    prod_ids = {p["ProdutoID"] for p in d_produto_b if p["Categoria"] == categoria}
    return {r["ClienteID"] for r in f_vendas_b if r["ProdutoID"] in prod_ids}


EXPECTED["filter.bidir.countrows_filtered_papelaria"] = len(clients_for_category("Papelaria"))
EXPECTED["filter.bidir.countrows_filtered_informatica"] = len(clients_for_category("Informatica"))
papelaria_prod_ids = {p["ProdutoID"] for p in d_produto_b if p["Categoria"] == "Papelaria"}
EXPECTED["filter.bidir.sum_filtered_papelaria"] = sum(r["Valor"] for r in f_vendas_b if r["ProdutoID"] in papelaria_prod_ids)
# CROSSFILTER(...,NONE) corta a propagacao entre f_Vendas e d_Cliente por
# completo (inclusive a direcao normal dimensao->fato nesse par), entao o
# filtro de Categoria (que so afeta f_Vendas/d_Produto) deixa de restringir
# d_Cliente.
EXPECTED["filter.bidir.crossfilter_none_blocks_propagation"] = len(d_cliente_b)
# CROSSFILTER(...,BOTH) entre f_Vendas e d_Produto torna essa relacao
# bidirecional nesta consulta; filtrar Cliente=Ana restringe f_Vendas as suas
# vendas, que agora se propagam ate d_Produto.
ana_id = next(c["ClienteID"] for c in d_cliente_b if c["Cliente"] == "Ana")
ana_prod_ids = {r["ProdutoID"] for r in f_vendas_b if r["ClienteID"] == ana_id}
EXPECTED["filter.bidir.crossfilter_both_on_produto"] = len(ana_prod_ids)
EXPECTED["filter.bidir.hasonevalue_after_propagation"] = len(clients_for_category("Informatica")) == 1


def _tolerant_equal(expected: Any, got: Any, tol: float = 1e-9) -> bool:
    if isinstance(expected, dict) and expected.get("error"):
        return True  # casos de erro sao conferidos manualmente (sem valor numerico)
    if isinstance(expected, float) and isinstance(got, (int, float)):
        return abs(expected - got) <= tol * max(1, abs(expected))
    return expected == got


def main() -> int:
    case_files = ["filter.json", "filter_rel.json", "filter_bidir.json"]
    failures = []
    checked = 0
    for fname in case_files:
        data = load_json("cases", fname)
        for case in data["cases"]:
            cid = case["id"]
            if cid not in EXPECTED:
                failures.append(f"{cid}: sem calculo no oraculo")
                continue
            checked += 1
            want_recorded = case["expected"]
            want_oracle = EXPECTED[cid]
            if not _tolerant_equal(want_recorded, want_oracle):
                failures.append(f"{cid}: json={want_recorded!r} oraculo={want_oracle!r}")

    print(f"{checked} casos conferidos contra o oraculo.")
    if failures:
        print(f"{len(failures)} divergencia(s):")
        for f in failures:
            print(" -", f)
        return 1
    print("OK: todos os valores esperados batem com o oraculo independente.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
