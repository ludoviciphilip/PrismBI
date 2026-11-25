#!/usr/bin/env /usr/bin/python3
"""
Independent Python oracle for the 'time' (time intelligence) corpus category.

Loads tests/dax-corpus/models/vendas.json, regenerates the same daily
calendar the TS/Rust harnesses build (see README.md), and implements DAX
time-intelligence semantics *from scratch*, following the documented
behaviour of each function (dax.guide / Microsoft Learn), never by reading
either engine's source or output. See docstrings per function for the
documented reference behaviour used.

Run standalone to sanity-check the 7 "core.ti.*" seed cases already present
in cases/core.json (they must match independently), then to (re)compute every
expected value used in cases/time.json and rewrite that file.

Usage:
    /usr/bin/python3 tests/dax-corpus/oracles/time.py --check-seed
    /usr/bin/python3 tests/dax-corpus/oracles/time.py --write
"""
from __future__ import annotations

import calendar as _cal
import json
import sys
from dataclasses import dataclass
from datetime import date, timedelta
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MODEL_PATH = ROOT / "models" / "vendas.json"
CORE_CASES_PATH = ROOT / "cases" / "core.json"
TIME_CASES_PATH = ROOT / "cases" / "time.json"

# ---------------------------------------------------------------------------
# Data loading — regenerate the exact same daily calendar + fact rows the
# TS (dax_corpus.test.ts) and Rust (dax_corpus.rs) harnesses build.
# ---------------------------------------------------------------------------


def parse_iso(s: str) -> date:
    y, m, d = s.split("-")
    return date(int(y), int(m), int(d))


def generate_calendar_rows(start: str, end: str):
    d = parse_iso(start)
    last = parse_iso(end)
    rows = []
    while d <= last:
        rows.append(
            {
                "Data": d,
                "Ano": d.year,
                "MesNum": d.month,
                "Trimestre": (d.month - 1) // 3 + 1,
                "AnoMes": f"{d.year:04d}-{d.month:02d}",
            }
        )
        d += timedelta(days=1)
    return rows


@dataclass
class Model:
    calendar: list
    calendar_start: date
    calendar_end: date
    fact: list  # list of dicts with Data (date), Valor (float)


def load_model(path: Path = MODEL_PATH) -> Model:
    raw = json.loads(path.read_text())
    cal_table = next(t for t in raw["tables"] if t["name"] == "d_Calendario")
    fact_table = next(t for t in raw["tables"] if t["name"] == "f_Vendas")
    cal_rows = generate_calendar_rows(cal_table["calendar"]["start"], cal_table["calendar"]["end"])
    col_names = [c["name"] for c in fact_table["columns"]]
    fact = []
    for r in fact_table["rows"]:
        row = dict(zip(col_names, r))
        row["Data"] = parse_iso(row["Data"])
        fact.append(row)
    return Model(
        calendar=cal_rows,
        calendar_start=cal_rows[0]["Data"],
        calendar_end=cal_rows[-1]["Data"],
        fact=fact,
    )


MODEL = load_model()

# ---------------------------------------------------------------------------
# Measure: [Total Vendas] = SUM(f_Vendas[Valor]) for rows whose Data is in
# the given set of dates (BLANK/None if the set is empty, matching DAX's
# "SUM over an empty filtered table is BLANK").
# ---------------------------------------------------------------------------


def total_vendas(dates):
    if dates is None:
        rows = MODEL.fact
    else:
        dateset = set(dates)
        if not dateset:
            return None
        rows = [r for r in MODEL.fact if r["Data"] in dateset]
    if not rows:
        return None
    return sum(r["Valor"] for r in rows if r["Valor"] is not None) or (
        None if not any(r["Valor"] is not None for r in rows) else 0
    )


# ---------------------------------------------------------------------------
# Calendar-column filters -> base "current context" set of dates on
# d_Calendario[Data]. Mirrors how a Power BI slicer/visual filter narrows the
# calendar table before CALCULATE / time-intelligence functions run.
# ---------------------------------------------------------------------------


