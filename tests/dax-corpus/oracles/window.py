#!/usr/bin/env python3
"""Oraculo independente para a categoria 'window' do corpus de conformidade DAX.

Recalcula, com Python puro (stdlib) e logica explicita derivada de dax.guide e
da documentacao Microsoft Learn (nunca da saida de um dos motores), os valores
esperados de tests/dax-corpus/cases/window.json sobre o modelo
tests/dax-corpus/models/vendas.json.

Premissa sobre o "contexto atual" (ver src-tauri/tests/dax_corpus.rs):
o harness Rust roda `model.evaluate_dax(&c.expr, &ctx)` com `ctx` construido
SO a partir do array `filters` do caso (um filtro de visual/slicer), SEM
nenhuma linha de tabela (`row_stack`) empilhada e sem CALCULATE implicito
externo. Ou seja, cada caso roda num contexto de consulta "nivel de visual":
quando o caso filtra `d_Produto[Produto] = "Mouse"`, isso e exatamente o
contexto de uma celula de tabela/cartao com aquele slicer selecionado, e
funcoes de janela que precisam de uma "linha atual" (OFFSET/INDEX com
PARTITIONBY, RANK, ROWNUMBER) resolvem essa linha a partir do filtro de
contexto corrente (ver `resolve_single_col_value` /
`resolve_relation_current_values` em src-tauri/.../window.rs: usa o valor
unico visivel na coluna quando o filtro ja restringe para 1 linha). Casos
sem `filters` (ex.: CALENDARAUTO, SUMMARIZECOLUMNS sem filtro) rodam no
contexto de consulta vazio (equivalente a nenhum slicer selecionado / total
geral).

Rodar: /usr/bin/python3 tests/dax-corpus/oracles/window.py
Sai com codigo != 0 se algum "expected" gravado no JSON nao bater com o
oraculo (exceto o id documentado como bug conhecido do motor, ver BUGS).
"""
from __future__ import annotations

import json
import os
import sys
from datetime import date, timedelta
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


vendas = load_json("models", "vendas.json")
f_vendas = table_rows(vendas, "f_Vendas")
d_produto = table_rows(vendas, "d_Produto")
d_cliente = table_rows(vendas, "d_Cliente")
prod_by_id = {p["ProdutoID"]: p for p in d_produto}
cli_by_id = {c["ClienteID"]: c for c in d_cliente}


def total_vendas(rows: list[dict]) -> float | None:
    return sum(r["Valor"] for r in rows) if rows else None


def total_vendas_for_product(produto_id: int) -> float | None:
    return total_vendas([r for r in f_vendas if r["ProdutoID"] == produto_id])


def total_vendas_for_client(cliente_id: int) -> float | None:
    return total_vendas([r for r in f_vendas if r["ClienteID"] == cliente_id])


def total_vendas_for_client_category(cliente_id: int, categoria: str) -> float | None:
    rows = [
        r
        for r in f_vendas
        if r["ClienteID"] == cliente_id and prod_by_id[r["ProdutoID"]]["Categoria"] == categoria
    ]
    return total_vendas(rows)


EXPECTED: dict[str, Any] = {}
SOURCE: dict[str, str] = {}
NOTES: dict[str, str] = {}

# ---------------------------------------------------------------------------
# CALENDAR — dax.guide/calendar: intervalo continuo e INCLUSIVE de datas entre
# start_date e end_date (sem truncar hora).
# ---------------------------------------------------------------------------
EXPECTED["window.calendar.leap_quarter_days"] = (date(2024, 3, 31) - date(2024, 1, 1)).days + 1
SOURCE["window.calendar.leap_quarter_days"] = "https://dax.guide/calendar/"
NOTES["window.calendar.leap_quarter_days"] = (
    "2024 e bissexto: jan(31)+fev(29)+mar(31) = 91 dias; CALENDAR() e inclusivo nas duas pontas."
)

EXPECTED["window.calendar.feb_non_leap"] = (date(2025, 2, 28) - date(2025, 2, 1)).days + 1
SOURCE["window.calendar.feb_non_leap"] = "https://dax.guide/calendar/"
NOTES["window.calendar.feb_non_leap"] = "2025 nao e bissexto: fevereiro tem 28 dias (1 a 28, inclusive)."

