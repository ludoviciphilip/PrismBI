#!/usr/bin/env /usr/bin/python3
"""Builds tests/dax-corpus/cases/time.json from the independent oracle in
oracles/time.py. Every expected value is computed by the oracle (never
copied from an engine). Run: /usr/bin/python3 oracles/generate_time_cases.py
"""
import importlib.util
import json
import sys
from datetime import date
from pathlib import Path

# `time.py` collides with the stdlib `time` module name, so load it directly
# by file path rather than via `import time` (which would resolve to the
# built-in module regardless of sys.path order).
_spec = importlib.util.spec_from_file_location("dax_time_oracle", Path(__file__).resolve().parent / "time.py")
oracle = importlib.util.module_from_spec(_spec)
sys.modules["dax_time_oracle"] = oracle
_spec.loader.exec_module(oracle)

OUT = Path(__file__).resolve().parents[1] / "cases" / "time.json"


def d(v: date) -> dict:
    return {"date": v.isoformat()}


def blank_or(v):
    return None if v is None else v


cases = []


def add(id_, expr, filters, expected, source=None, notes=None, tolerance=None):
    c = {"id": id_, "expr": expr, "expected": expected}
    if filters:
        c["filters"] = filters
    if tolerance is not None:
        c["tolerance"] = tolerance
    if source:
        c["source"] = source
    if notes:
        c["notes"] = notes
    cases.append(c)


def f(col, values):
    return [{"table": "d_Calendario", "column": col, "values": values}]


# ---------------------------------------------------------------------------
# TOTALYTD / DATESYTD / TOTALQTD / DATESQTD / TOTALMTD / DATESMTD
# ---------------------------------------------------------------------------

ctx = oracle.dates_for_filters(f("AnoMes", ["2025-03"]))
add(
    "time.totalytd.month_2025_03",
    "TOTALYTD([Total Vendas], d_Calendario[Data])",
    f("AnoMes", ["2025-03"]),
    oracle.total_vendas(oracle.datesytd(ctx)),
    notes="YTD through Mar 2025 = Jan1-Mar31 2025 (rows 8,9,10,11,12)",
)

ctx = oracle.dates_for_filters(f("Ano", [2024]))
add(
    "time.totalytd.full_year_2024",
    "TOTALYTD([Total Vendas], d_Calendario[Data])",
    f("Ano", [2024]),
    oracle.total_vendas(oracle.datesytd(ctx)),
    notes="Whole year selected: YTD == full year total",
)

ctx = oracle.dates_for_filters(f("AnoMes", ["2024-01"]))
add(
    "time.datesqtd.jan_2024",
    "CALCULATE([Total Vendas], DATESQTD(d_Calendario[Data]))",
    f("AnoMes", ["2024-01"]),
    oracle.total_vendas(oracle.datesqtd(ctx)),
    notes="QTD through Jan 2024 == Jan total (start of Q1)",
)

ctx = oracle.dates_for_filters(f("AnoMes", ["2024-03"]))
add(
    "time.totalqtd.mar_2024",
    "TOTALQTD([Total Vendas], d_Calendario[Data])",
    f("AnoMes", ["2024-03"]),
    oracle.total_vendas(oracle.datesqtd(ctx)),
    notes="QTD through end of Q1 2024 == full Q1 total",
)

ctx = oracle.dates_for_filters(f("Data", ["2025-03-01"]))
add(
    "time.datesqtd.single_day_q1_2025",
    "CALCULATE([Total Vendas], DATESQTD(d_Calendario[Data]))",
    f("Data", ["2025-03-01"]),
    oracle.total_vendas(oracle.datesqtd(ctx)),
    notes="QTD as of 2025-03-01 (single date filter): Q1 2025 start through that day",
)

ctx = oracle.dates_for_filters(f("AnoMes", ["2024-03"]))
add(
    "time.datesmtd.mar_2024",
    "CALCULATE([Total Vendas], DATESMTD(d_Calendario[Data]))",
    f("AnoMes", ["2024-03"]),
    oracle.total_vendas(oracle.datesmtd(ctx)),
    notes="MTD through end of March 2024 == full March total",
)

ctx = oracle.dates_for_filters(f("Data", ["2025-01-10"]))
add(
    "time.totalmtd.single_day_jan_2025",
    "TOTALMTD([Total Vendas], d_Calendario[Data])",
    f("Data", ["2025-01-10"]),
    oracle.total_vendas(oracle.datesmtd(ctx)),
    notes="MTD as of 2025-01-10 (single date filter)",
)

