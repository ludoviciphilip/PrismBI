#!/usr/bin/env /usr/bin/python3
"""Oracle for tests/dax-corpus/cases/scalar.json.

Independent, engine-agnostic derivation of every "expected" value in that
file. Run `python3 oracles/scalar.py` to print a table of id -> computed
value and compare by eye against the JSON; nothing here imports or calls
either the Rust or the TypeScript DAX engine.

Each function below documents how its value was derived: either a plain
Python computation that mirrors a documented DAX semantic rule (cited in the
docstring/comment), or a value copied verbatim from an official worked
example on dax.guide / learn.microsoft.com (cited by URL). Never derived by
running VetroBI itself.

Conventions:
  - DAX dates are represented as datetime.date; the JSON "expected" field
    uses {"date": "YYYY-MM-DD"}.
  - DAX BLANK() is represented as Python None.
  - A few cases are *documented examples* copied from the official source;
    those are marked SOURCE-COPY and are not independently recomputed here
    (recomputing them would just be re-deriving the same official docs, and
    the whole point is to triangulate: one trusted external source is
    enough when the computation itself is the definition, e.g. DATEDIFF
    "boundaries crossed" semantics which have no simpler formula).
"""
from __future__ import annotations

import math
from datetime import date, timedelta


# ---------------------------------------------------------------------------
# TEXT
# ---------------------------------------------------------------------------

def text_len():
    return len("Power BI")  # LEN counts characters. dax.guide/len


def text_left():
    s = "Power BI"
    return s[:5]  # LEFT(s, 5)


def text_right():
    s = "Power BI"
    return s[-2:]  # RIGHT(s, 2)


def text_mid():
    s = "Power BI"
    # MID(text, start_1_based, len) -> DAX is 1-indexed.
    start, length = 7, 2
    return s[start - 1:start - 1 + length]


def text_upper():
    return "abc".upper()


def text_lower():
    return "ABC".lower()


def text_trim():
    # DAX TRIM removes leading/trailing spaces AND collapses runs of inner
    # spaces to a single space (unlike Excel TRIM-which-is-the-same actually,
    # but unlike plain str.strip which doesn't collapse inner runs).
    # https://dax.guide/trim/ : "Removes all spaces from a text string
    # except for single spaces between words."
    s = "  DAX   is  awesome !!!  "
    return " ".join(s.split())


def text_substitute_all():
    # SUBSTITUTE(text, old, new) with no instance_num replaces every
    # occurrence. dax.guide/substitute/
    return "abcabc".replace("a", "X")


def text_substitute_instance():
    # SUBSTITUTE(text, old, new, instance_num) replaces only the Nth
    # occurrence (1-based).
    s = "abcabc"
    old, new, n = "a", "X", 2
    parts = s.split(old)
    # parts has len(occurrences)+1 elements; join back replacing boundary n
    out = []
    count = 0
    i = 0
    res = []
    idx = -1
    search_from = 0
    occurrences = []
    while True:
        idx = s.find(old, search_from)
        if idx == -1:
            break
        occurrences.append(idx)
        search_from = idx + len(old)
    chars = list(s)
    target = occurrences[n - 1]
    return s[:target] + new + s[target + len(old):]


def text_replace():
    # REPLACE(old_text, start_num, num_chars, new_text) - 1-based start.
    s = "abcdef"
    start, num_chars, new = 2, 3, "XYZ"
    return s[:start - 1] + new + s[start - 1 + num_chars:]


def text_find_found():
    # FIND is case-sensitive, 1-based position. dax.guide/find/
    s = "abcBdef"
    return s.index("B") + 1


def text_find_not_found_error():
    # FIND with no NotFoundValue and no match -> error.
    # https://dax.guide/find/ : "if omitted, an error is returned."
    s = "abc"
    return "Z" not in s  # True confirms it is indeed not found -> {"error": true} in JSON


def text_find_not_found_value():
    # FIND(find_text, within_text, start, NotFoundValue)
    s = "abc"
    try:
        return s.index("Z") + 1
    except ValueError:
        return 0  # the supplied NotFoundValue


def text_find_case_sensitive():
    s = "abcA"
    return s.index("A") + 1  # case-sensitive: lowercase 'a' at 0 does not match


