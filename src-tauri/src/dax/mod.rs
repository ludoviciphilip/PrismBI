pub mod ast;
pub mod evaluator;
pub mod lexer;
pub mod parser;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq)]
pub enum DaxValue {
    Blank,
    Number(f64),
    Text(String),
    Boolean(bool),
}

impl DaxValue {
    pub fn is_blank(&self) -> bool {
        matches!(self, DaxValue::Blank)
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            DaxValue::Blank => Some(0.0),
            DaxValue::Number(n) => Some(*n),
            DaxValue::Boolean(b) => Some(if *b { 1.0 } else { 0.0 }),
            DaxValue::Text(s) => s.trim().replace(',', ".").parse::<f64>().ok(),
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            DaxValue::Blank => false,
            DaxValue::Boolean(b) => *b,
            DaxValue::Number(n) => *n != 0.0,
            DaxValue::Text(s) => {
                let l = s.trim().to_lowercase();
                !l.is_empty() && l != "false" && l != "0" && l != "não" && l != "nao"
            }
        }
    }

    pub fn as_display_string(&self) -> String {
        match self {
            DaxValue::Blank => String::new(),
            DaxValue::Number(n) => {
                if (n.fract()).abs() < 1e-12 {
                    format!("{}", *n as i64)
                } else {
                    n.to_string()
                }
            }
            DaxValue::Text(s) => s.clone(),
            DaxValue::Boolean(b) => b.to_string(),
        }
    }

    pub fn canonical_key(&self) -> String {
        match self {
            DaxValue::Blank => "__BLANK__".to_string(),
            DaxValue::Number(n) => {
                if (n.fract()).abs() < 1e-9 {
                    format!("n:{}", n.round() as i64)
                } else {
                    format!("n:{:.6}", n)
                }
            }
            DaxValue::Boolean(b) => format!("b:{}", b),
            DaxValue::Text(s) => format!("t:{}", s.trim().to_lowercase()),
        }
    }

    pub fn dax_equals(&self, other: &DaxValue) -> bool {
        match (self, other) {
            (DaxValue::Blank, DaxValue::Blank) => true,
            (DaxValue::Blank, DaxValue::Number(n)) | (DaxValue::Number(n), DaxValue::Blank) => {
                *n == 0.0
            }
            (DaxValue::Blank, DaxValue::Text(s)) | (DaxValue::Text(s), DaxValue::Blank) => {
                s.is_empty()
            }
            (DaxValue::Blank, DaxValue::Boolean(b)) | (DaxValue::Boolean(b), DaxValue::Blank) => {
                !*b
            }
            (DaxValue::Number(a), DaxValue::Number(b)) => (a - b).abs() < 1e-9,
            (DaxValue::Boolean(a), DaxValue::Boolean(b)) => a == b,
            (DaxValue::Text(a), DaxValue::Text(b)) => a.trim().eq_ignore_ascii_case(b.trim()),
            _ => self.canonical_key() == other.canonical_key(),
        }
    }

    pub fn strict_equals(&self, other: &DaxValue) -> bool {
        match (self, other) {
            (DaxValue::Blank, DaxValue::Blank) => true,
            (DaxValue::Blank, _) | (_, DaxValue::Blank) => false,
            _ => self.dax_equals(other),
        }
    }

    pub fn to_json(&self) -> JsonValue {
        match self {
            DaxValue::Blank => JsonValue::Null,
            DaxValue::Number(n) => serde_json::Number::from_f64(*n)
                .map(JsonValue::Number)
                .unwrap_or(JsonValue::Null),
            DaxValue::Text(s) => JsonValue::String(s.clone()),
            DaxValue::Boolean(b) => JsonValue::Bool(*b),
        }
    }

    pub fn from_json(v: &JsonValue) -> Self {
        match v {
            JsonValue::Null => DaxValue::Blank,
            JsonValue::Bool(b) => DaxValue::Boolean(*b),
            JsonValue::Number(n) => DaxValue::Number(n.as_f64().unwrap_or(0.0)),
            JsonValue::String(s) => {
                if s.is_empty() {
                    DaxValue::Blank
                } else {
                    DaxValue::Text(s.clone())
                }
            }
            other => DaxValue::Text(other.to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DaxTableData {
    pub name: String,
    pub columns: Vec<String>,
    pub col_index: HashMap<String, usize>,
    pub rows: Vec<Vec<DaxValue>>,
}

impl DaxTableData {
    pub fn from_json_rows(name: &str, columns: Vec<String>, raw_rows: &[serde_json::Map<String, JsonValue>]) -> Self {
        let mut col_index = HashMap::new();
        for (idx, c) in columns.iter().enumerate() {
            col_index.insert(c.to_lowercase(), idx);
        }
        let rows = raw_rows
            .iter()
            .map(|rmap| {
                columns
                    .iter()
                    .map(|c| rmap.get(c).map(DaxValue::from_json).unwrap_or(DaxValue::Blank))
                    .collect()
            })
            .collect();
        Self {
            name: name.to_string(),
            columns,
            col_index,
            rows,
        }
    }

    pub fn get_col_idx(&self, col_name: &str) -> Option<usize> {
        self.col_index.get(&col_name.to_lowercase()).copied()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaxRelationshipInput {
    pub from_table: String,
    pub from_column: String,
    pub to_table: String,
    pub to_column: String,
    pub cross_filter_direction: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaxMeasureInput {
    pub name: String,
    pub table_name: String,
    pub formula: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaxTableInput {
    pub name: String,
    pub columns: Vec<String>,
    pub rows: Vec<serde_json::Map<String, JsonValue>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaxModelSyncPayload {
    pub tables: Vec<DaxTableInput>,
    pub relationships: Vec<DaxRelationshipInput>,
    pub measures: Vec<DaxMeasureInput>,
}

#[derive(Debug, Clone, Default)]
pub struct FilterContext {
    /// table_lower -> col_lower -> allowed canonical keys
    pub filters: HashMap<String, HashMap<String, HashSet<String>>>,
}

impl FilterContext {
    pub fn set_column_filter(&mut self, table: &str, col: &str, allowed: HashSet<String>) {
        self.filters
            .entry(table.to_lowercase())
            .or_default()
            .insert(col.to_lowercase(), allowed);
    }

    pub fn clear_table(&mut self, table: &str) {
        self.filters.remove(&table.to_lowercase());
    }

    pub fn clear_column(&mut self, table: &str, col: &str) {
        if let Some(cols) = self.filters.get_mut(&table.to_lowercase()) {
            cols.remove(&col.to_lowercase());
        }
    }

    pub fn clear_except(&mut self, table: &str, keep_cols: &[String]) {
        let keep: HashSet<String> = keep_cols.iter().map(|s| s.to_lowercase()).collect();
        if let Some(cols) = self.filters.get_mut(&table.to_lowercase()) {
            cols.retain(|k, _| keep.contains(k));
        }
    }
}

pub struct DaxModel {
    pub tables: HashMap<String, DaxTableData>,
    pub relationships: Vec<DaxRelationshipInput>,
    pub measures: HashMap<String, DaxMeasureInput>,
}

impl Default for DaxModel {
    fn default() -> Self {
        Self::new()
    }
}

impl DaxModel {
    pub fn new() -> Self {
        Self {
            tables: HashMap::new(),
            relationships: Vec::new(),
            measures: HashMap::new(),
        }
    }

    pub fn sync_from_payload(&mut self, payload: DaxModelSyncPayload) {
        self.tables.clear();
        for t in payload.tables {
            let key = t.name.to_lowercase();
            self.tables
                .insert(key, DaxTableData::from_json_rows(&t.name, t.columns, &t.rows));
        }
        self.relationships = payload.relationships;
        self.measures.clear();
        for m in payload.measures {
            self.measures.insert(m.name.to_lowercase(), m);
        }
    }

    pub fn get_table(&self, name: &str) -> Option<&DaxTableData> {
        self.tables.get(&name.trim().to_lowercase())
    }

    pub fn get_measure(&self, name: &str) -> Option<&DaxMeasureInput> {
        self.measures.get(&name.trim().to_lowercase())
    }

    /// Computes visible row indices for `target_table` under `ctx` by propagating active relationships.
    pub fn get_visible_row_indices(&self, target_table: &str, ctx: &FilterContext) -> Vec<usize> {
        let Some(tbl) = self.get_table(target_table) else {
            return Vec::new();
        };
        if ctx.filters.is_empty() {
            return (0..tbl.rows.len()).collect();
        }

        // Start with direct column filters on every table
        let mut table_masks: HashMap<String, Vec<bool>> = HashMap::new();
        for (tkey, tdata) in &self.tables {
            let mut mask = vec![true; tdata.rows.len()];
            if let Some(col_filters) = ctx.filters.get(tkey) {
                for (ckey, allowed) in col_filters {
                    if let Some(cidx) = tdata.get_col_idx(ckey) {
                        for (r_idx, row) in tdata.rows.iter().enumerate() {
                            if mask[r_idx] && !allowed.contains(&row[cidx].canonical_key()) {
                                mask[r_idx] = false;
                            }
                        }
                    }
                }
            }
            table_masks.insert(tkey.clone(), mask);
        }

        // Propagate filters along relationships (1/to -> N/from always, and from -> to if bidirectional)
        let mut queue: VecDeque<String> = ctx.filters.keys().cloned().collect();
        let mut hops = 0usize;
        while let Some(src_key) = queue.pop_front() {
            if hops > 64 {
                break;
            }
            hops += 1;

            for rel in &self.relationships {
                if !rel.is_active {
                    continue;
                }
                let from_k = rel.from_table.to_lowercase();
                let to_k = rel.to_table.to_lowercase();

                // Dimension (to_table) -> Fact (from_table)
                if src_key == to_k {
                    if let (Some(to_tbl), Some(from_tbl)) =
                        (self.tables.get(&to_k), self.tables.get(&from_k))
                    {
                        if let (Some(to_col), Some(from_col)) = (
                            to_tbl.get_col_idx(&rel.to_column),
                            from_tbl.get_col_idx(&rel.from_column),
                        ) {
                            let to_mask = &table_masks[&to_k];
                            let allowed_keys: HashSet<String> = to_tbl
                                .rows
                                .iter()
                                .enumerate()
                                .filter(|(i, _)| to_mask[*i])
                                .map(|(_, r)| r[to_col].canonical_key())
                                .collect();

                            let from_mask = table_masks.get_mut(&from_k).unwrap();
                            let mut changed = false;
                            for (r_idx, row) in from_tbl.rows.iter().enumerate() {
                                if from_mask[r_idx] && !allowed_keys.contains(&row[from_col].canonical_key()) {
                                    from_mask[r_idx] = false;
                                    changed = true;
                                }
                            }
                            if changed {
                                queue.push_back(from_k.clone());
                            }
                        }
                    }
                }

                // Fact (from_table) -> Dimension (to_table) when crossFilterDirection == "both"
                if src_key == from_k && rel.cross_filter_direction.eq_ignore_ascii_case("both") {
                    if let (Some(from_tbl), Some(to_tbl)) =
                        (self.tables.get(&from_k), self.tables.get(&to_k))
                    {
                        if let (Some(from_col), Some(to_col)) = (
                            from_tbl.get_col_idx(&rel.from_column),
                            to_tbl.get_col_idx(&rel.to_column),
                        ) {
                            let from_mask = &table_masks[&from_k];
                            let allowed_keys: HashSet<String> = from_tbl
                                .rows
                                .iter()
                                .enumerate()
                                .filter(|(i, _)| from_mask[*i])
                                .map(|(_, r)| r[from_col].canonical_key())
                                .collect();

                            let to_mask = table_masks.get_mut(&to_k).unwrap();
                            let mut changed = false;
                            for (r_idx, row) in to_tbl.rows.iter().enumerate() {
                                if to_mask[r_idx] && !allowed_keys.contains(&row[to_col].canonical_key()) {
                                    to_mask[r_idx] = false;
                                    changed = true;
                                }
                            }
                            if changed {
                                queue.push_back(to_k);
                            }
                        }
                    }
                }
            }
        }

        let target_mask = &table_masks[&target_table.to_lowercase()];
        target_mask
            .iter()
            .enumerate()
            .filter_map(|(idx, &vis)| if vis { Some(idx) } else { None })
            .collect()
    }

    pub fn evaluate_dax(&self, expression: &str, ctx: &FilterContext) -> Result<DaxValue, String> {
        let ast = parser::parse_dax(expression)?;
        evaluator::evaluate_ast(&ast, self, ctx, &[], &HashMap::new())
    }
}