ctx = oracle.dates_for_filters(f("AnoMes", ["2024-06"]))
add(
    "time.datesmtd.no_sales_month",
    "CALCULATE([Total Vendas], DATESMTD(d_Calendario[Data]))",
    f("AnoMes", ["2024-06"]),
    oracle.total_vendas(oracle.datesmtd(ctx)),
    notes="June 2024 MTD through end of month == full June total (one sale on 06-30)",
)

ctx = oracle.dates_for_filters(f("AnoMes", ["2025-02"]))
add(
    "time.datesytd.feb_2025_no_prior_blank_check",
    "CALCULATE([Total Vendas], DATESYTD(d_Calendario[Data]))",
    f("AnoMes", ["2025-02"]),
    oracle.total_vendas(oracle.datesytd(ctx)),
    notes="Same family as core.ti.datesytd but kept distinct id/expr form",
)

# Period with zero sales entirely -> BLANK
ctx = oracle.dates_for_filters(f("AnoMes", ["2024-04"]))
add(
    "time.totalmtd.blank_no_sales",
    "TOTALMTD([Total Vendas], d_Calendario[Data])",
    f("AnoMes", ["2024-04"]),
    oracle.total_vendas(oracle.datesmtd(ctx)),
    notes="April 2024 has zero f_Vendas rows -> BLANK",
)

# ---------------------------------------------------------------------------
# TOTALYTD with fiscal year-end argument
# ---------------------------------------------------------------------------

ctx = oracle.dates_for_filters(f("AnoMes", ["2025-03"]))
add(
    "time.totalytd.fiscal_year_end_0630",
    'TOTALYTD([Total Vendas], d_Calendario[Data], "06-30")',
    f("AnoMes", ["2025-03"]),
    oracle.total_vendas(oracle.datesytd(ctx, "06-30")),
    source="dax.guide/DATESYTD (fiscal year_end_date argument)",
    notes="FY2025 (Jul2024-Jun2025) YTD through Mar 2025: starts 2024-07-01",
)

ctx = oracle.dates_for_filters(f("AnoMes", ["2024-08"]))
add(
    "time.totalytd.fiscal_year_end_0630_second_half",
    'TOTALYTD([Total Vendas], d_Calendario[Data], "06-30")',
    f("AnoMes", ["2024-08"]),
    oracle.total_vendas(oracle.datesytd(ctx, "06-30")),
    notes="Aug 2024 is start of FY2025 (just after 2024-06-30 fiscal year end)",
)

# ---------------------------------------------------------------------------
# SAMEPERIODLASTYEAR
# ---------------------------------------------------------------------------

ctx = oracle.dates_for_filters(f("AnoMes", ["2024-03"]))
add(
    "time.sameperiodlastyear.first_year_blank",
    "CALCULATE([Total Vendas], SAMEPERIODLASTYEAR(d_Calendario[Data]))",
    f("AnoMes", ["2024-03"]),
    oracle.total_vendas(oracle.sameperiodlastyear(ctx)),
    notes="2024 is the first year in the calendar -> prior-year dates don't exist -> BLANK",
)

ctx = oracle.dates_for_filters(f("Trimestre", [1]) + f("Ano", [2025]))
add(
    "time.sameperiodlastyear.quarter_2025",
    "CALCULATE([Total Vendas], SAMEPERIODLASTYEAR(d_Calendario[Data]))",
    f("Trimestre", [1]) + f("Ano", [2025]),
    oracle.total_vendas(oracle.sameperiodlastyear(ctx)),
    notes="Q1 2025 -> Q1 2024",
)

# ---------------------------------------------------------------------------
# DATEADD across intervals, positive and negative, incl. month-end edge cases
# ---------------------------------------------------------------------------

ctx = oracle.dates_for_filters(f("Data", ["2024-01-31"]))
add(
    "time.dateadd.month_end_jan31_minus1",
    "CALCULATE([Total Vendas], DATEADD(d_Calendario[Data], -1, MONTH))",
    f("Data", ["2024-01-31"]),
    oracle.total_vendas(oracle.dateadd(ctx, -1, "MONTH")),
    notes="Jan 31 - 1 month clamps to Dec 31 (Dec has 31 days, no clamping needed here)",
)

ctx = oracle.dates_for_filters(f("Data", ["2025-01-31"]))
add(
    "time.dateadd.month_end_jan31_minus1_feb_clamp",
    "CALCULATE([Total Vendas], DATEADD(d_Calendario[Data], -1, MONTH))",
    f("Data", ["2025-01-31"]),
    oracle.total_vendas(oracle.dateadd(ctx, -1, "MONTH")),
    notes="Jan 31 2025 - 1 month clamps to Feb 28 2025 (non-leap)",
)