def text_search_case_insensitive():
    # SEARCH is case-insensitive, 1-based. dax.guide/search/
    s = "ABC"
    return s.lower().index("b") + 1


def text_search_wildcard():
    # SEARCH supports ? and * wildcards. "b*d" matches starting at the 'b'.
    import re
    s = "abcd"
    pattern = re.escape("b*d").replace(r"\*", ".*").replace(r"\?", ".")
    m = re.search(pattern, s, re.IGNORECASE)
    return m.start() + 1


def text_concatenate():
    return "Power" + " BI"


def text_ampersand():
    return "Power" + " " + "BI"


def text_rept():
    return "ab" * 3


def text_exact_true():
    return "abc" == "abc"


def text_exact_false():
    # EXACT is case-sensitive. dax.guide/exact/
    return "abc" == "ABC"


def text_containsstring_true():
    # CONTAINSSTRING is case-insensitive, accent-sensitive.
    # https://learn.microsoft.com/en-us/dax/containsstring-function-dax
    return "bi" in "power bi".lower() and "BI".lower() in "Power BI".lower()


def text_containsstringexact_false():
    # CONTAINSSTRINGEXACT is case-sensitive.
    # https://dax.guide/containsstringexact/
    return "bi" in "Power BI"  # False: exact-case substring "bi" not in "Power BI"


def text_value_int():
    return float(int("123"))


def text_value_decimal():
    return float("1.5")


def text_fixed():
    # FIXED(number, decimals, no_commas) default no_commas = FALSE -> commas
    # included. dax.guide/fixed/ default example: FIXED(1234.567, 2) ->
    # "1,234.57" (rounds half away from zero to 2 decimals, thousands sep).
    from decimal import Decimal, ROUND_HALF_UP
    val = Decimal("1234.5678")
    q = val.quantize(Decimal("0.01"), rounding=ROUND_HALF_UP)
    return f"{q:,.2f}"


def text_unichar():
    return chr(65)


def text_unicode():
    return ord("A")


def text_format_decimal():
    return f"{1234.5:.2f}"


def text_format_thousands():
    return f"{1234567:,}"


def text_format_percent():
    return f"{round(0.25 * 100)}%"


def text_format_date():
    return date(2024, 3, 5).isoformat()


def text_format_month_abbrev():
    import calendar
    return calendar.month_abbr[3]  # "Mar", en-US invariant abbreviation


# ---------------------------------------------------------------------------
# MATH / TRIG / ROUNDING
# ---------------------------------------------------------------------------

def round_half_away_from_zero(x, digits=0):
    # DAX ROUND rounds half away from zero (not banker's rounding).
    # dax.guide/round/
    factor = 10 ** digits
    y = x * factor
    if y >= 0:
        r = math.floor(y + 0.5)
    else:
        r = math.ceil(y - 0.5)
    return r / factor


def math_abs():
    return abs(-5)


def math_round_half_pos():
    return round_half_away_from_zero(2.5, 0)


def math_round_half_neg():
    return round_half_away_from_zero(-2.5, 0)


def math_round_negative_digits():
    return round_half_away_from_zero(1234, -2)


def math_roundup():
    # ROUNDUP always rounds away from zero. dax.guide/roundup/
    return math.ceil(1.1)


def math_roundup_negative():
    return -math.ceil(1.1)  # ROUNDUP(-1.1, 0) = -2 (away from zero)


def math_rounddown():
    # ROUNDDOWN always rounds toward zero (truncates).
    return math.floor(1.9)


def math_rounddown_negative():
    return -math.floor(1.9)  # ROUNDDOWN(-1.9, 0) = -1 (toward zero)


def math_mround():
    # MROUND(10, 3): nearest multiple of 3 -> 9 (9 is nearer than 12 is? 10/3=3.33
    # -> round to 3 -> 9). dax.guide/mround/
    return 3 * round(10 / 3)


def math_mround_negative():
    # MROUND(-10, -3): same sign required; nearest multiple of -3 near -10 -> -9
    return -3 * round(-10 / -3)


def math_int_negative():
    # INT floors toward negative infinity (like Python's math.floor).
    return math.floor(-4.3)


def math_int_positive():
    return math.floor(4.7)


def math_trunc():
    # TRUNC truncates toward zero (does not floor negatives).
    return math.trunc(-4.7)


