//! Property test: `etl_from_m(etl_to_m(q)) == q` (ignoring ids) for every
//! step variant and source kind, over randomly generated queries.
//!
//! Generation respects the documented normalizations (they are semantic
//! no-ops, so the round-trip of *those* inputs lands on the canonical form):
//! `eq/neq null` ⇒ `isNull/isNotNull`; `or[isNull c, eq c ""]` ⇒ `isBlank c`;
//! `and[isNotNull c, neq c ""]` ⇒ `isNotBlank c`; single-child and/or;
//! `count` without column ⇒ `countRows`; custom formulas shaped like a
//! conditional column ⇒ `addConditionalColumn`.

use crate::etl::m_from::query_from_m;
use crate::etl::m_to::query_to_m;
use crate::etl::model::*;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn bool(&mut self) -> bool {
        self.next() & 1 == 1
    }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.below(v.len())]
    }
}

const NAMES: &[&str] = &[
    "Valor",
    "Data",
    "Nome do Mês",
    "Mês/Ano",
    "Entra na DRE (competência)",
    "a\"b",
    "x#(y",
    "each",
    " lead",
    "tab\there",
    "linha\nnova",
    "Ano - Copy",
    "a]b",
    "k=v, w",
    "ç",
    "_x",
    "Column1",
    "a\"; DROP TABLE x; --",
    "日本",
];

const TEXTS: &[&str] = &["", "x", "S1", "it's", "quote\"d", "#(lf) literal", "multi\nline", "ação", "a;b", "\u{1}ctl"];

const NUMBERS: &[f64] = &[0.0, 1.0, -1.0, 6.0, 0.1, -2.5, 1234.5678, 1e-7, 1e20, 123456789.0];

fn name(r: &mut Rng) -> String {
    r.pick(NAMES).to_string()
}

fn names(r: &mut Rng, min: usize) -> Vec<String> {
    let k = min + r.below(3);
    (0..k).map(|_| name(r)).collect()
}

fn scalar(r: &mut Rng, allow_null: bool) -> Scalar {
    match r.below(if allow_null { 4 } else { 3 }) {
        0 => Scalar::Text(r.pick(TEXTS).to_string()),
        1 => Scalar::Number(*r.pick(NUMBERS)),
        2 => Scalar::Bool(r.bool()),
        _ => Scalar::Null,
    }
}

fn text_scalar(r: &mut Rng) -> Scalar {
    Scalar::Text(r.pick(TEXTS).to_string())
}

fn leaf(r: &mut Rng) -> Cond {
    let c = name(r);
    let op = match r.below(16) {
        0 => CondOp::Eq { value: scalar(r, false) },
        1 => CondOp::Neq { value: scalar(r, false) },
        2 => CondOp::Gt { value: scalar(r, true) },
        3 => CondOp::Gte { value: scalar(r, true) },
        4 => CondOp::Lt { value: scalar(r, true) },
        5 => CondOp::Lte { value: scalar(r, true) },
        6 => CondOp::Contains { value: text_scalar(r) },
        7 => CondOp::NotContains { value: text_scalar(r) },
        8 => CondOp::StartsWith { value: text_scalar(r) },
        9 => CondOp::EndsWith { value: text_scalar(r) },
        10 => CondOp::IsNull,
        11 => CondOp::IsNotNull,
        12 => CondOp::IsBlank,
        13 => CondOp::IsNotBlank,
        14 => CondOp::In {
            values: (0..r.below(4)).map(|_| scalar(r, true)).collect(),
        },
        _ => CondOp::NotIn {
            values: (0..r.below(4)).map(|_| scalar(r, true)).collect(),
        },
    };
    Cond::cmp(c, op)
}

fn is_blank_shape(a: &Cond, b: &Cond) -> bool {
    match (a, b) {
        (
            Cond::Compare(CompareCond { column: c1, op: o1 }),
            Cond::Compare(CompareCond { column: c2, op: o2 }),
        ) if c1 == c2 => matches!(
            (o1, o2),
            (CondOp::IsNull, CondOp::Eq { .. })
                | (CondOp::IsNotNull, CondOp::Neq { .. })
        ),
        _ => false,
    }
}