def dates_for_filters(filters):
    """filters: list of {table, column, values} — only d_Calendario columns
    matter for this oracle. Returns the sorted list of Data values (date
    objects) whose row matches every filter (AND across columns, OR within
    a column's values list) -- same semantics as a Power BI filter context.
    """
    rows = MODEL.calendar
    for f in filters or []:
        if f["table"] != "d_Calendario":
            continue
        col = f["column"]
        allowed = set(f["values"])
        if col == "Data":
            allowed = {parse_iso(v) if isinstance(v, str) else v for v in f["values"]}
        rows = [r for r in rows if r[col] in allowed]
    return sorted(r["Data"] for r in rows)


# ---------------------------------------------------------------------------
# Calendar arithmetic helpers (month/quarter/year shifting with "clamp to
# month end" semantics, exactly as DATEADD/EDATE behave: shifting Jan 31 by
# -1 MONTH lands on Feb 28/29, never Mar 3).
# ---------------------------------------------------------------------------


def month_add(d: date, n: int) -> date:
    total = d.year * 12 + (d.month - 1) + n
    y, m = divmod(total, 12)
    m += 1
    last_day = _cal.monthrange(y, m)[1]
    return date(y, m, min(d.day, last_day))


def quarter_add(d: date, n: int) -> date:
    return month_add(d, 3 * n)


def year_add(d: date, n: int) -> date:
    return month_add(d, 12 * n)


def shift(d: date, n: int, interval: str) -> date:
    interval = interval.upper()
    if interval == "DAY":
        return d + timedelta(days=n)
    if interval == "MONTH":
        return month_add(d, n)
    if interval == "QUARTER":
        return quarter_add(d, n)
    if interval == "YEAR":
        return year_add(d, n)
    raise ValueError(f"unknown interval {interval}")


def start_of_month(d: date) -> date:
    return date(d.year, d.month, 1)


def end_of_month(d: date) -> date:
    last_day = _cal.monthrange(d.year, d.month)[1]
    return date(d.year, d.month, last_day)


def start_of_quarter(d: date) -> date:
    q = (d.month - 1) // 3
    return date(d.year, q * 3 + 1, 1)


def end_of_quarter(d: date) -> date:
    q = (d.month - 1) // 3
    last_month = q * 3 + 3
    last_day = _cal.monthrange(d.year, last_month)[1]
    return date(d.year, last_month, last_day)


def start_of_year(d: date) -> date:
    return date(d.year, 1, 1)


def end_of_year(d: date) -> date:
    return date(d.year, 12, 31)


def dates_in_range(a: date, b: date):
    """All calendar dates between a and b inclusive, clipped to the actual
    generated calendar range (dates outside -> simply absent, which is what
    produces BLANK results for e.g. SAMEPERIODLASTYEAR in the first year)."""
    if a > b:
        return []
    a = max(a, MODEL.calendar_start)
    b = min(b, MODEL.calendar_end)
    if a > b:
        return []
    out = []
    d = a
    while d <= b:
        out.append(d)
        d += timedelta(days=1)
    return out


# ---------------------------------------------------------------------------
# Time-intelligence table functions. Each takes/returns a *set/list of dates*
# representing "the new filter on d_Calendario[Data]" that CALCULATE would
# apply. `current` is the base context (see dates_for_filters).
# ---------------------------------------------------------------------------


def dateadd(current, n: int, interval: str):
    """DAX DATEADD: shift the min/max of the current date range by n
    intervals (dax.guide: 'Moves the given set of dates by a specified
    interval', operating on the contiguous span of the current selection)."""
    if not current:
        return []
    lo, hi = min(current), max(current)
    return dates_in_range(shift(lo, n, interval), shift(hi, n, interval))