ctx = oracle.dates_for_filters(f("Data", ["2024-03-31"]))
add(
    "time.dateadd.month_end_mar31_minus1_leap_feb29",
    "CALCULATE([Total Vendas], DATEADD(d_Calendario[Data], -1, MONTH))",
    f("Data", ["2024-03-31"]),
    oracle.total_vendas(oracle.dateadd(ctx, -1, "MONTH")),
    notes="Mar 31 2024 - 1 month clamps to Feb 29 2024 (leap year)",
)

ctx = oracle.dates_for_filters(f("Data", ["2025-03-31"]))
add(
    "time.dateadd.month_end_mar31_minus1_nonleap_feb28",
    "CALCULATE([Total Vendas], DATEADD(d_Calendario[Data], -1, MONTH))",
    f("Data", ["2025-03-31"]),
    oracle.total_vendas(oracle.dateadd(ctx, -1, "MONTH")),
    notes="Mar 31 2025 - 1 month clamps to Feb 28 2025 (non-leap)",
)

ctx = oracle.dates_for_filters(f("AnoMes", ["2024-03"]))
add(
    "time.dateadd.month_plus1",
    "CALCULATE([Total Vendas], DATEADD(d_Calendario[Data], 1, MONTH))",
    f("AnoMes", ["2024-03"]),
    oracle.total_vendas(oracle.dateadd(ctx, 1, "MONTH")),
    notes="March 2024 + 1 month = April 2024 (no sales -> BLANK)",
)

ctx = oracle.dates_for_filters(f("AnoMes", ["2024-03"]))
add(
    "time.dateadd.quarter_minus1",
    "CALCULATE([Total Vendas], DATEADD(d_Calendario[Data], -1, QUARTER))",
    f("AnoMes", ["2024-03"]),
    oracle.total_vendas(oracle.dateadd(ctx, -1, "QUARTER")),
    notes="March 2024 - 1 quarter shifts into Dec 2023, before calendar start -> BLANK",
)

ctx = oracle.dates_for_filters(f("Trimestre", [1]) + f("Ano", [2025]))
add(
    "time.dateadd.quarter_minus1_q1_2025",
    "CALCULATE([Total Vendas], DATEADD(d_Calendario[Data], -1, QUARTER))",
    f("Trimestre", [1]) + f("Ano", [2025]),
    oracle.total_vendas(oracle.dateadd(ctx, -1, "QUARTER")),
    notes="Q1 2025 - 1 quarter = Q4 2024",
)

ctx = oracle.dates_for_filters(f("Trimestre", [2]) + f("Ano", [2024]))
add(
    "time.dateadd.quarter_plus1_q2_2024",
    "CALCULATE([Total Vendas], DATEADD(d_Calendario[Data], 1, QUARTER))",
    f("Trimestre", [2]) + f("Ano", [2024]),
    oracle.total_vendas(oracle.dateadd(ctx, 1, "QUARTER")),
    notes="Q2 2024 + 1 quarter = Q3 2024 (no sales -> BLANK)",
)

ctx = oracle.dates_for_filters(f("Ano", [2024]))
add(
    "time.dateadd.year_minus1_blank",
    "CALCULATE([Total Vendas], DATEADD(d_Calendario[Data], -1, YEAR))",
    f("Ano", [2024]),
    oracle.total_vendas(oracle.dateadd(ctx, -1, "YEAR")),
    notes="2024 - 1 year = 2023, before calendar start -> BLANK",
)

ctx = oracle.dates_for_filters(f("Ano", [2024]))
add(
    "time.dateadd.year_plus1",
    "CALCULATE([Total Vendas], DATEADD(d_Calendario[Data], 1, YEAR))",
    f("Ano", [2024]),
    oracle.total_vendas(oracle.dateadd(ctx, 1, "YEAR")),
    notes="2024 + 1 year = 2025",
)

ctx = oracle.dates_for_filters(f("Data", ["2025-01-31"]))
add(
    "time.dateadd.day_minus1_hits_sale",
    "CALCULATE([Total Vendas], DATEADD(d_Calendario[Data], -1, DAY))",
    f("Data", ["2025-01-31"]),
    oracle.total_vendas(oracle.dateadd(ctx, -1, "DAY")),
    notes="2025-01-31 - 1 day = 2025-01-30 (no sale -> BLANK)",
)

ctx = oracle.dates_for_filters(f("Data", ["2024-03-01"]))
add(
    "time.dateadd.day_plus4_hits_sale",
    "CALCULATE([Total Vendas], DATEADD(d_Calendario[Data], 4, DAY))",
    f("Data", ["2024-03-01"]),
    oracle.total_vendas(oracle.dateadd(ctx, 4, "DAY")),
    notes="2024-03-01 + 4 days = 2024-03-05, hits row4 (150)",
)