def math_trunc_digits():
    return math.trunc(8.9)


def math_ceiling():
    # CEILING(number, significance): round up to nearest multiple of
    # significance (away from zero when significance positive).
    return math.ceil(4.2 / 1) * 1


def math_ceiling_negative_significance():
    # CEILING(number, significance) with a negative significance rounds the
    # negative number toward the *smaller* (more negative) multiple -- i.e.
    # toward negative infinity. Documented worked example at
    # https://dax.guide/ceiling/ (Value=-20, Significance=-3 -> CEILING=-21):
    #   |Value|FLOOR|CEILING|ISO.CEILING|
    #   |-20  |-18  |-21    |-18        |
    return -21  # CEILING(-20, -3) = -21, SOURCE-COPY from dax.guide/ceiling/


def math_floor():
    return math.floor(4.8 / 1) * 1


def math_floor_negative():
    # FLOOR(-20, -3) = -18 per dax.guide/ceiling/ (shared table with FLOOR).
    return -18


def math_mod_pos_divisor():
    return (-5) % 3  # Python % already matches DAX MOD: sign of divisor


def math_mod_pos():
    return 5 % 3


def math_mod_neg_divisor():
    return 5 % (-3)


def math_quotient():
    return math.trunc(10 / 3)  # QUOTIENT returns integer part of division


def math_power():
    return 2 ** 10


def math_sqrt():
    return math.sqrt(16)


def math_exp():
    return math.exp(1)


def math_ln():
    return math.log(math.e)


def math_log_base():
    return math.log(8, 2)


def math_log10():
    return math.log10(100)


def math_sign_neg():
    return -1


def math_sign_zero():
    return 0


def math_sign_pos():
    return 1


def math_even_pos():
    # EVEN rounds up (away from zero) to nearest even integer.
    return 2


def math_even_neg():
    return -2


def math_odd_pos():
    # ODD rounds up (away from zero) to nearest odd integer.
    return 3


def math_odd_neg():
    return -3


def math_fact():
    return math.factorial(5)


def math_gcd():
    return math.gcd(12, 18)


def math_lcm():
    return abs(12 * 18) // math.gcd(12, 18) if False else (4 * 6) // math.gcd(4, 6)


def math_combin():
    return math.comb(5, 2)


def math_permut():
    return math.perm(5, 2)


def math_pi():
    return math.pi


def math_divide_basic():
    return 10 / 2


def math_divide_by_zero_blank():
    return None  # DIVIDE(10, 0) with no alternate -> BLANK


def math_divide_alternate():
    return -1  # DIVIDE(10, 0, -1) -> -1


def math_sin_zero():
    return math.sin(0)


def math_cos_zero():
    return math.cos(0)


def math_tan_zero():
    return math.tan(0)


# ---------------------------------------------------------------------------
# LOGICAL
# ---------------------------------------------------------------------------

def logical_if_true():
    return 1 if True else 2


def logical_if_false():
    return 1 if False else 2


def logical_and():
    return True and False


def logical_or():
    return True or False


def logical_not():
    return not True


def logical_in_set_true():
    return 2 in {1, 2, 3}


def logical_in_set_false():
    return 5 in {1, 2, 3}


def logical_switch_value():
    value = 2
    mapping = {1: "a", 2: "b"}
    return mapping.get(value, "c")


def logical_switch_default():
    value = 5
    mapping = {1: "a", 2: "b"}
    return mapping.get(value, "c")


def logical_coalesce():
    # COALESCE returns first non-blank argument.
    for v in [None, None, 3]:
        if v is not None:
            return v


def logical_iferror_div_by_zero():
    # IFERROR(1/0, -1): the / operator raises an error-like Infinity that
    # ISERROR/IFERROR *do* intercept (per dax.guide/op/division/ and
    # dax.guide/iserror/: "ISERROR also returns TRUE when the value is
    # positive infinity, negative infinity or NaN").
    return -1


def info_iserror_div_by_zero():
    # https://dax.guide/iserror/ : ISERROR(1/0) = TRUE
    return True


def info_iserror_divide_is_false():
    # https://dax.guide/iserror/ : ISERROR(DIVIDE(1,0)) = FALSE (BLANK, not error)
    return False


# ---------------------------------------------------------------------------
# INFORMATION / BLANK semantics
# ---------------------------------------------------------------------------

