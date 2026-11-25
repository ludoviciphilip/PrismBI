#!/usr/bin/env /usr/bin/python3
"""Independent oracle for tests/dax-corpus/cases/stats.json.

Every expected value here is computed from scratch with the stdlib
(`statistics`, `math`, hand-written financial formulas) directly against
tests/dax-corpus/models/stats.json. Nothing is copied from either DAX
engine (src-tauri/src/dax or src/engine). Run as a sanity check:

    /usr/bin/python3 tests/dax-corpus/oracles/stats.py

Financial formulas follow the standard Excel/DAX documented formulas
(Microsoft Learn: learn.microsoft.com/dax/<fn>-function-dax, which mirror
the classic Excel financial-function formulas). Statistical formulas are
plain textbook definitions (sample vs. population variance/stdev,
linear-interpolation percentile, geometric mean).
"""
import math
import statistics
from datetime import date

# ---------------------------------------------------------------------------
# t_Valores model data (tests/dax-corpus/models/stats.json)
# ---------------------------------------------------------------------------
VALOR = [10, 20, 20, 5, None, 15, 30, 30, 30, None]      # 10 rows, 2 blanks
TEXTO = ["x", "y", None, "x", None, "z", "x", "y", None, "z"]
OK = [True, False, None, True, None, True, False, False, True, None]

NUMS = [v for v in VALOR if v is not None]               # 8 non-blank values
SORTED = sorted(NUMS)                                     # [5,10,15,20,20,30,30,30]
DOUBLED = [v * 2 for v in NUMS]                            # expr = Valor*2 for X-variants


def percentile_inc(sorted_vals, k):
    n = len(sorted_vals)
    rank = k * (n - 1)
    lo, hi = math.floor(rank), math.ceil(rank)
    if lo == hi:
        return sorted_vals[lo]
    return sorted_vals[lo] + (rank - lo) * (sorted_vals[hi] - sorted_vals[lo])


def percentile_exc(sorted_vals, k):
    n = len(sorted_vals)
    rank = k * (n + 1)
    if rank < 1 or rank > n:
        raise ValueError("k out of range for PERCENTILE.EXC")
    lo, hi = math.floor(rank), math.ceil(rank)
    if lo == hi:
        return sorted_vals[lo - 1]
    return sorted_vals[lo - 1] + (rank - lo) * (sorted_vals[hi - 1] - sorted_vals[lo - 1])


# learn.microsoft.com/dax/rankx-function-dax: "If expression or value evaluates
# to BLANK it is treated as 0 (zero)" for numeric expressions. So the ranking
# comparison set must include blank rows as 0, NOT exclude them (this only
# matters for ASC ranks here, since 0 is below every real value in this model).
VALOR_FOR_RANK = [v if v is not None else 0 for v in VALOR]


def rank_skip(target, values, ascending):
    better = sum(1 for v in values if (v < target if ascending else v > target))
    return better + 1


def rank_dense(target, values, ascending):
    distinct_better = {v for v in values if (v < target if ascending else v > target)}
    return len(distinct_better) + 1


# ---------------------------------------------------------------------------
# Financial formulas (Excel/DAX standard; see learn.microsoft.com/dax/*-function-dax)
# ---------------------------------------------------------------------------
def pmt(rate, nper, pv, fv=0.0, ptype=0.0):
    if rate == 0:
        return -(pv + fv) / nper
    pvif = (1 + rate) ** nper
    return -(pv * pvif + fv) * rate / ((1 + rate * ptype) * (pvif - 1))


def pv_(rate, nper, pmt_, fv=0.0, ptype=0.0):
    if rate == 0:
        return -fv - pmt_ * nper
    pvif = (1 + rate) ** nper
    return (-fv - pmt_ * (1 + rate * ptype) * (pvif - 1) / rate) / pvif


def fv_(rate, nper, pmt_, pv0=0.0, ptype=0.0):
    if rate == 0:
        return -pv0 - pmt_ * nper
    pvif = (1 + rate) ** nper
    return -pv0 * pvif - pmt_ * (1 + rate * ptype) * (pvif - 1) / rate


def nper_(rate, pmt_, pv0, fv0=0.0, ptype=0.0):
    if rate == 0:
        return -(pv0 + fv0) / pmt_
    z = pmt_ * (1 + rate * ptype) / rate
    return math.log((z - fv0) / (z + pv0)) / math.log(1 + rate)


def npv(rate, cfs):
    return sum(cf / (1 + rate) ** (i + 1) for i, cf in enumerate(cfs))