# ---------------------------------------------------------------------------
# PARALLELPERIOD across intervals
# ---------------------------------------------------------------------------

ctx = oracle.dates_for_filters(f("AnoMes", ["2024-03"]))
add(
    "time.parallelperiod.month_minus1",
    "CALCULATE([Total Vendas], PARALLELPERIOD(d_Calendario[Data], -1, MONTH))",
    f("AnoMes", ["2024-03"]),
    oracle.total_vendas(oracle.parallelperiod(ctx, -1, "MONTH")),
    notes="March 2024 parallel period -1 month = Feb 2024",
)

ctx = oracle.dates_for_filters(f("AnoMes", ["2024-02"]))
add(
    "time.parallelperiod.month_plus1",
    "CALCULATE([Total Vendas], PARALLELPERIOD(d_Calendario[Data], 1, MONTH))",
    f("AnoMes", ["2024-02"]),
    oracle.total_vendas(oracle.parallelperiod(ctx, 1, "MONTH")),
    notes="Feb 2024 parallel period +1 month = Mar 2024",
)

ctx = oracle.dates_for_filters(f("Trimestre", [1]) + f("Ano", [2025]))
add(
    "time.parallelperiod.quarter_minus1",
    "CALCULATE([Total Vendas], PARALLELPERIOD(d_Calendario[Data], -1, QUARTER))",
    f("Trimestre", [1]) + f("Ano", [2025]),
    oracle.total_vendas(oracle.parallelperiod(ctx, -1, "QUARTER")),
    notes="Q1 2025 parallel period -1 quarter = Q4 2024",
)

# ---------------------------------------------------------------------------
# PREVIOUS/NEXT DAY/MONTH/QUARTER/YEAR
# ---------------------------------------------------------------------------

ctx = oracle.dates_for_filters(f("Data", ["2025-01-31"]))
add(
    "time.previousday.hits_sale",
    "CALCULATE([Total Vendas], PREVIOUSDAY(d_Calendario[Data]))",
    f("Data", ["2025-01-31"]),
    oracle.total_vendas(oracle.previousday(ctx)),
    notes="PREVIOUSDAY of 2025-01-31 = 2025-01-30 (no sale -> BLANK)",
)

ctx = oracle.dates_for_filters(f("Data", ["2025-01-11"]))
add(
    "time.previousday.day_before_sale",
    "CALCULATE([Total Vendas], PREVIOUSDAY(d_Calendario[Data]))",
    f("Data", ["2025-01-11"]),
    oracle.total_vendas(oracle.previousday(ctx)),
    notes="PREVIOUSDAY of 2025-01-11 = 2025-01-10, hits row8 (20)",
)

ctx = oracle.dates_for_filters(f("Data", ["2024-12-30"]))
add(
    "time.nextday.hits_sale",
    "CALCULATE([Total Vendas], NEXTDAY(d_Calendario[Data]))",
    f("Data", ["2024-12-30"]),
    oracle.total_vendas(oracle.nextday(ctx)),
    notes="NEXTDAY of 2024-12-30 = 2024-12-31, hits row7 (60)",
)

ctx = oracle.dates_for_filters(f("AnoMes", ["2025-01"]))
add(
    "time.previousmonth.first_date_based",
    "CALCULATE([Total Vendas], PREVIOUSMONTH(d_Calendario[Data]))",
    f("AnoMes", ["2025-01"]),
    oracle.total_vendas(oracle.previousperiod(ctx, "MONTH")),
    notes="Distinct id/expr variant of the core.ti.previousmonth seed case",
)

ctx = oracle.dates_for_filters(f("Data", ["2024-01-15", "2024-01-20"]))
add(
    "time.nextmonth.uses_last_date_of_multi_day_selection",
    "CALCULATE([Total Vendas], NEXTMONTH(d_Calendario[Data]))",
    f("Data", ["2024-01-15", "2024-01-20"]),
    oracle.total_vendas(oracle.nextperiod(ctx, "MONTH")),
    source="learn.microsoft.com/en-us/dax/time-intelligence-functions-dax (NEXTMONTH uses last date)",
    notes="Selection {Jan15,Jan20} 2024 -> last date Jan20 -> next month = Feb 2024",
)

