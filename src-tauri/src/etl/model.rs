//! Serde types of the ETL contract (`docs/ETL_V09.md`, mirrored by
//! `src/types/etl.ts`). JSON shape: steps are internally tagged by `"op"`,
//! every field is camelCase.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value as JsonValue;

/// Column type vocabulary shared with the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EtlType {
    Text,
    Integer,
    Decimal,
    Currency,
    Date,
    Datetime,
    Boolean,
}

impl EtlType {
    pub const ALL: [EtlType; 7] = [
        EtlType::Text,
        EtlType::Integer,
        EtlType::Decimal,
        EtlType::Currency,
        EtlType::Date,
        EtlType::Datetime,
        EtlType::Boolean,
    ];
}

/// `string | number | boolean | null`.
#[derive(Debug, Clone, PartialEq)]
pub enum Scalar {
    Null,
    Bool(bool),
    Number(f64),
    Text(String),
}

impl Scalar {
    pub fn text(s: impl Into<String>) -> Self {
        Scalar::Text(s.into())
    }
    pub fn is_null(&self) -> bool {
        matches!(self, Scalar::Null)
    }
    pub fn to_json(&self) -> JsonValue {
        match self {
            Scalar::Null => JsonValue::Null,
            Scalar::Bool(b) => JsonValue::Bool(*b),
            Scalar::Number(n) => number_to_json(*n),
            Scalar::Text(s) => JsonValue::String(s.clone()),
        }
    }
    pub fn from_json(v: &JsonValue) -> Result<Self, String> {
        Ok(match v {
            JsonValue::Null => Scalar::Null,
            JsonValue::Bool(b) => Scalar::Bool(*b),
            JsonValue::Number(n) => Scalar::Number(
                n.as_f64()
                    .ok_or_else(|| format!("número fora do intervalo: {}", n))?,
            ),
            JsonValue::String(s) => Scalar::Text(s.clone()),
            other => return Err(format!("valor escalar inválido: {}", other)),
        })
    }
}

pub(crate) fn number_to_json(n: f64) -> JsonValue {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 9.0e15 {
        JsonValue::from(n as i64)
    } else {
        serde_json::Number::from_f64(n)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null)
    }
}

impl Serialize for Scalar {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.to_json().serialize(s)
    }
}

impl<'de> Deserialize<'de> for Scalar {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = JsonValue::deserialize(d)?;
        Scalar::from_json(&v).map_err(serde::de::Error::custom)
    }
}

/// Leaf comparison operator of a [`Cond`] (tag `"op"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum CondOp {
    Eq { value: Scalar },
    Neq { value: Scalar },
    Gt { value: Scalar },
    Gte { value: Scalar },
    Lt { value: Scalar },
    Lte { value: Scalar },
    Contains { value: Scalar },
    NotContains { value: Scalar },
    StartsWith { value: Scalar },
    EndsWith { value: Scalar },
    IsNull,
    IsNotNull,
    IsBlank,
    IsNotBlank,
    In { values: Vec<Scalar> },
    NotIn { values: Vec<Scalar> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompareCond {
    pub column: String,
    #[serde(flatten)]
    pub op: CondOp,
}

/// Row predicate. Untagged: `{column, op, ...}` | `{and}` | `{or}` | `{not}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Cond {
    And { and: Vec<Cond> },
    Or { or: Vec<Cond> },
    Not { not: Box<Cond> },
    Compare(CompareCond),
}