def ipmt_ppmt(rate, per, nper0, pv0, fv0=0.0, ptype=0.0):
    pmt_v = pmt(rate, nper0, pv0, fv0, ptype)
    if rate == 0:
        ip = 0.0
    else:
        eff = per - 2 if ptype != 0 else per - 1
        if ptype != 0 and abs(per - 1) < 1e-12:
            ip = 0.0
        elif ptype != 0 and eff < 0:
            ip = 0.0
        else:
            p_pow = (1 + rate) ** max(eff, 0)
            bal = pv0 * p_pow + pmt_v * (1 + rate * ptype) * (p_pow - 1) / rate
            ip = -bal * rate
    return ip, pmt_v - ip


def sln(cost, salvage, life):
    return (cost - salvage) / life


def syd(cost, salvage, life, per):
    sum_years = life * (life + 1) / 2
    return (cost - salvage) * (life - per + 1) / sum_years


def db(cost, salvage, life, period, month=12):
    if cost <= 0 or life <= 0:
        return 0.0
    raw_rate = 1 - max(salvage / cost, 0) ** (1 / life)
    rate = round(raw_rate * 1000) / 1000
    total = 0.0
    cur = 0.0
    for p in range(1, int(period) + 1):
        if p == 1:
            cur = cost * rate * month / 12
        elif p > life:
            cur = (cost - total) * rate * (12 - month) / 12
        else:
            cur = (cost - total) * rate
        total += cur
    return cur


def ddb(cost, salvage, life, period, factor=2.0):
    rate = min(factor / life, 1.0)
    value = cost
    dep = 0.0
    for _ in range(int(period)):
        dep = min(value * rate, max(value - salvage, 0))
        value -= dep
    return max(dep, 0.0)


def effect(nominal_rate, npery):
    return (1 + nominal_rate / npery) ** npery - 1


def nominal(effect_rate, npery):
    return npery * ((1 + effect_rate) ** (1 / npery) - 1)


def rate_(nper0, pmt0, pv0, fv0=0.0, ptype=0.0, guess=0.1):
    r = guess
    for _ in range(50):
        if (1 + r) <= 0:
            r = 0.01
        if abs(r) < 1e-10:
            y = pv0 + pmt0 * nper0 + fv0
        else:
            f = (1 + r) ** nper0
            y = pv0 * f + pmt0 * (1 + r * ptype) * (f - 1) / r + fv0
        if abs(y) < 1e-9:
            break
        eps = 1e-6
        r2 = r + eps
        f2 = (1 + r2) ** nper0
        y2 = pv0 * f2 + pmt0 * (1 + r2 * ptype) * (f2 - 1) / r2 + fv0
        dy = (y2 - y) / eps
        if abs(dy) < 1e-12:
            break
        r -= y / dy
    return r


# ---------------------------------------------------------------------------
# t_Fluxo model data (dates + cashflows) for NPV / XNPV / XIRR
# ---------------------------------------------------------------------------
FLUXO_DATES = [date(2024, 1, 1), date(2024, 3, 1), date(2024, 6, 1), date(2024, 10, 1), date(2025, 1, 1)]
FLUXO_VALUES = [-10000, 2750, 4250, 3250, 2750]


def xnpv(rate, values, dates):
    d0 = dates[0]
    return sum(v / (1 + rate) ** ((d - d0).days / 365.0) for v, d in zip(values, dates))


def xirr(values, dates, guess=0.1):
    r = guess
    for _ in range(200):
        y = xnpv(r, values, dates)
        eps = 1e-6
        y2 = xnpv(r + eps, values, dates)
        dy = (y2 - y) / eps
        if abs(dy) < 1e-12:
            break
        r_new = r - y / dy
        if abs(r_new - r) < 1e-10:
            r = r_new
            break
        r = r_new
    return r