ctx = oracle.dates_for_filters(f("Trimestre", [1]) + f("Ano", [2025]))
add(
    "time.previousquarter.q1_2025",
    "CALCULATE([Total Vendas], PREVIOUSQUARTER(d_Calendario[Data]))",
    f("Trimestre", [1]) + f("Ano", [2025]),
    oracle.total_vendas(oracle.previousperiod(ctx, "QUARTER")),
    notes="PREVIOUSQUARTER of Q1 2025 = Q4 2024 (no sales -> BLANK)",
)

ctx = oracle.dates_for_filters(f("Ano", [2024]))
add(
    "time.previousyear.2024_before_calendar_blank",
    "CALCULATE([Total Vendas], PREVIOUSYEAR(d_Calendario[Data]))",
    f("Ano", [2024]),
    oracle.total_vendas(oracle.previousperiod(ctx, "YEAR")),
    notes="PREVIOUSYEAR of 2024 = 2023, outside calendar -> BLANK",
)

ctx = oracle.dates_for_filters(f("Ano", [2024]))
add(
    "time.nextyear.2024_to_2025",
    "CALCULATE([Total Vendas], NEXTYEAR(d_Calendario[Data]))",
    f("Ano", [2024]),
    oracle.total_vendas(oracle.nextperiod(ctx, "YEAR")),
    notes="NEXTYEAR of 2024 = 2025",
)

# ---------------------------------------------------------------------------
# DATESBETWEEN / DATESINPERIOD
# ---------------------------------------------------------------------------

add(
    "time.datesbetween.literal_range",
    "CALCULATE([Total Vendas], DATESBETWEEN(d_Calendario[Data], DATE(2024,1,1), DATE(2024,3,31)))",
    None,
    oracle.total_vendas(oracle.datesbetween(date(2024, 1, 1), date(2024, 3, 31))),
    notes="Absolute literal range Jan1-Mar31 2024, ignores any current context",
)

add(
    "time.datesbetween.literal_range_no_sales",
    "CALCULATE([Total Vendas], DATESBETWEEN(d_Calendario[Data], DATE(2024,4,1), DATE(2024,5,31)))",
    None,
    oracle.total_vendas(oracle.datesbetween(date(2024, 4, 1), date(2024, 5, 31))),
    notes="April-May 2024 has no f_Vendas rows -> BLANK",
)

add(
    "time.datesbetween.single_day",
    "CALCULATE([Total Vendas], DATESBETWEEN(d_Calendario[Data], DATE(2024,3,5), DATE(2024,3,5)))",
    None,
    oracle.total_vendas(oracle.datesbetween(date(2024, 3, 5), date(2024, 3, 5))),
    notes="Start==end collapses to a single day",
)

add(
    "time.datesinperiod.rolling_12m_ending_2025_06_30",
    "CALCULATE([Total Vendas], DATESINPERIOD(d_Calendario[Data], DATE(2025,6,30), -12, MONTH))",
    None,
    oracle.total_vendas(oracle.datesinperiod(date(2025, 6, 30), -12, "MONTH")),
    source="learn.microsoft.com/en-us/dax/datesinperiod-function-dax (worked example)",
    notes="Rolling 12 months ending 2025-06-30 = 2024-07-01..2025-06-30",
)

add(
    "time.datesinperiod.forward_3_months",
    "CALCULATE([Total Vendas], DATESINPERIOD(d_Calendario[Data], DATE(2024,1,1), 3, MONTH))",
    None,
    oracle.total_vendas(oracle.datesinperiod(date(2024, 1, 1), 3, "MONTH")),
    notes="Forward window [2024-01-01, 2024-04-01) i.e. Jan-Mar 2024",
)

add(
    "time.datesinperiod.backward_7_days",
    "CALCULATE([Total Vendas], DATESINPERIOD(d_Calendario[Data], DATE(2025,2,14), -7, DAY))",
    None,
    oracle.total_vendas(oracle.datesinperiod(date(2025, 2, 14), -7, "DAY")),
    notes="Backward 7-day window ending 2025-02-14 inclusive, hits row10",
)

add(
    "time.datesinperiod.forward_1_year",
    "CALCULATE([Total Vendas], DATESINPERIOD(d_Calendario[Data], DATE(2024,1,1), 1, YEAR))",
    None,
    oracle.total_vendas(oracle.datesinperiod(date(2024, 1, 1), 1, "YEAR")),
    notes="Forward 1-year window [2024-01-01,2025-01-01) == calendar year 2024",
)