fn cond(r: &mut Rng, depth: usize) -> Cond {
    if depth == 0 || r.below(3) == 0 {
        return leaf(r);
    }
    match r.below(3) {
        0 | 1 => {
            let mut kids: Vec<Cond> = (0..2 + r.below(2)).map(|_| cond(r, depth - 1)).collect();
            if kids.len() == 2 && is_blank_shape(&kids[0], &kids[1]) {
                kids.push(leaf(r));
            }
            if r.bool() {
                Cond::And { and: kids }
            } else {
                Cond::Or { or: kids }
            }
        }
        _ => Cond::Not {
            not: Box::new(cond(r, depth - 1)),
        },
    }
}

fn voc(r: &mut Rng) -> ValueOrColumn {
    if r.bool() {
        ValueOrColumn::Column { column: name(r) }
    } else {
        ValueOrColumn::Value(scalar(r, true))
    }
}

fn ety(r: &mut Rng) -> EtlType {
    *r.pick(&EtlType::ALL)
}

const CUSTOM_EXPRS: &[&str] = &[
    "[Valor] + 1",
    "Text.Upper([Nome do Mês])",
    "Date.Year([Data])",
    "[#\"a\"\"b\"] & \"-\" & Text.From([Valor])",
    "Number.Round([Valor] * 1.1, 2)",
    "Date.MonthName([Data], \"pt-BR\")",
    "Text.Start([Nome do Mês], 3)",
    "[Valor] / (1 + [Valor])",
];

const AGG_OPS: &[AggOp] = &[
    AggOp::Sum,
    AggOp::Avg,
    AggOp::Min,
    AggOp::Max,
    AggOp::Count,
    AggOp::CountDistinct,
    AggOp::CountRows,
    AggOp::Median,
];

fn agg(r: &mut Rng) -> Agg {
    let op = *r.pick(AGG_OPS);
    let column = match op {
        AggOp::CountRows => None,
        AggOp::CountDistinct => r.bool().then(|| name(r)),
        _ => Some(name(r)),
    };
    Agg {
        name: name(r),
        op,
        column,
    }
}

/// All variants, indexed so the test can enumerate each one explicitly.
pub(super) const VARIANT_COUNT: usize = 34;