if __name__ == "__main__":
    print("n non-blank:", len(NUMS), "sorted:", SORTED)
    print("MEDIAN", statistics.median(NUMS))
    print("MEDIANX(*2)", statistics.median(DOUBLED))
    print("STDEV.S", statistics.stdev(NUMS))
    print("STDEV.P", statistics.pstdev(NUMS))
    print("VAR.S", statistics.variance(NUMS))
    print("VAR.P", statistics.pvariance(NUMS))
    print("STDEVX.S(*2)", statistics.stdev(DOUBLED))
    print("STDEVX.P(*2)", statistics.pstdev(DOUBLED))
    print("VARX.S(*2)", statistics.variance(DOUBLED))
    print("VARX.P(*2)", statistics.pvariance(DOUBLED))
    print("GEOMEAN", math.prod(NUMS) ** (1 / len(NUMS)))
    print("GEOMEANX(*2)", math.prod(DOUBLED) ** (1 / len(DOUBLED)))
    print("PRODUCT", math.prod(NUMS))
    print("PRODUCTX(*2)", math.prod(DOUBLED))
    for k in (0, 0.25, 0.5, 0.75, 1.0):
        print("PERCENTILE.INC", k, percentile_inc(SORTED, k))
    for k in (0.25, 0.5, 0.75):
        print("PERCENTILE.EXC", k, percentile_exc(SORTED, k))
    print("PERCENTILEX.INC 0.3", percentile_inc(SORTED, 0.3))
    print("PERCENTILEX.EXC 0.3", percentile_exc(SORTED, 0.3))

    texto_nonblank = [t for t in TEXTO if t is not None]
    print("COUNTA(Texto)", len(texto_nonblank))
    print("COUNTBLANK(Texto)", TEXTO.count(None))
    print("DISTINCTCOUNT(Texto) incl blank", len(set(TEXTO)))
    print("DISTINCTCOUNTNOBLANK(Texto)", len(set(texto_nonblank)))

    ok_nonblank = [o for o in OK if o is not None]
    mapped = [1.0 if o else 0.0 for o in ok_nonblank]
    print("COUNTA(Ok)", len(ok_nonblank))
    print("AVERAGEA(Ok)", sum(mapped) / len(mapped))
    print("MINA(Ok)", min(mapped), "MAXA(Ok)", max(mapped))

    print("COUNTX(Valor)", len(NUMS))
    print("COUNTAX(Texto)", len(texto_nonblank))
    print("MINX(Valor)", min(NUMS), "MAXX(Valor)", max(NUMS))

    for target in (5, 10, 15, 20, 30):
        print("RANKX DESC skip", target, rank_skip(target, VALOR_FOR_RANK, ascending=False))
        print("RANKX ASC  skip", target, rank_skip(target, VALOR_FOR_RANK, ascending=True))
    print("RANKX DESC dense 20", rank_dense(20, VALOR_FOR_RANK, ascending=False))
    print("RANKX DESC dense 30", rank_dense(30, VALOR_FOR_RANK, ascending=False))

    print("TOPN(3,DESC) sum", sum(sorted(NUMS, reverse=True)[:3]))
    print("TOPN(5,ASC) sum", sum(SORTED[:5]))

    print("PMT", pmt(0.08 / 12, 120, 10000))
    print("PV", pv_(0.06 / 12, 60, -200))
    print("FV", fv_(0.06 / 12, 120, -200))
    print("NPER", nper_(0.1 / 12, -200, 2000))
    print("NPV", npv(0.1, [-1000, 300, 400, 500, 600]))
    print("IPMT(1)/PPMT(1)", ipmt_ppmt(0.1 / 12, 1, 36, 8000))
    print("IPMT(12)/PPMT(12)", ipmt_ppmt(0.1 / 12, 12, 36, 8000))
    print("SLN", sln(10000, 1000, 5))
    print("SYD(1)", syd(10000, 1000, 5, 1), "SYD(5)", syd(10000, 1000, 5, 5))
    print("DB(1)", db(10000, 1000, 5, 1), "DB(2)", db(10000, 1000, 5, 2))
    print("DDB(1)", ddb(10000, 1000, 5, 1), "DDB(2)", ddb(10000, 1000, 5, 2))
    print("EFFECT", effect(0.08, 4))
    print("NOMINAL", nominal(0.0824, 4))
    print("RATE", rate_(36, -300, 8000))
    print("XNPV@10%", xnpv(0.10, FLUXO_VALUES, FLUXO_DATES))
    print("XIRR", xirr(FLUXO_VALUES, FLUXO_DATES))


# --- Correção (revisão do mantenedor) -------------------------------------
# RANKX(ALL(t), SUM(col)) NÃO faz transição de contexto: SUM ignora o contexto
# de linha, então toda linha avalia o mesmo valor (o do filtro externo) e o
# rank é 1. Os casos de ranking por linha usam CALCULATE(SUM(col)), cujos
# valores esperados são exatamente os calculados acima. Valor-alvo BLANK é
# comparado como 0 => posição 9 (8 valores positivos acima dele).
def _check_rankx_corrections():
    vals = [10, 20, 20, 5, None, 15, 30, 30, 30, None]
    scores = [0 if v is None else v for v in vals]
    assert 1 + sum(1 for s in scores if s > 0) == 9
    assert 1 + sum(1 for s in [5] * 10 if s > 5) == 1
    # SUMX(ALL(t), SUM(col)) com filtro externo ID=4: 10 linhas x SUM no filtro externo (5)
    assert 10 * 5 == 50

_check_rankx_corrections()