# ---------------------------------------------------------------------------
# CALENDARAUTO — dax.guide/calendarauto e Microsoft Learn: varre TODAS as
# colunas de data do modelo EXCETO colunas calculadas e tabelas calculadas
# (so colunas "importadas" contam), encontra o min/max global e estende para
# o ano fiscal completo (termina no mes `fiscal_year_end_month`, default 12).
#
# Nesse corpus, d_Calendario e uma tabela de calendario GERADA (equivalente a
# uma tabela calculada via CALENDAR() no modelo real) e por isso deve ser
# ignorada por CALENDARAUTO; so f_Vendas[Data] (coluna de data "importada",
# com uma linha por venda) deve entrar na varredura.
# ---------------------------------------------------------------------------
date_only_cols = [r["Data"] for r in f_vendas]  # unica coluna de data "importada" do modelo
min_date = min(date.fromisoformat(d) for d in date_only_cols)
max_date = max(date.fromisoformat(d) for d in date_only_cols)


def calendarauto_range(fiscal_year_end_month: int) -> tuple[date, date]:
    """Replica a formula de dax.guide/calendarauto (full fiscal years)."""
    fm = fiscal_year_end_month
    if fm == 12:
        return date(min_date.year, 1, 1), date(max_date.year, 12, 31)
    start_year = min_date.year - 1 if min_date.month <= fm else min_date.year
    start = date(start_year, fm + 1, 1)
    end_year = max_date.year if max_date.month <= fm else max_date.year + 1
    end_month_first = date(end_year, fm + 1, 1) if fm < 12 else date(end_year + 1, 1, 1)
    end = end_month_first - timedelta(days=1)
    return start, end


_start_dec, _end_dec = calendarauto_range(12)
EXPECTED["window.calendarauto.default_dec"] = (_end_dec - _start_dec).days + 1
SOURCE["window.calendarauto.default_dec"] = "https://dax.guide/calendarauto/"
NOTES["window.calendarauto.default_dec"] = (
    f"Unica coluna de data importada e f_Vendas[Data] ({min_date}..{max_date}); "
    f"fm=12 default -> anos civis completos {_start_dec}..{_end_dec} "
    "(2024 bissexto=366 + 2025=365 = 731). Coincide com o range do d_Calendario "
    "gerado so porque os dois cobrem exatamente 2024-2025; ver BUGS para o caso "
    "fiscal_july_days, onde a diferenca aparece."
)

_start_jul, _end_jul = calendarauto_range(7)
_calendarauto_fiscal_july_oracle = (_end_jul - _start_jul).days + 1
SOURCE["window.calendarauto.fiscal_july_days"] = "https://dax.guide/calendarauto/"
NOTES["window.calendarauto.fiscal_july_days"] = (
    f"BUG DO MOTOR (ver BUGS abaixo): com fm=7 e so f_Vendas[Data] "
    f"({min_date}..{max_date}), o ano fiscal completo e {_start_jul}..{_end_jul} = "
    f"{_calendarauto_fiscal_july_oracle} dias. O valor gravado antes desta auditoria "
    "era 1096, que so bate se CALENDARAUTO tambem varrer d_Calendario (tabela de "
    "calendario GERADA, equivalente a uma tabela calculada), o que a doc "
    "proibe explicitamente (\"CALENDARAUTO ignores calculated tables and "
    "calculated columns\"). Corrigido para o valor do oraculo."
)
EXPECTED["window.calendarauto.fiscal_july_days"] = _calendarauto_fiscal_july_oracle

EXPECTED["window.calendarauto.fiscal_july_start_year"] = _start_jul.year
SOURCE["window.calendarauto.fiscal_july_start_year"] = "https://dax.guide/calendarauto/"
NOTES["window.calendarauto.fiscal_july_start_year"] = (
    f"Inicio do ano fiscal (fm=7) e {_start_jul}; YEAR(...) = {_start_jul.year}. "
    "Esse caso bate com o oraculo independentemente do bug acima, porque o mes "
    "minimo (janeiro) e o mesmo em f_Vendas e em d_Calendario."
)