fn step_kind(r: &mut Rng, variant: usize) -> StepKind {
    match variant {
        0 => StepKind::PromoteHeaders,
        1 => StepKind::ChangeType {
            columns: (0..1 + r.below(3))
                .map(|_| ColumnTypeSpec {
                    column: name(r),
                    ty: ety(r),
                })
                .collect(),
            culture: if r.bool() { Some(r.pick(&["pt-BR", "it-IT", "en-US"]).to_string()) } else { None },
        },
        2 => StepKind::RenameColumns {
            renames: (0..1 + r.below(3)).map(|_| (name(r), name(r))).collect(),
        },
        3 => StepKind::RemoveColumns { columns: names(r, 1) },
        4 => StepKind::SelectColumns { columns: names(r, 1) },
        5 => StepKind::DuplicateColumn {
            column: name(r),
            new_column: name(r),
        },
        6 => StepKind::FilterRows { condition: cond(r, 3) },
        7 => StepKind::RemoveBlankRows,
        8 => StepKind::RemoveRowsWithErrors {
            columns: if r.bool() { Some(names(r, 0)) } else { None },
        },
        9 => StepKind::KeepTopRows { count: r.below(1000) as u64 },
        10 => StepKind::RemoveTopRows { count: r.below(1000) as u64 },
        11 => StepKind::KeepBottomRows { count: r.below(1000) as u64 },
        12 => StepKind::RemoveBottomRows { count: r.below(1000) as u64 },
        13 => StepKind::RemoveDuplicates {
            columns: if r.bool() { Some(names(r, 1)) } else { None },
        },
        14 => StepKind::Sort {
            by: (0..1 + r.below(3))
                .map(|_| SortKey {
                    column: name(r),
                    descending: r.bool(),
                })
                .collect(),
        },
        15 => StepKind::ReplaceValues {
            columns: names(r, 1),
            find: scalar(r, true),
            replace: scalar(r, true),
            match_entire_cell: r.bool(),
        },
        16 => StepKind::FillDown { columns: names(r, 1) },
        17 => StepKind::FillUp { columns: names(r, 1) },
        18 => {
            let mode = *r.pick(&[
                SplitMode::DelimiterEach,
                SplitMode::DelimiterLeftmost,
                SplitMode::DelimiterRightmost,
                SplitMode::Positions,
                SplitMode::FixedLength,
            ]);
            let (delimiter, positions, length) = match mode {
                SplitMode::Positions => (None, Some(vec![0, 3, 7]), None),
                SplitMode::FixedLength => (None, None, Some(1 + r.below(5) as u64)),
                _ => (Some(r.pick(&[",", ";", " - ", "\"", "#("]).to_string()), None, None),
            };
            StepKind::SplitColumn {
                column: name(r),
                mode,
                delimiter,
                positions,
                length,
                into: names(r, 1),
            }
        }
        19 => StepKind::MergeColumns {
            columns: names(r, 1),
            separator: r.pick(&["/", "", " - ", "\""]).to_string(),
            new_column: name(r),
        },
        20 => StepKind::TextTransform {
            columns: names(r, 1),
            func: *r.pick(&[TextFn::Upper, TextFn::Lower, TextFn::Proper, TextFn::Trim, TextFn::Clean]),
        },
        21 => StepKind::UnpivotOtherColumns {
            keep: names(r, 0),
            attribute_column: name(r),
            value_column: name(r),
        },
        22 => StepKind::UnpivotColumns {
            columns: names(r, 1),
            attribute_column: name(r),
            value_column: name(r),
        },
        23 => StepKind::Pivot {
            column: name(r),
            values_column: name(r),
            aggregation: *r.pick(AGG_OPS),
        },
        24 => StepKind::GroupBy {
            keys: names(r, 0),
            aggregations: (0..1 + r.below(3)).map(|_| agg(r)).collect(),
        },
        25 => StepKind::AddConditionalColumn {
            new_column: name(r),
            rules: (0..1 + r.below(3))
                .map(|_| ConditionalRule {
                    when: cond(r, 2),
                    then: voc(r),
                })
                .collect(),
            otherwise: voc(r),
            ty: if r.bool() { Some(ety(r)) } else { None },
        },
        26 => StepKind::AddCustomColumn {
            new_column: name(r),
            expression: r.pick(CUSTOM_EXPRS).to_string(),
            ty: if r.bool() { Some(ety(r)) } else { None },
        },
        27 => StepKind::AddIndexColumn {
            new_column: name(r),
            start: r.below(5) as i64 - 2,
            step: r.below(5) as i64 - 2,
        },
        28 => {
            let k = 1 + r.below(2);
            StepKind::MergeQueries {
                right_query: r.pick(&["dCategorias", "Other Query", "q\"x"]).to_string(),
                left_keys: (0..k).map(|_| name(r)).collect(),
                right_keys: (0..k).map(|_| name(r)).collect(),
                join_kind: *r.pick(&[
                    JoinKind::LeftOuter,
                    JoinKind::Inner,
                    JoinKind::RightOuter,
                    JoinKind::FullOuter,
                    JoinKind::LeftAnti,
                    JoinKind::RightAnti,
                ]),
                expand: (0..r.below(3))
                    .map(|_| ExpandColumn {
                        column: name(r),
                        as_name: name(r),
                    })
                    .collect(),
            }
        }
        29 => StepKind::AppendQueries {
            queries: (0..1 + r.below(2))
                .map(|_| r.pick(&["fAberturas", "Other Query", "q\"x"]).to_string())
                .collect(),
        },
        30 => StepKind::OpaqueM {
            m: r.pick(&[
                "Table.Buffer(Source)",
                "Table.TransformColumnNames(Source, each Text.Upper(_))",
                "Table.AddColumn(Source, \"X\", (r) => r[A])",
            ])
            .to_string(),
        },
        31 => StepKind::ChangeType {
            columns: vec![ColumnTypeSpec {
                column: name(r),
                ty: ety(r),
            }],
            culture: None,
        },
        32 => StepKind::FilterRows { condition: leaf(r) },
        _ => StepKind::RemoveRowsWithErrors { columns: None },
    }
}