# ---------------------------------------------------------------------------
# STARTOFMONTH/QUARTER/YEAR, ENDOFMONTH/QUARTER/YEAR (date-returning)
#
# These are DAX *table* functions (single-column/single-row tables); real DAX
# implicitly coerces that to a scalar when used as a whole measure. To get a
# definite scalar we extract it with an explicit MAX(<dates>), and nest two
# CALCULATE calls -- CALCULATE(CALCULATE(MAX(dates), TIFunc(dates)), filter)
# -- so the outer slicer-style filter narrows the context *before* the TI
# function reads it (this mirrors how a Power BI visual filter + a measure's
# own CALCULATE compose, and sidesteps an engine quirk where passing the
# slicer filter and the TI function as two simultaneous arguments of the same
# CALCULATE yields a spurious BLANK even though each one works alone).
# ---------------------------------------------------------------------------

ctx = oracle.dates_for_filters(f("AnoMes", ["2024-03"]))
add(
    "time.startofmonth.march_2024",
    'CALCULATE(CALCULATE(MAX(d_Calendario[Data]), STARTOFMONTH(d_Calendario[Data])), d_Calendario[AnoMes] = "2024-03")',
    None,
    d(oracle.startofmonth(ctx)[0]),
    notes="Scalar extracted via nested CALCULATE(MAX(...), STARTOFMONTH(...))",
)

ctx = oracle.dates_for_filters(f("AnoMes", ["2024-03"]))
add(
    "time.endofmonth.march_2024",
    'CALCULATE(CALCULATE(MAX(d_Calendario[Data]), ENDOFMONTH(d_Calendario[Data])), d_Calendario[AnoMes] = "2024-03")',
    None,
    d(oracle.endofmonth(ctx)[0]),
)

ctx = oracle.dates_for_filters(f("AnoMes", ["2024-02"]))
add(
    "time.endofmonth.leap_february_2024",
    'CALCULATE(CALCULATE(MAX(d_Calendario[Data]), ENDOFMONTH(d_Calendario[Data])), d_Calendario[AnoMes] = "2024-02")',
    None,
    d(oracle.endofmonth(ctx)[0]),
    notes="Leap year: end of Feb 2024 is the 29th",
)

ctx = oracle.dates_for_filters(f("AnoMes", ["2025-02"]))
add(
    "time.endofmonth.nonleap_february_2025",
    'CALCULATE(CALCULATE(MAX(d_Calendario[Data]), ENDOFMONTH(d_Calendario[Data])), d_Calendario[AnoMes] = "2025-02")',
    None,
    d(oracle.endofmonth(ctx)[0]),
    notes="Non-leap year: end of Feb 2025 is the 28th",
)

ctx = oracle.dates_for_filters(f("Trimestre", [1]) + f("Ano", [2025]))
add(
    "time.startofquarter.q1_2025",
    "CALCULATE(CALCULATE(MAX(d_Calendario[Data]), STARTOFQUARTER(d_Calendario[Data])), d_Calendario[Trimestre] = 1, d_Calendario[Ano] = 2025)",
    None,
    d(oracle.startofquarter(ctx)[0]),
    notes="Trimestre/Ano passed as two separate CALCULATE filter args (not &&)",
)

ctx = oracle.dates_for_filters(f("Trimestre", [1]) + f("Ano", [2025]))
add(
    "time.endofquarter.q1_2025",
    "CALCULATE(CALCULATE(MAX(d_Calendario[Data]), ENDOFQUARTER(d_Calendario[Data])), d_Calendario[Trimestre] = 1, d_Calendario[Ano] = 2025)",
    None,
    d(oracle.endofquarter(ctx)[0]),
)

ctx = oracle.dates_for_filters(f("Ano", [2024]))
add(
    "time.startofyear.2024",
    "CALCULATE(CALCULATE(MAX(d_Calendario[Data]), STARTOFYEAR(d_Calendario[Data])), d_Calendario[Ano] = 2024)",
    None,
    d(oracle.startofyear(ctx)[0]),
)

ctx = oracle.dates_for_filters(f("Ano", [2025]))
add(
    "time.endofyear.2025",
    "CALCULATE(CALCULATE(MAX(d_Calendario[Data]), ENDOFYEAR(d_Calendario[Data])), d_Calendario[Ano] = 2025)",
    None,
    d(oracle.endofyear(ctx)[0]),
)

# ---------------------------------------------------------------------------
# FIRSTDATE / LASTDATE (date-returning)
# ---------------------------------------------------------------------------

ctx = oracle.dates_for_filters(f("AnoMes", ["2024-03"]))
add(
    "time.firstdate.march_2024",
    'CALCULATE(CALCULATE(MAX(d_Calendario[Data]), FIRSTDATE(d_Calendario[Data])), d_Calendario[AnoMes] = "2024-03")',
    None,
    d(oracle.firstdate(ctx)[0]),
)