# ---------------------------------------------------------------------------
# SUMMARIZECOLUMNS — Microsoft Learn/dax.guide: uma linha por combinacao
# distinta das colunas de agrupamento que aparece na(s) tabela(s) de origem
# apos o(s) filtro(s) de tabela opcionais; linhas em que TODAS as expressoes
# de extensao avaliam para BLANK sao omitidas.
# ---------------------------------------------------------------------------
categoria_totals = {
    cat: total_vendas([r for r in f_vendas if prod_by_id[r["ProdutoID"]]["Categoria"] == cat])
    for cat in {p["Categoria"] for p in d_produto}
}
EXPECTED["window.summarizecolumns.categories_count"] = len(categoria_totals)
SOURCE["window.summarizecolumns.categories_count"] = "https://dax.guide/summarizecolumns/"
NOTES["window.summarizecolumns.categories_count"] = (
    "2 categorias distintas em d_Produto (Papelaria, Informatica), ambas com "
    "Total Vendas != BLANK -> nenhuma linha omitida."
)

EXPECTED["window.summarizecolumns.sum_totals"] = sum(categoria_totals.values())
SOURCE["window.summarizecolumns.sum_totals"] = "https://dax.guide/summarizecolumns/"
NOTES["window.summarizecolumns.sum_totals"] = "Soma das 2 categorias = SUM(f_Vendas[Valor]) geral = 1135."

cliente_categoria_totals = {
    (c["ClienteID"], cat): total_vendas_for_client_category(c["ClienteID"], cat)
    for c in d_cliente
    for cat in {p["Categoria"] for p in d_produto}
}
_non_blank_combos = {k: v for k, v in cliente_categoria_totals.items() if v is not None}
EXPECTED["window.summarizecolumns.omits_all_blank_combos"] = len(_non_blank_combos)
SOURCE["window.summarizecolumns.omits_all_blank_combos"] = "https://dax.guide/summarizecolumns/"
NOTES["window.summarizecolumns.omits_all_blank_combos"] = (
    "3 clientes x 2 categorias = 6 combinacoes possiveis; Carla nao comprou "
    "nada de Papelaria -> BLANK -> omitida; restam 5."
)

_papelaria_by_client = {
    c["ClienteID"]: total_vendas_for_client_category(c["ClienteID"], "Papelaria") for c in d_cliente
}
EXPECTED["window.summarizecolumns.with_filter_table"] = sum(
    1 for v in _papelaria_by_client.values() if v is not None
)
SOURCE["window.summarizecolumns.with_filter_table"] = "https://dax.guide/summarizecolumns/"
NOTES["window.summarizecolumns.with_filter_table"] = (
    "FILTER(ALL(d_Produto), Categoria=\"Papelaria\") restringe o filtro de tabela; "
    "agrupando por Cliente, so Ana (81) e Bruno (134) tem total != BLANK; Carla fica BLANK e e omitida -> 2."
)

# ---------------------------------------------------------------------------
# OFFSET / INDEX / WINDOW — dax.guide: ordenam a `relation` (aqui ALL(...),
# que ignora filtros existentes nas colunas listadas) pelo ORDERBY, localizam
# a "linha atual" (pinada pelo filtro de visual do caso) dentro da partition
# ativa (PARTITIONBY), e deslocam por `delta` (OFFSET) ou indexam por posicao
# absoluta 1-based (INDEX) ou por intervalo (WINDOW). Fora dos limites da
# partition -> BLANK.
# ---------------------------------------------------------------------------
produtos_by_price = sorted(d_produto, key=lambda p: p["Preco"])
# Caneta(2.5) < Caderno(12) < Mouse(80) < Teclado(150)

_mouse_pos = next(i for i, p in enumerate(produtos_by_price) if p["Produto"] == "Mouse")
EXPECTED["window.offset.previous_product_by_price"] = total_vendas_for_product(
    produtos_by_price[_mouse_pos - 1]["ProdutoID"]
)
SOURCE["window.offset.previous_product_by_price"] = "https://dax.guide/offset/"
NOTES["window.offset.previous_product_by_price"] = (
    "Produtos por Preco ASC: Caneta,Caderno,Mouse,Teclado. OFFSET(-1) na linha do "
    "Mouse -> Caderno (Total Vendas=120)."
)