fn source(r: &mut Rng) -> EtlSource {
    match r.below(10) {
        0 => EtlSource::excel(r.pick(&["C:\\Dados\\Fluxo de Caixa.xlsx", "/home/u/a \"b\".xlsx"]).to_string(), name(r)),
        1 => EtlSource::csv("C:\\x\\vendas.csv"),
        2 => {
            let mut s = EtlSource::empty(SourceKind::Parquet);
            s.path = Some("/data/f.parquet".into());
            s
        }
        3 => {
            let mut s = EtlSource::empty(SourceKind::Json);
            s.path = Some("/data/f.json".into());
            s
        }
        4 => EtlSource::table_ref(r.pick(&["dCategorias", "Other Query", "q\"x"]).to_string()),
        5 => {
            let cols = names(r, 1);
            let rows = (0..r.below(3)).map(|_| cols.iter().map(|_| scalar(r, true)).collect()).collect();
            EtlSource::inline(cols, rows)
        }
        6 => {
            let mut s = EtlSource::empty(SourceKind::ListDates);
            s.start = Some("2026-04-01".into());
            s.count = Some(275);
            s.step_days = Some(1);
            s.function = if r.bool() { Some("Query1".into()) } else { None };
            s
        }
        7 => {
            let mut s = EtlSource::empty(SourceKind::Db);
            s.table = Some("pg.public.orders".into());
            s
        }
        8 => EtlSource::opaque("Sql.Database(\"srv\", \"db\")"),
        _ => EtlSource::excel("C:\\Dados\\x.xlsx", "Fato"),
    }
}

#[track_caller]
fn assert_round_trip(q: &EtlQuery) {
    let m = query_to_m(q).unwrap_or_else(|e| panic!("to_m falhou: {}\n{:?}", e, q));
    let back = query_from_m(&q.name, &m).unwrap_or_else(|e| panic!("from_m falhou: {}\n{}", e, m));
    assert_eq!(
        back.query.without_ids(),
        q.without_ids(),
        "round-trip divergiu\nM:\n{}\nwarnings: {:?}",
        m,
        back.warnings
    );
}

fn unique_step_names(steps: &mut [EtlStep]) {
    for (i, s) in steps.iter_mut().enumerate() {
        s.name = format!("{} {}", s.kind.default_step_name(), i + 1);
    }
}

#[test]
fn round_trip_every_step_variant() {
    let mut r = Rng(0x9E3779B97F4A7C15);
    for variant in 0..VARIANT_COUNT {
        for _ in 0..60 {
            let kind = step_kind(&mut r, variant);
            let q = EtlQuery {
                name: "q".into(),
                source: EtlSource::excel("C:\\Dados\\x.xlsx", "Fato"),
                steps: vec![EtlStep::new("id-1", "Etapa", kind)],
            };
            assert_round_trip(&q);
        }
    }
}

#[test]
fn round_trip_every_source_kind_and_long_chains() {
    let mut r = Rng(42);
    for _ in 0..400 {
        let mut steps: Vec<EtlStep> = (0..r.below(8))
            .map(|i| {
                let v = r.below(VARIANT_COUNT);
                EtlStep::new(format!("id{}", i), "", step_kind(&mut r, v))
            })
            .collect();
        unique_step_names(&mut steps);
        let q = EtlQuery {
            name: "Consulta".into(),
            source: source(&mut r),
            steps,
        };
        assert_round_trip(&q);
    }
}

#[test]
fn round_trip_names_colliding_with_source_bindings() {
    let q = EtlQuery {
        name: "q".into(),
        source: EtlSource::excel("x.xlsx", "Fato"),
        steps: vec![
            EtlStep::new("", "Source", StepKind::PromoteHeaders),
            EtlStep::new("", "Fato_Sheet", StepKind::RemoveBlankRows),
        ],
    };
    assert_round_trip(&q);
}

#[test]
fn normalizations_are_semantic_no_ops() {
    // eq null → isNull; or[isNull, eq ""] → isBlank
    let q = EtlQuery {
        name: "q".into(),
        source: EtlSource::csv("a.csv"),
        steps: vec![EtlStep::new(
            "",
            "F",
            StepKind::FilterRows {
                condition: Cond::Or {
                    or: vec![
                        Cond::cmp("A", CondOp::Eq { value: Scalar::Null }),
                        Cond::cmp("A", CondOp::Eq { value: Scalar::text("") }),
                    ],
                },
            },
        )],
    };
    let back = query_from_m("q", &query_to_m(&q).unwrap()).unwrap().query;
    assert_eq!(
        back.steps[0].kind,
        StepKind::FilterRows {
            condition: Cond::cmp("A", CondOp::IsBlank)
        }
    );
}