ctx = oracle.dates_for_filters(f("AnoMes", ["2024-03"]))
add(
    "time.lastdate.march_2024",
    'CALCULATE(CALCULATE(MAX(d_Calendario[Data]), LASTDATE(d_Calendario[Data])), d_Calendario[AnoMes] = "2024-03")',
    None,
    d(oracle.lastdate(ctx)[0]),
)

# ---------------------------------------------------------------------------
# OPENINGBALANCE*/CLOSINGBALANCE*
# ---------------------------------------------------------------------------

ctx = oracle.dates_for_filters(f("AnoMes", ["2025-01"]))
add(
    "time.openingbalancemonth.jan_2025",
    "OPENINGBALANCEMONTH([Total Vendas], d_Calendario[Data])",
    f("AnoMes", ["2025-01"]),
    oracle.total_vendas(oracle.openingbalance(ctx, "MONTH")),
    source="dax.guide/openingbalancemonth (evaluated at last date of previous month)",
    notes="Opening balance of Jan 2025 = value at 2024-12-31, hits row7 (60)",
)

ctx = oracle.dates_for_filters(f("AnoMes", ["2025-01"]))
add(
    "time.closingbalancemonth.jan_2025",
    "CLOSINGBALANCEMONTH([Total Vendas], d_Calendario[Data])",
    f("AnoMes", ["2025-01"]),
    oracle.total_vendas(oracle.closingbalance(ctx, "MONTH")),
    notes="Closing balance of Jan 2025 = value at 2025-01-31, hits row9 (300)",
)

ctx = oracle.dates_for_filters(f("AnoMes", ["2024-03"]))
add(
    "time.openingbalancemonth.march_2024_blank",
    "OPENINGBALANCEMONTH([Total Vendas], d_Calendario[Data])",
    f("AnoMes", ["2024-03"]),
    oracle.total_vendas(oracle.openingbalance(ctx, "MONTH")),
    notes="Opening balance of March 2024 = value at 2024-02-29 (leap), no sale that day -> BLANK",
)

ctx = oracle.dates_for_filters(f("Trimestre", [1]) + f("Ano", [2025]))
add(
    "time.openingbalancequarter.q1_2025",
    "OPENINGBALANCEQUARTER([Total Vendas], d_Calendario[Data])",
    f("Trimestre", [1]) + f("Ano", [2025]),
    oracle.total_vendas(oracle.openingbalance(ctx, "QUARTER")),
    notes="Opening balance of Q1 2025 = value at 2024-12-31, hits row7 (60)",
)

ctx = oracle.dates_for_filters(f("Trimestre", [1]) + f("Ano", [2025]))
add(
    "time.closingbalancequarter.q1_2025",
    "CLOSINGBALANCEQUARTER([Total Vendas], d_Calendario[Data])",
    f("Trimestre", [1]) + f("Ano", [2025]),
    oracle.total_vendas(oracle.closingbalance(ctx, "QUARTER")),
    notes="Closing balance of Q1 2025 = value at 2025-03-31, hits row12 (150)",
)

ctx = oracle.dates_for_filters(f("Ano", [2025]))
add(
    "time.openingbalanceyear.2025",
    "OPENINGBALANCEYEAR([Total Vendas], d_Calendario[Data])",
    f("Ano", [2025]),
    oracle.total_vendas(oracle.openingbalance(ctx, "YEAR")),
    notes="Opening balance of 2025 = value at 2024-12-31, hits row7 (60)",
)

ctx = oracle.dates_for_filters(f("Ano", [2024]))
add(
    "time.closingbalanceyear.2024",
    "CLOSINGBALANCEYEAR([Total Vendas], d_Calendario[Data])",
    f("Ano", [2024]),
    oracle.total_vendas(oracle.closingbalance(ctx, "YEAR")),
    notes="Closing balance of 2024 = value at 2024-12-31, hits row7 (60)",
)

# ---------------------------------------------------------------------------
# FIRSTNONBLANK / LASTNONBLANK
# ---------------------------------------------------------------------------

ctx = oracle.dates_for_filters(f("Ano", [2024]))
add(
    "time.firstnonblank.year_2024",
    'CALCULATE(CALCULATE(MAX(d_Calendario[Data]), FIRSTNONBLANK(d_Calendario[Data], [Total Vendas])), d_Calendario[Ano] = 2024)',
    None,
    d(oracle.firstnonblank(ctx)),
    notes="First date in 2024 with non-blank Total Vendas = 2024-01-15 (row1)",
)