EXPECTED["window.offset.first_row_previous_is_blank"] = None
SOURCE["window.offset.first_row_previous_is_blank"] = "https://dax.guide/offset/"
NOTES["window.offset.first_row_previous_is_blank"] = (
    "Caneta e a 1a linha por Preco ASC; OFFSET(-1) fica fora da partition -> BLANK."
)

_informatica_by_price = sorted(
    (p for p in d_produto if p["Categoria"] == "Informatica"), key=lambda p: p["Preco"]
)
_mouse_pos_part = next(i for i, p in enumerate(_informatica_by_price) if p["Produto"] == "Mouse")
EXPECTED["window.offset.partitioned_next"] = total_vendas_for_product(
    _informatica_by_price[_mouse_pos_part + 1]["ProdutoID"]
)
SOURCE["window.offset.partitioned_next"] = "https://dax.guide/offset/"
NOTES["window.offset.partitioned_next"] = (
    "PARTITIONBY(Categoria) isola Informatica={Mouse(80),Teclado(150)}; OFFSET(1) a "
    "partir do Mouse -> Teclado (Total Vendas=600)."
)

EXPECTED["window.offset.partitioned_boundary_is_blank"] = None
SOURCE["window.offset.partitioned_boundary_is_blank"] = "https://dax.guide/offset/"
NOTES["window.offset.partitioned_boundary_is_blank"] = (
    "Mouse e o 1o da partition Informatica por Preco ASC; OFFSET(-1) fica fora -> BLANK."
)

_by_price_desc = sorted(d_produto, key=lambda p: -p["Preco"])
EXPECTED["window.index.top_price_desc"] = total_vendas_for_product(_by_price_desc[0]["ProdutoID"])
SOURCE["window.index.top_price_desc"] = "https://dax.guide/index-dax/"
NOTES["window.index.top_price_desc"] = (
    "Sem filtro de visual (contexto de consulta vazio): INDEX(1) por Preco DESC -> "
    "Teclado (o mais caro), Total Vendas=600."
)

_papelaria_by_price = sorted((p for p in d_produto if p["Categoria"] == "Papelaria"), key=lambda p: p["Preco"])
EXPECTED["window.index.last_in_partition"] = total_vendas_for_product(_papelaria_by_price[-1]["ProdutoID"])
SOURCE["window.index.last_in_partition"] = "https://dax.guide/index-dax/"
NOTES["window.index.last_in_partition"] = (
    "Filtro de visual Categoria=Papelaria pina a partition; INDEX(-1) por Preco ASC "
    "-> ultimo da partition = Caderno (Total Vendas=120)."
)

_running_ids = [p["ProdutoID"] for p in produtos_by_price[: _mouse_pos + 1]]
EXPECTED["window.window.running_total"] = sum(total_vendas_for_product(pid) for pid in _running_ids)
SOURCE["window.window.running_total"] = "https://dax.guide/window/"
NOTES["window.window.running_total"] = (
    "WINDOW(1,ABS,0,REL) = do 1o produto por Preco ASC at\u00e9 a linha atual (Mouse) -> "
    "Caneta(95)+Caderno(120)+Mouse(320) = 535."
)

_moving_ids = [p["ProdutoID"] for p in produtos_by_price[_mouse_pos - 1 : _mouse_pos + 1]]
EXPECTED["window.window.moving_two_items"] = sum(total_vendas_for_product(pid) for pid in _moving_ids)
SOURCE["window.window.moving_two_items"] = "https://dax.guide/window/"
NOTES["window.window.moving_two_items"] = (
    "WINDOW(-1,REL,0,REL) = linha anterior at\u00e9 a atual (Mouse) -> "
    "Caderno(120)+Mouse(320) = 440."
)