def parallelperiod(current, n: int, interval: str):
    """DAX PARALLELPERIOD: like DATEADD but snapped to *full* periods of the
    given granularity (dax.guide: 'a period parallel to the dates ... with
    the dates shifted'; internally PREVIOUSMONTH = PARALLELPERIOD(FIRSTDATE(
    dates),-1,MONTH), i.e. it expands to whole months/quarters/years)."""
    if not current:
        return []
    lo, hi = min(current), max(current)
    shifted_lo = shift(lo, n, interval)
    shifted_hi = shift(hi, n, interval)
    interval = interval.upper()
    if interval == "DAY":
        return dates_in_range(shifted_lo, shifted_hi)
    if interval == "MONTH":
        return dates_in_range(start_of_month(shifted_lo), end_of_month(shifted_hi))
    if interval == "QUARTER":
        return dates_in_range(start_of_quarter(shifted_lo), end_of_quarter(shifted_hi))
    if interval == "YEAR":
        return dates_in_range(start_of_year(shifted_lo), end_of_year(shifted_hi))
    raise ValueError(interval)


def sameperiodlastyear(current):
    return dateadd(current, -1, "YEAR")


def previousday(current):
    if not current:
        return []
    return dates_in_range(min(current) - timedelta(days=1), min(current) - timedelta(days=1))


def nextday(current):
    if not current:
        return []
    return dates_in_range(max(current) + timedelta(days=1), max(current) + timedelta(days=1))


def previousperiod(current, interval: str):
    """PREVIOUS{MONTH,QUARTER,YEAR}: full period before the period containing
    the *first* date in current context (dax.guide: 'based on first date in
    the dates argument'; PREVIOUSMONTH = PARALLELPERIOD(FIRSTDATE(d),-1,MONTH))."""
    if not current:
        return []
    first = min(current)
    return parallelperiod([first], -1, interval)


def nextperiod(current, interval: str):
    """NEXT{MONTH,QUARTER,YEAR}: full period after the period containing the
    *last* date in current context (MS Learn: 'based on the last date')."""
    if not current:
        return []
    last = max(current)
    return parallelperiod([last], 1, interval)


def datesytd(current, year_end: str | None = None):
    """DATESYTD: dates from the start of the fiscal year containing MAX(
    current) through MAX(current). Default fiscal year end = Dec 31. With a
    <year_end_date> literal "MM-DD" (TOTALYTD/DATESYTD 2nd arg), the fiscal
    year boundary moves to that month/day (dax patterns: fiscal YTD)."""
    if not current:
        return []
    last = max(current)
    if not year_end:
        return dates_in_range(start_of_year(last), last)
    mm, dd = (int(x) for x in year_end.split("-"))
    fy_end_this_year = date(last.year, mm, dd)
    if last <= fy_end_this_year:
        start = date(last.year - 1, mm, dd) + timedelta(days=1)
    else:
        start = fy_end_this_year + timedelta(days=1)
    return dates_in_range(start, last)


def datesqtd(current):
    if not current:
        return []
    last = max(current)
    return dates_in_range(start_of_quarter(last), last)


def datesmtd(current):
    if not current:
        return []
    last = max(current)
    return dates_in_range(start_of_month(last), last)


def datesbetween(start_date: date, end_date: date):
    """DATESBETWEEN: absolute literal range, independent of current context."""
    return dates_in_range(start_date, end_date)


def datesinperiod(start_date: date, n: int, interval: str):
    """DATESINPERIOD(dates, start_date, n, interval): per Microsoft Learn
    worked example (filtered to June 2020, MAX=2020-06-30, -1 YEAR => range
    2019-07-01..2020-06-30): for n<0 the range is (shifted, start_date], for
    n>=0 it is [start_date, shifted)."""
    shifted = shift(start_date, n, interval)
    if n < 0:
        return dates_in_range(shifted + timedelta(days=1), start_date)
    return dates_in_range(start_date, shifted - timedelta(days=1))


def startofmonth(current):
    return [start_of_month(max(current))] if current else []


def endofmonth(current):
    return [end_of_month(max(current))] if current else []


def startofquarter(current):
    return [start_of_quarter(max(current))] if current else []