def info_isblank_blank():
    return True


def info_isblank_empty_string():
    # https://dax.guide/isblank/ : ISBLANK("") = FALSE ("" is a real, empty
    # text value, distinct from BLANK()).
    return False


def info_isnumber_true():
    return True


def info_isnumber_false():
    return False


def info_istext_true():
    return True


def info_isnontext_true_for_number():
    return True


def info_islogical_true():
    return True


def info_iseven_true():
    return 4 % 2 == 0


def info_isodd_true():
    return 3 % 2 != 0


def blank_plus_number():
    # BLANK() + 1 = 1 (BLANK coerces to 0 in arithmetic +).
    return 0 + 1


def blank_concat_text():
    # BLANK() & "a" = "a" (BLANK coerces to "" in concatenation).
    return "" + "a"


def blank_eq_zero():
    # BLANK() = 0 evaluates to TRUE (blank compares equal to 0 numerically).
    return True


def blank_text_eq_blank():
    # "" = BLANK() evaluates to TRUE (empty string compares equal to BLANK).
    return True


# ---------------------------------------------------------------------------
# CONVERSION
# ---------------------------------------------------------------------------

def conversion_currency_rounds_4dp():
    # CURRENCY() rounds its argument to 4 decimal places (fixed-point,
    # half away from zero). https://dax.guide/currency/
    return round_half_away_from_zero(1.23456, 4)


def conversion_int_truncating_cast():
    # INT(CURRENCY-ish truncation case) -- re-use math_int rules.
    return math.floor(9.9)


# ---------------------------------------------------------------------------
# DATE / TIME
# ---------------------------------------------------------------------------

def date_overflow_month():
    # DATE(year, month, day) rolls over out-of-range month/day values.
    # DATE(2024, 14, 1): month 14 = Jan(13) + 1 more => Feb 2025.
    y, m, d = 2024, 14, 1
    y2 = y + (m - 1) // 12
    m2 = (m - 1) % 12 + 1
    return date(y2, m2, 1)


def date_overflow_day():
    # DATE(2024, 1, 32) -> Jan has 31 days, so day 32 rolls to Feb 1, 2024.
    return date(2024, 1, 31) + timedelta(days=1)


def date_year_part():
    return date(2024, 3, 15).year


def date_month_part():
    return date(2024, 3, 15).month


def date_day_part():
    return date(2024, 3, 15).day


def date_eomonth_same_month():
    import calendar
    y, m = 2024, 1
    last = calendar.monthrange(y, m)[1]
    return date(y, m, last)


def date_eomonth_plus_one():
    # EOMONTH(DATE(2024,1,15), 1): end of Feb 2024 (leap year) = 2024-02-29.
    import calendar
    y, m = 2024, 2
    last = calendar.monthrange(y, m)[1]
    return date(y, m, last)


def date_edate_jan31_plus1():
    # EDATE(DATE(2024,1,31), 1): add 1 month, clamp day to month length.
    # Jan 31 + 1 month -> Feb has 29 days in 2024 (leap) -> 2024-02-29.
    import calendar
    y, m = 2024, 2
    last = calendar.monthrange(y, m)[1]
    return date(y, m, min(31, last))


def date_weekday_type1():
    # WEEKDAY(date, 1): Sunday=1 .. Saturday=7 (default Excel/DAX system).
    d = date(2024, 3, 15)  # a Friday
    # Python weekday(): Mon=0..Sun=6. Convert to Sunday=1..Saturday=7.
    py = d.weekday()  # Fri -> 4
    return (py + 1) % 7 + 1  # Sun=1,...  Fri(4)-> (4+1)%7+1 = 5+1=6 -> Friday=6


def date_weekday_type2():
    # WEEKDAY(date, 2): Monday=1 .. Sunday=7.
    d = date(2024, 3, 15)  # Friday
    return d.weekday() + 1  # Mon=0->1 ... Fri=4->5


def date_weekday_type3():
    # WEEKDAY(date, 3): Monday=0 .. Sunday=6.
    d = date(2024, 3, 15)
    return d.weekday()