ctx = oracle.dates_for_filters(f("Ano", [2024]))
add(
    "time.lastnonblank.year_2024",
    'CALCULATE(CALCULATE(MAX(d_Calendario[Data]), LASTNONBLANK(d_Calendario[Data], [Total Vendas])), d_Calendario[Ano] = 2024)',
    None,
    d(oracle.lastnonblank(ctx)),
    notes="Last date in 2024 with non-blank Total Vendas = 2024-12-31 (row7)",
)

ctx = oracle.dates_for_filters(f("Trimestre", [2]) + f("Ano", [2024]))
add(
    "time.firstnonblank.quarter_single_sale",
    'CALCULATE(CALCULATE(MAX(d_Calendario[Data]), FIRSTNONBLANK(d_Calendario[Data], [Total Vendas])), d_Calendario[Trimestre] = 2, d_Calendario[Ano] = 2024)',
    None,
    d(oracle.firstnonblank(ctx)),
    notes="Q2 2024 has a single sale on 2024-06-30 (row6); first == last",
)

# ---------------------------------------------------------------------------
# Filters by Trimestre / MesNum (direct, no time-intel function) combined
# with a time-intel function — exercise those two generated columns.
# ---------------------------------------------------------------------------

ctx = oracle.dates_for_filters(f("MesNum", [3]) + f("Ano", [2025]))
add(
    "time.dateadd.mesnum_filter_quarter_minus1",
    "CALCULATE([Total Vendas], DATEADD(d_Calendario[Data], -1, QUARTER))",
    f("MesNum", [3]) + f("Ano", [2025]),
    oracle.total_vendas(oracle.dateadd(ctx, -1, "QUARTER")),
    notes="Filter via MesNum=3 & Ano=2025 (March 2025), -1 quarter = Dec 2024",
)

ctx = oracle.dates_for_filters(f("Trimestre", [4]) + f("Ano", [2024]))
add(
    "time.samperiodlastyear.trimestre_filter_q4_2024",
    "CALCULATE([Total Vendas], SAMEPERIODLASTYEAR(d_Calendario[Data]))",
    f("Trimestre", [4]) + f("Ano", [2024]),
    oracle.total_vendas(oracle.sameperiodlastyear(ctx)),
    notes="Q4 2024 (filtered via Trimestre=4) -> Q4 2023, before calendar -> BLANK",
)

# ---------------------------------------------------------------------------
# Leap year 2024-02-29 edge cases
# ---------------------------------------------------------------------------

add(
    "time.dateadd.year_minus1_from_leap_day",
    "CALCULATE([Total Vendas], DATEADD(d_Calendario[Data], -1, YEAR))",
    f("Data", ["2024-02-29"]),
    oracle.total_vendas(oracle.dateadd(oracle.dates_for_filters(f("Data", ["2024-02-29"])), -1, "YEAR")),
    notes="2024-02-29 - 1 year clamps to 2023-02-28, outside calendar -> BLANK",
)

add(
    "time.dateadd.year_plus1_from_leap_day_clamps",
    "CALCULATE([Total Vendas], DATEADD(d_Calendario[Data], 1, YEAR))",
    f("Data", ["2024-02-29"]),
    oracle.total_vendas(oracle.dateadd(oracle.dates_for_filters(f("Data", ["2024-02-29"])), 1, "YEAR")),
    notes="2024-02-29 + 1 year clamps to 2025-02-28 (no sale -> BLANK)",
)

ctx = oracle.dates_for_filters(f("Data", ["2024-02-29"]))
add(
    "time.endofmonth.from_leap_day",
    "CALCULATE(CALCULATE(MAX(d_Calendario[Data]), ENDOFMONTH(d_Calendario[Data])), d_Calendario[Data] = DATE(2024,2,29))",
    None,
    d(oracle.endofmonth(ctx)[0]),
    notes="ENDOFMONTH of the leap day itself is still 2024-02-29",
)

# ---------------------------------------------------------------------------
# DATESBETWEEN / DATESINPERIOD combined with TOTALYTD-style checks and an
# extra fiscal-year boundary case to round things out.
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    ids = [c["id"] for c in cases]
    assert len(ids) == len(set(ids)), "duplicate ids generated"
    seed_ids = {
        "core.ti.totalytd",
        "core.ti.datesytd",
        "core.ti.sameperiodlastyear_year",
        "core.ti.sameperiodlastyear_month",
        "core.ti.previousmonth",
        "core.ti.dateadd_month",
        "core.ti.parallelperiod_year",
    }
    assert not (set(ids) & seed_ids), "collides with seed ids"
    payload = {"model": "vendas", "category": "time", "cases": cases}
    OUT.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n")
    print(f"wrote {len(cases)} cases to {OUT}")