def endofquarter(current):
    return [end_of_quarter(max(current))] if current else []


def startofyear(current):
    return [start_of_year(max(current))] if current else []


def endofyear(current):
    return [end_of_year(max(current))] if current else []


def firstdate(current):
    return [min(current)] if current else []


def lastdate(current):
    return [max(current)] if current else []


def openingbalance(current, interval: str):
    """OPENINGBALANCE{MONTH,QUARTER,YEAR}(expr,dates) ==
    CALCULATE(expr, PREVIOUSDAY(STARTOF<period>(dates))) i.e. evaluated at
    the single date = day before the first day of the current period
    (dax.guide / stackoverflow: 'the day before that period starts',
    equivalently the last date of the previous period)."""
    if not current:
        return []
    last = max(current)
    start_fn = {"MONTH": start_of_month, "QUARTER": start_of_quarter, "YEAR": start_of_year}[interval]
    return [start_fn(last) - timedelta(days=1)]


def closingbalance(current, interval: str):
    """CLOSINGBALANCE{MONTH,QUARTER,YEAR}(expr,dates) == CALCULATE(expr,
    ENDOF<period>(dates)): evaluated at the last date of the current period."""
    if not current:
        return []
    last = max(current)
    end_fn = {"MONTH": end_of_month, "QUARTER": end_of_quarter, "YEAR": end_of_year}[interval]
    return [end_fn(last)]


def firstnonblank(current):
    """FIRSTNONBLANK(d_Calendario[Data], [Total Vendas]): earliest date
    (among VALUES(Data) in current context) for which the measure, evaluated
    with context transition to that single date, is not BLANK."""
    for d in sorted(current):
        if total_vendas([d]) is not None:
            return d
    return None


def lastnonblank(current):
    for d in sorted(current, reverse=True):
        if total_vendas([d]) is not None:
            return d
    return None


# ---------------------------------------------------------------------------
# Self-check against the 7 pre-existing "core.ti.*" seed cases in core.json.
# These are the only independently-trusted numbers we didn't derive
# ourselves, so we cross-validate our semantics against them.
# ---------------------------------------------------------------------------


def check_seed():
    core = json.loads(CORE_CASES_PATH.read_text())
    seed_ids = [c["id"] for c in core["cases"] if c["id"].startswith("core.ti.")]
    assert len(seed_ids) == 7, f"expected 7 seed cases, found {len(seed_ids)}: {seed_ids}"
    ok = True
    for c in core["cases"]:
        if not c["id"].startswith("core.ti."):
            continue
        current = dates_for_filters(c.get("filters"))
        cid = c["id"]
        if cid == "core.ti.totalytd":
            got = total_vendas(datesytd(current))
        elif cid == "core.ti.datesytd":
            got = total_vendas(datesytd(current))
        elif cid == "core.ti.sameperiodlastyear_year":
            got = total_vendas(sameperiodlastyear(current))
        elif cid == "core.ti.sameperiodlastyear_month":
            got = total_vendas(sameperiodlastyear(current))
        elif cid == "core.ti.previousmonth":
            got = total_vendas(previousperiod(current, "MONTH"))
        elif cid == "core.ti.dateadd_month":
            got = total_vendas(dateadd(current, -1, "MONTH"))
        elif cid == "core.ti.parallelperiod_year":
            got = total_vendas(parallelperiod(current, -1, "YEAR"))
        else:
            continue
        want = c["expected"]
        status = "OK" if got == want else "MISMATCH"
        if got != want:
            ok = False
        print(f"{status:9s} {cid:40s} want={want!r} got={got!r}")
    if not ok:
        raise SystemExit("seed self-check failed")
    print("all 7 seed cases match the independent oracle")


if __name__ == "__main__":
    if "--check-seed" in sys.argv:
        check_seed()
    if "--write" in sys.argv:
        # generate_cases.py (sibling, imports this module) builds cases/time.json
        print("use generate_cases.py to (re)build cases/time.json")