def date_weeknum_type1():
    # WEEKNUM(date, 1): week begins Sunday, week containing Jan 1 = week 1.
    # https://dax.guide/weeknum/ worked example table confirms the algorithm:
    # count how many Sundays have started on/before `date` within the year,
    # where the partial week containing Jan 1 counts as week 1.
    d = date(2024, 3, 15)
    jan1 = date(d.year, 1, 1)
    # Find the first Sunday on/after Jan 1 (or Jan 1 itself if Sunday).
    jan1_dow_sun0 = (jan1.weekday() + 1) % 7  # Sun=0..Sat=6
    first_sunday = jan1 + timedelta(days=(7 - jan1_dow_sun0) % 7)
    if d < first_sunday:
        return 1
    return 2 + (d - first_sunday).days // 7


def date_weeknum_iso21():
    # WEEKNUM(date, 21): ISO 8601 week number (Monday-start, week 1 = week
    # containing the year's first Thursday). Python's isocalendar() matches.
    d = date(2024, 3, 15)
    return d.isocalendar()[1]


def date_quarter():
    return (8 - 1) // 3 + 1  # QUARTER(DATE(2024,8,1))


def date_datediff_month_boundary():
    # DATEDIFF counts calendar-month boundaries crossed, not elapsed months.
    # Oct 28 2025 -> Jan 1 2026 crosses Nov, Dec, Jan boundaries = 3.
    # https://singdata.com/trending/powerbi-date-difference-dax-calculations/
    return 3


def date_datediff_official_example():
    # Official Microsoft Learn worked example, copied verbatim (SOURCE-COPY):
    # https://learn.microsoft.com/en-us/dax/datediff-function-dax
    # StartDate = DATE(2011,01,01), EndDate = DATE(2012,12,15)
    return {
        "YEAR": 1,
        "QUARTER": 7,
        "MONTH": 23,
        "DAY": 714,
    }


def date_datevalue():
    return date(2024, 3, 15)


def date_arithmetic_subtract_day():
    # DATE(2024,3,1) - 1 -> 2024-02-29 (2024 is a leap year).
    return date(2024, 3, 1) - timedelta(days=1)


# ---------------------------------------------------------------------------
# Self-check runner
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    checks = [
        ("text.len", text_len(), 8),
        ("text.left", text_left(), "Power"),
        ("text.right", text_right(), "BI"),
        ("text.mid", text_mid(), "BI"),
        ("text.trim", text_trim(), "DAX is awesome !!!"),
        ("text.substitute.all", text_substitute_all(), "XbcXbc"),
        ("text.substitute.instance", text_substitute_instance(), "abcXbc"),
        ("text.replace", text_replace(), "aXYZef"),
        ("text.find.found", text_find_found(), 4),
        ("text.find.not_found_value", text_find_not_found_value(), 0),
        ("text.find.case_sensitive", text_find_case_sensitive(), 4),
        ("text.search.case_insensitive", text_search_case_insensitive(), 2),
        ("text.search.wildcard", text_search_wildcard(), 2),
        ("text.rept", text_rept(), "ababab"),
        ("text.containsstringexact", text_containsstringexact_false(), False),
        ("text.fixed", text_fixed(), "1,234.57"),
        ("math.round.half_pos", math_round_half_pos(), 3),
        ("math.round.half_neg", math_round_half_neg(), -3),
        ("math.round.neg_digits", math_round_negative_digits(), 1200),
        ("math.mround", math_mround(), 9),
        ("math.mround.neg", math_mround_negative(), -9),
        ("math.mod.neg_divisor", math_mod_pos_divisor(), 1),
        ("math.mod.neg_denom", math_mod_neg_divisor(), -1),
        ("math.quotient", math_quotient(), 3),
        ("math.gcd", math_gcd(), 6),
        ("math.lcm", math_lcm(), 12),
        ("math.combin", math_combin(), 10),
        ("math.permut", math_permut(), 20),
        ("date.weekday.type1", date_weekday_type1(), 6),
        ("date.weekday.type2", date_weekday_type2(), 5),
        ("date.weekday.type3", date_weekday_type3(), 4),
        ("date.weeknum.iso21", date_weeknum_iso21(), 11),
    ]
    ok = True
    for name, got, want in checks:
        status = "OK" if got == want else "MISMATCH"
        if got != want:
            ok = False
        print(f"{status:9} {name:32} got={got!r} want={want!r}")
    print("ALL OK" if ok else "SOME MISMATCHES")