impl Cond {
    pub fn cmp(column: impl Into<String>, op: CondOp) -> Self {
        Cond::Compare(CompareCond {
            column: column.into(),
            op,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AggOp {
    Sum,
    Avg,
    Min,
    Max,
    Count,
    CountDistinct,
    CountRows,
    Median,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agg {
    pub name: String,
    pub op: AggOp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnTypeSpec {
    pub column: String,
    #[serde(rename = "type")]
    pub ty: EtlType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SortKey {
    pub column: String,
    pub descending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SplitMode {
    DelimiterEach,
    DelimiterLeftmost,
    DelimiterRightmost,
    Positions,
    FixedLength,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextFn {
    Upper,
    Lower,
    Proper,
    Trim,
    Clean,
}

/// `Scalar | { column }` used by conditional-column branches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ValueOrColumn {
    Column { column: String },
    Value(Scalar),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConditionalRule {
    pub when: Cond,
    pub then: ValueOrColumn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JoinKind {
    LeftOuter,
    Inner,
    RightOuter,
    FullOuter,
    LeftAnti,
    RightAnti,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpandColumn {
    pub column: String,
    #[serde(rename = "as")]
    pub as_name: String,
}

/// Step payload, internally tagged by `"op"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum StepKind {
    PromoteHeaders,
    ChangeType {
        columns: Vec<ColumnTypeSpec>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        culture: Option<String>,
    },
    RenameColumns {
        renames: Vec<(String, String)>,
    },
    RemoveColumns {
        columns: Vec<String>,
    },
    SelectColumns {
        columns: Vec<String>,
    },
    DuplicateColumn {
        column: String,
        new_column: String,
    },
    FilterRows {
        condition: Cond,
    },
    RemoveBlankRows,
    RemoveRowsWithErrors {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        columns: Option<Vec<String>>,
    },
    KeepTopRows {
        count: u64,
    },
    RemoveTopRows {
        count: u64,
    },
    KeepBottomRows {
        count: u64,
    },
    RemoveBottomRows {
        count: u64,
    },
    RemoveDuplicates {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        columns: Option<Vec<String>>,
    },
    Sort {
        by: Vec<SortKey>,
    },
    ReplaceValues {
        columns: Vec<String>,
        find: Scalar,
        replace: Scalar,
        match_entire_cell: bool,
    },
    FillDown {
        columns: Vec<String>,
    },
    FillUp {
        columns: Vec<String>,
    },
    SplitColumn {
        column: String,
        mode: SplitMode,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        delimiter: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        positions: Option<Vec<u64>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        length: Option<u64>,
        into: Vec<String>,
    },
    MergeColumns {
        columns: Vec<String>,
        separator: String,
        new_column: String,
    },
    TextTransform {
        columns: Vec<String>,
        #[serde(rename = "fn")]
        func: TextFn,
    },
    UnpivotOtherColumns {
        keep: Vec<String>,
        attribute_column: String,
        value_column: String,
    },
    UnpivotColumns {
        columns: Vec<String>,
        attribute_column: String,
        value_column: String,
    },
    Pivot {
        column: String,
        values_column: String,
        aggregation: AggOp,
    },
    GroupBy {
        keys: Vec<String>,
        aggregations: Vec<Agg>,
    },
    AddConditionalColumn {
        new_column: String,
        rules: Vec<ConditionalRule>,
        otherwise: ValueOrColumn,
        #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
        ty: Option<EtlType>,
    },
    AddCustomColumn {
        new_column: String,
        expression: String,
        #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
        ty: Option<EtlType>,
    },
    AddIndexColumn {
        new_column: String,
        start: i64,
        step: i64,
    },
    MergeQueries {
        right_query: String,
        left_keys: Vec<String>,
        right_keys: Vec<String>,
        join_kind: JoinKind,
        expand: Vec<ExpandColumn>,
    },
    AppendQueries {
        queries: Vec<String>,
    },
    OpaqueM {
        m: String,
    },
    // ---- Rodada 2 (docs/ETL_V09.md §"Rodada 2") ----
    TransformColumns {
        columns: Vec<String>,
        expression: String,
        #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
        ty: Option<EtlType>,
    },
    ReorderColumns {
        columns: Vec<String>,
    },
    DemoteHeaders,
    Transpose,
    ReverseRows,
    KeepRange {
        offset: u64,
        count: u64,
    },
    RemoveAlternateRows {
        offset: u64,
        remove: u64,
        keep: u64,
    },
    KeepDuplicates {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        columns: Option<Vec<String>>,
    },
    ReplaceErrors {
        replacements: Vec<ReplaceErrorSpec>,
    },
    KeepRowsWithErrors {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        columns: Option<Vec<String>>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceErrorSpec {
    pub column: String,
    pub value: Scalar,
}

impl StepKind {
    /// Contract name of the variant (value of `"op"`).
    pub fn op_name(&self) -> &'static str {
        match self {
            StepKind::PromoteHeaders => "promoteHeaders",
            StepKind::ChangeType { .. } => "changeType",
            StepKind::RenameColumns { .. } => "renameColumns",
            StepKind::RemoveColumns { .. } => "removeColumns",
            StepKind::SelectColumns { .. } => "selectColumns",
            StepKind::DuplicateColumn { .. } => "duplicateColumn",
            StepKind::FilterRows { .. } => "filterRows",
            StepKind::RemoveBlankRows => "removeBlankRows",
            StepKind::RemoveRowsWithErrors { .. } => "removeRowsWithErrors",
            StepKind::KeepTopRows { .. } => "keepTopRows",
            StepKind::RemoveTopRows { .. } => "removeTopRows",
            StepKind::KeepBottomRows { .. } => "keepBottomRows",
            StepKind::RemoveBottomRows { .. } => "removeBottomRows",
            StepKind::RemoveDuplicates { .. } => "removeDuplicates",
            StepKind::Sort { .. } => "sort",
            StepKind::ReplaceValues { .. } => "replaceValues",
            StepKind::FillDown { .. } => "fillDown",
            StepKind::FillUp { .. } => "fillUp",
            StepKind::SplitColumn { .. } => "splitColumn",
            StepKind::MergeColumns { .. } => "mergeColumns",
            StepKind::TextTransform { .. } => "textTransform",
            StepKind::UnpivotOtherColumns { .. } => "unpivotOtherColumns",
            StepKind::UnpivotColumns { .. } => "unpivotColumns",
            StepKind::Pivot { .. } => "pivot",
            StepKind::GroupBy { .. } => "groupBy",
            StepKind::AddConditionalColumn { .. } => "addConditionalColumn",
            StepKind::AddCustomColumn { .. } => "addCustomColumn",
            StepKind::AddIndexColumn { .. } => "addIndexColumn",
            StepKind::MergeQueries { .. } => "mergeQueries",
            StepKind::AppendQueries { .. } => "appendQueries",
            StepKind::OpaqueM { .. } => "opaqueM",
            StepKind::TransformColumns { .. } => "transformColumns",
            StepKind::ReorderColumns { .. } => "reorderColumns",
            StepKind::DemoteHeaders => "demoteHeaders",
            StepKind::Transpose => "transpose",
            StepKind::ReverseRows => "reverseRows",
            StepKind::KeepRange { .. } => "keepRange",
            StepKind::RemoveAlternateRows { .. } => "removeAlternateRows",
            StepKind::KeepDuplicates { .. } => "keepDuplicates",
            StepKind::ReplaceErrors { .. } => "replaceErrors",
            StepKind::KeepRowsWithErrors { .. } => "keepRowsWithErrors",
        }
    }

    /// Default Power Query step name (English UI), used when a step has no name.
    pub fn default_step_name(&self) -> &'static str {
        match self {
            StepKind::PromoteHeaders => "Promoted Headers",
            StepKind::ChangeType { .. } => "Changed Type",
            StepKind::RenameColumns { .. } => "Renamed Columns",
            StepKind::RemoveColumns { .. } => "Removed Columns",
            StepKind::SelectColumns { .. } => "Removed Other Columns",
            StepKind::DuplicateColumn { .. } => "Duplicated Column",
            StepKind::FilterRows { .. } => "Filtered Rows",
            StepKind::RemoveBlankRows => "Removed Blank Rows",
            StepKind::RemoveRowsWithErrors { .. } => "Removed Errors",
            StepKind::KeepTopRows { .. } => "Kept First Rows",
            StepKind::RemoveTopRows { .. } => "Removed Top Rows",
            StepKind::KeepBottomRows { .. } => "Kept Last Rows",
            StepKind::RemoveBottomRows { .. } => "Removed Bottom Rows",
            StepKind::RemoveDuplicates { .. } => "Removed Duplicates",
            StepKind::Sort { .. } => "Sorted Rows",
            StepKind::ReplaceValues { .. } => "Replaced Value",
            StepKind::FillDown { .. } => "Filled Down",
            StepKind::FillUp { .. } => "Filled Up",
            StepKind::SplitColumn { .. } => "Split Column by Delimiter",
            StepKind::MergeColumns { .. } => "Merged Columns",
            StepKind::TextTransform { .. } => "Transformed Text",
            StepKind::UnpivotOtherColumns { .. } => "Unpivoted Other Columns",
            StepKind::UnpivotColumns { .. } => "Unpivoted Columns",
            StepKind::Pivot { .. } => "Pivoted Column",
            StepKind::GroupBy { .. } => "Grouped Rows",
            StepKind::AddConditionalColumn { .. } => "Added Conditional Column",
            StepKind::AddCustomColumn { .. } => "Added Custom",
            StepKind::AddIndexColumn { .. } => "Added Index",
            StepKind::MergeQueries { .. } => "Merged Queries",
            StepKind::AppendQueries { .. } => "Appended Query",
            StepKind::OpaqueM { .. } => "Custom Step",
            StepKind::TransformColumns { .. } => "Transformed Column",
            StepKind::ReorderColumns { .. } => "Reordered Columns",
            StepKind::DemoteHeaders => "Demoted Headers",
            StepKind::Transpose => "Transposed Table",
            StepKind::ReverseRows => "Reversed Rows",
            StepKind::KeepRange { .. } => "Kept Range of Rows",
            StepKind::RemoveAlternateRows { .. } => "Removed Alternate Rows",
            StepKind::KeepDuplicates { .. } => "Kept Duplicates",
            StepKind::ReplaceErrors { .. } => "Replaced Errors",
            StepKind::KeepRowsWithErrors { .. } => "Kept Rows With Errors",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EtlStep {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(flatten)]
    pub kind: StepKind,
}

impl EtlStep {
    pub fn new(id: impl Into<String>, name: impl Into<String>, kind: StepKind) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    Excel,
    Csv,
    Parquet,
    Json,
    Db,
    Table,
    /// Deviation from the v0.9 draft: literal `#table(...)` (e.g. a measures host table).
    Inline,
    /// Deviation from the v0.9 draft: `List.Dates(start, count, #duration(d,0,0,0))`
    /// (directly or through a user function with the same signature) followed by
    /// `Table.FromList(.., Splitter.SplitByNothing(), ..)` → one column `Column1` of dates.
    ListDates,
    OpaqueM,
}

/// Where `raw__<query>` comes from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EtlSource {
    pub kind: SourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheet: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub m: Option<String>,
    /// `inline`: column names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<String>>,
    /// `inline`: row values (each row has `columns.len()` cells).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<Vec<Vec<Scalar>>>,
    /// `listDates`: first date, ISO `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    /// `listDates`: number of dates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    /// `listDates`: whole days between dates (`#duration(stepDays, 0, 0, 0)`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_days: Option<i64>,
    /// `listDates`: name of the M function called (omitted = `List.Dates`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
}

impl EtlSource {
    pub fn empty(kind: SourceKind) -> Self {
        Self {
            kind,
            path: None,
            sheet: None,
            table: None,
            m: None,
            columns: None,
            rows: None,
            start: None,
            count: None,
            step_days: None,
            function: None,
        }
    }
    pub fn excel(path: impl Into<String>, sheet: impl Into<String>) -> Self {
        let mut s = Self::empty(SourceKind::Excel);
        s.path = Some(path.into());
        s.sheet = Some(sheet.into());
        s
    }
    pub fn csv(path: impl Into<String>) -> Self {
        let mut s = Self::empty(SourceKind::Csv);
        s.path = Some(path.into());
        s
    }
    pub fn table_ref(table: impl Into<String>) -> Self {
        let mut s = Self::empty(SourceKind::Table);
        s.table = Some(table.into());
        s
    }
    pub fn inline(columns: Vec<String>, rows: Vec<Vec<Scalar>>) -> Self {
        let mut s = Self::empty(SourceKind::Inline);
        s.columns = Some(columns);
        s.rows = Some(rows);
        s
    }
    pub fn opaque(m: impl Into<String>) -> Self {
        let mut s = Self::empty(SourceKind::OpaqueM);
        s.m = Some(m.into());
        s
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EtlQuery {
    pub name: String,
    pub source: EtlSource,
    #[serde(default)]
    pub steps: Vec<EtlStep>,
}

impl EtlQuery {
    /// Copy with every step id blanked (round-trip comparisons ignore ids).
    pub fn without_ids(&self) -> EtlQuery {
        let mut q = self.clone();
        for s in &mut q.steps {
            s.id.clear();
        }
        q
    }

    /// Names of other queries this one reads (source `table`, merges, appends).
    pub fn referenced_queries(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.source.kind == SourceKind::Table {
            if let Some(t) = &self.source.table {
                out.push(t.clone());
            }
        }
        for s in &self.steps {
            match &s.kind {
                StepKind::MergeQueries { right_query, .. } => out.push(right_query.clone()),
                StepKind::AppendQueries { queries } => out.extend(queries.iter().cloned()),
                _ => {}
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn step_json_shape_matches_contract() {
        let step = EtlStep::new(
            "s1",
            "Changed Type",
            StepKind::ChangeType {
                columns: vec![ColumnTypeSpec {
                    column: "Valor".into(),
                    ty: EtlType::Currency,
                }],
                culture: Some("pt-BR".into()),
            },
        );
        let v = serde_json::to_value(&step).unwrap();
        assert_eq!(
            v,
            json!({"id": "s1", "name": "Changed Type", "op": "changeType",
                   "columns": [{"column": "Valor", "type": "currency"}], "culture": "pt-BR"})
        );
        let back: EtlStep = serde_json::from_value(v).unwrap();
        assert_eq!(back, step);

        let split: EtlStep = serde_json::from_value(json!({
            "id": "x", "name": "Split", "op": "splitColumn", "column": "A",
            "mode": "delimiterEach", "delimiter": ",", "into": ["A.1", "A.2"]
        }))
        .unwrap();
        assert!(matches!(split.kind, StepKind::SplitColumn { mode: SplitMode::DelimiterEach, .. }));

        let tt = serde_json::to_value(StepKind::TextTransform {
            columns: vec!["A".into()],
            func: TextFn::Proper,
        })
        .unwrap();
        assert_eq!(tt, json!({"op": "textTransform", "columns": ["A"], "fn": "proper"}));

        let mq = serde_json::to_value(StepKind::MergeQueries {
            right_query: "dCat".into(),
            left_keys: vec!["k".into()],
            right_keys: vec!["k".into()],
            join_kind: JoinKind::LeftOuter,
            expand: vec![ExpandColumn {
                column: "G".into(),
                as_name: "dCat.G".into(),
            }],
        })
        .unwrap();
        assert_eq!(
            mq,
            json!({"op": "mergeQueries", "rightQuery": "dCat", "leftKeys": ["k"], "rightKeys": ["k"],
                   "joinKind": "leftOuter", "expand": [{"column": "G", "as": "dCat.G"}]})
        );
    }

    #[test]
    fn cond_json_shapes() {
        let c: Cond = serde_json::from_value(json!({
            "and": [
                {"column": "A", "op": "gt", "value": 5},
                {"not": {"column": "B", "op": "isNull"}},
                {"or": [{"column": "C", "op": "in", "values": ["x", null, 1.5]}]}
            ]
        }))
        .unwrap();
        let Cond::And { and } = &c else { panic!("and") };
        assert_eq!(
            and[0],
            Cond::cmp("A", CondOp::Gt { value: Scalar::Number(5.0) })
        );
        assert_eq!(and[1], Cond::Not { not: Box::new(Cond::cmp("B", CondOp::IsNull)) });
        let back = serde_json::to_value(&c).unwrap();
        assert_eq!(back["and"][0], json!({"column": "A", "op": "gt", "value": 5}));
        assert_eq!(back["and"][2]["or"][0]["values"], json!(["x", null, 1.5]));
    }

    #[test]
    fn conditional_branch_value_or_column() {
        let r: ConditionalRule = serde_json::from_value(json!({
            "when": {"column": "M", "op": "lte", "value": 6}, "then": {"column": "X"}
        }))
        .unwrap();
        assert_eq!(r.then, ValueOrColumn::Column { column: "X".into() });
        let r2: ConditionalRule = serde_json::from_value(json!({
            "when": {"column": "M", "op": "lte", "value": 6}, "then": "S1"
        }))
        .unwrap();
        assert_eq!(r2.then, ValueOrColumn::Value(Scalar::text("S1")));
    }

    #[test]
    fn query_json_round_trip() {
        let q = EtlQuery {
            name: "dCategorias".into(),
            source: EtlSource::excel("C:\\Dados\\x.xlsx", "Categorias"),
            steps: vec![
                EtlStep::new("1", "Promoted Headers", StepKind::PromoteHeaders),
                EtlStep::new("2", "Erros", StepKind::RemoveRowsWithErrors { columns: None }),
            ],
        };
        let v = serde_json::to_value(&q).unwrap();
        assert_eq!(v["source"], json!({"kind": "excel", "path": "C:\\Dados\\x.xlsx", "sheet": "Categorias"}));
        assert_eq!(v["steps"][0], json!({"id": "1", "name": "Promoted Headers", "op": "promoteHeaders"}));
        assert_eq!(v["steps"][1], json!({"id": "2", "name": "Erros", "op": "removeRowsWithErrors"}));
        let back: EtlQuery = serde_json::from_value(v).unwrap();
        assert_eq!(back, q);
    }
}