# ---------------------------------------------------------------------------
# RANK / ROWNUMBER — dax.guide/rank-dax e dax.guide/rownumber: ordenam por
# ORDERBY; em empate, SKIP salta os numeros, DENSE nao. ROWNUMBER exige ordem
# total: quando ORDERBY nao desempata sozinho, o motor completa com as demais
# colunas da `relation` (aqui, Cliente, na ordem declarada em ALL(...)) para
# dar um resultado deterministico — sem isso, a doc diz que a funcao retorna
# erro por falta de ordem total.
# ---------------------------------------------------------------------------
_clientes_by_city = sorted(d_cliente, key=lambda c: (c["Cidade"], c["Cliente"]))
# Curitiba < Londrina; empate em Curitiba (Ana, Carla) desempatado por Cliente
# (coluna extra de ALL(d_Cliente[Cliente], d_Cliente[Cidade])): Ana < Carla.
_order_names = [c["Cliente"] for c in _clientes_by_city]
assert _order_names == ["Ana", "Carla", "Bruno"], _order_names

_bruno_pos = _order_names.index("Bruno")  # 2 (0-based) -> rank SKIP = 3, DENSE = 2
EXPECTED["window.rank.skip_with_ties"] = _bruno_pos + 1
SOURCE["window.rank.skip_with_ties"] = "https://dax.guide/rank-dax/"
NOTES["window.rank.skip_with_ties"] = (
    "Ordem por Cidade ASC com desempate por Cliente: Ana(Curitiba), Carla(Curitiba), "
    "Bruno(Londrina). Ana e Carla empatam em Cidade -> RANK(SKIP) de Bruno pula os "
    "2 primeiros postos = 3."
)
EXPECTED["window.rank.dense_with_ties"] = 2
SOURCE["window.rank.dense_with_ties"] = "https://dax.guide/rank-dax/"
NOTES["window.rank.dense_with_ties"] = (
    "RANK(DENSE) nao pula posicoes por empate: Ana/Carla=1, Bruno=2 (2 valores "
    "distintos de Cidade antes/incluindo o de Bruno)."
)

_carla_pos = _order_names.index("Carla")
EXPECTED["window.rownumber.tie_broken_by_client_name"] = _carla_pos + 1
SOURCE["window.rownumber.tie_broken_by_client_name"] = "https://dax.guide/rownumber/"
NOTES["window.rownumber.tie_broken_by_client_name"] = (
    "ROWNUMBER exige ordem total; como Ana e Carla empatam em Cidade, o motor "
    "desempata pela outra coluna da relation (Cliente, ASC): Ana(1), Carla(2), Bruno(3)."
)

# ---------------------------------------------------------------------------
# Bugs conhecidos do motor Rust encontrados nesta auditoria (NAO corrigidos
# aqui; registrados no relatorio da tarefa).
# ---------------------------------------------------------------------------
BUGS = [
    (
        "window.calendarauto.fiscal_july_days",
        "eval_calendarauto (src-tauri/src/dax/evaluator/table.rs) varre "
        "`model.tables.values()` sem excluir tabelas/colunas calculadas, "
        "violando a regra documentada 'CALENDARAUTO ignores calculated tables "
        "and calculated columns' (dax.guide/calendarauto). DaxTableData/"
        "DaxTableInput nao carregam nenhuma flag is_calculated hoje (so "
        "pbi::Table/pbi::Column tem isso, no loader TMDL/PBIP) -- plumbing "
        "isso pela DaxModel e um fix maior, fora do escopo de window.rs; "
        "so registrado, nao corrigido nesta rodada.",
    )
]


def _tolerant_equal(expected: Any, got: Any, tol: float = 1e-9) -> bool:
    if isinstance(expected, dict) and expected.get("error"):
        return True
    if isinstance(expected, (int, float)) and isinstance(got, (int, float)):
        return abs(expected - got) <= tol * max(1, abs(expected))
    return expected == got


def main() -> int:
    data = load_json("cases", "window.json")
    failures = []
    checked = 0
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
    if BUGS:
        print(f"\n{len(BUGS)} bug(s) de motor registrado(s) (nao corrigidos aqui):")
        for cid, desc in BUGS:
            print(f" - {cid}: {desc}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
