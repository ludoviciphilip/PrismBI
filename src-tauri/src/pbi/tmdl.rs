use crate::pbi::{
    map_tmsl_datatype, PbiColumnMeta, PbiMeasureMeta, PbiPartitionMeta, PbiRelationshipMeta,
    PbiTableMeta,
};

/// Parses a TMDL table file (`definition/tables/*.tmdl`).
pub fn parse_table_tmdl(content: &str) -> Result<PbiTableMeta, String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut table_name = String::new();
    let mut is_hidden = false;
    let mut is_calculated_table = false;
    let mut columns = Vec::new();
    let mut measures = Vec::new();
    let mut partitions = Vec::new();

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("///") {
            i += 1;
            continue;
        }

        let indent = leading_indent(line);

        if indent == 0 && trimmed.starts_with("table ") {
            let rest = trimmed.trim_start_matches("table ").trim();
            table_name = unquote_tmdl_name(rest);
            i += 1;
            continue;
        }

        if trimmed == "isHidden" && indent <= 1 {
            is_hidden = true;
            i += 1;
            continue;
        }

        if let Some(after_col) = trimmed.strip_prefix("column ") {
            let col_indent = indent;
            let (raw_name, inline_expr) = split_tmdl_assignment(after_col);
            let name = unquote_tmdl_name(raw_name);
            let mut data_type = "text".to_string();
            let mut format_string = None;
            let mut sort_by_column = None;
            let mut col_hidden = false;
            let mut expression = inline_expr;

            i += 1;
            while i < lines.len() {
                let sub = lines[i];
                let sub_trim = sub.trim();
                if sub_trim.is_empty() {
                    i += 1;
                    continue;
                }
                let sub_indent = leading_indent(sub);
                if sub_indent <= col_indent {
                    break;
                }
                if let Some(dt) = sub_trim.strip_prefix("dataType:") {
                    data_type = map_tmsl_datatype(dt).to_string();
                } else if let Some(fs) = sub_trim.strip_prefix("formatString:") {
                    format_string = Some(fs.trim().trim_matches('"').to_string());
                } else if let Some(sbc) = sub_trim.strip_prefix("sortByColumn:") {
                    sort_by_column = Some(unquote_tmdl_name(sbc.trim()));
                } else if sub_trim == "isHidden" {
                    col_hidden = true;
                } else if let Some(expr_rest) = sub_trim.strip_prefix("expression:") {
                    expression = Some(expr_rest.trim().to_string());
                }
                i += 1;
            }

            let is_calc = expression.is_some();
            columns.push(PbiColumnMeta {
                name,
                data_type,
                format_string,
                sort_by_column,
                is_hidden: col_hidden,
                is_calculated: is_calc,
                expression,
            });
            continue;
        }

        if let Some(after_measure) = trimmed.strip_prefix("measure ") {
            let m_indent = indent;
            let (raw_name, inline_expr) = split_tmdl_assignment(after_measure);
            let name = unquote_tmdl_name(raw_name);
            let mut expr_lines: Vec<String> = Vec::new();
            let mut in_code_fence = false;

            if let Some(first) = inline_expr {
                let ftrim = first.trim();
                if ftrim == "```" {
                    in_code_fence = true;
                } else if !ftrim.is_empty() {
                    expr_lines.push(ftrim.to_string());
                }
            }

            let mut format_string = None;
            let mut display_folder = None;
            let mut description = None;

            i += 1;
            while i < lines.len() {
                let sub = lines[i];
                let sub_trim = sub.trim();
                if in_code_fence {
                    if sub_trim == "```" {
                        in_code_fence = false;
                    } else {
                        expr_lines.push(sub_trim.to_string());
                    }
                    i += 1;
                    continue;
                }

                if sub_trim.is_empty() {
                    i += 1;
                    continue;
                }

                let sub_indent = leading_indent(sub);
                if sub_indent <= m_indent {
                    break;
                }

                if let Some(fs) = sub_trim.strip_prefix("formatString:") {
                    format_string = Some(fs.trim().trim_matches('"').to_string());
                } else if let Some(df) = sub_trim.strip_prefix("displayFolder:") {
                    display_folder = Some(df.trim().trim_matches('"').to_string());
                } else if let Some(desc) = sub_trim.strip_prefix("description:") {
                    description = Some(desc.trim().trim_matches('"').to_string());
                } else if sub_trim.starts_with("lineageTag:")
                    || sub_trim.starts_with("annotation ")
                    || sub_trim.starts_with("changedProperty ")
                {
                    // metadata property
                } else if sub_trim == "```" {
                    in_code_fence = true;
                } else if !sub_trim.contains(':') || expr_lines.is_empty() || sub_indent >= m_indent + 2 {
                    expr_lines.push(sub_trim.to_string());
                }
                i += 1;
            }

            measures.push(PbiMeasureMeta {
                name,
                table_name: table_name.clone(),
                expression: expr_lines.join("\n").trim().to_string(),
                format_string,
                display_folder,
                description,
            });
            continue;
        }

        if let Some(after_part) = trimmed.strip_prefix("partition ") {
            let p_indent = indent;
            let (raw_name, rhs) = split_tmdl_assignment(after_part);
            let name = unquote_tmdl_name(raw_name);
            let source_type = rhs.unwrap_or_else(|| "m".to_string()).trim().to_string();
            if source_type.eq_ignore_ascii_case("calculated") {
                is_calculated_table = true;
            }

            let mut mode = "import".to_string();
            let mut expr_lines = Vec::new();
            let mut reading_source = false;

            i += 1;
            while i < lines.len() {
                let sub = lines[i];
                let sub_trim = sub.trim();
                if sub_trim.is_empty() {
                    if reading_source {
                        expr_lines.push(String::new());
                    }
                    i += 1;
                    continue;
                }
                let sub_indent = leading_indent(sub);
                if sub_indent <= p_indent {
                    break;
                }

                if let Some(m) = sub_trim.strip_prefix("mode:") {
                    mode = m.trim().to_string();
                    reading_source = false;
                } else if let Some(src_rest) = sub_trim.strip_prefix("source =") {
                    reading_source = true;
                    let r = src_rest.trim();
                    if !r.is_empty() && r != "```" {
                        expr_lines.push(r.to_string());
                    }
                } else if sub_trim.starts_with("annotation ") {
                    reading_source = false;
                } else if reading_source && sub_trim != "```" {
                    expr_lines.push(sub_trim.to_string());
                }
                i += 1;
            }

            partitions.push(PbiPartitionMeta {
                name,
                mode,
                source_type,
                expression: expr_lines.join("\n").trim().to_string(),
            });
            continue;
        }

        i += 1;
    }

    if table_name.is_empty() {
        return Err("Arquivo TMDL sem declaração 'table'.".to_string());
    }

    // Ensure measure table_name is populated even if table declaration was parsed cleanly
    for m in &mut measures {
        if m.table_name.is_empty() {
            m.table_name = table_name.clone();
        }
    }

    Ok(PbiTableMeta {
        name: table_name,
        columns,
        measures,
        partitions,
        is_calculated: is_calculated_table,
        is_hidden,
    })
}

/// Parses `definition/relationships.tmdl`.
pub fn parse_relationships_tmdl(content: &str) -> Vec<PbiRelationshipMeta> {
    let lines: Vec<&str> = content.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        if trimmed.starts_with("relationship ") {
            let rel_indent = leading_indent(line);
            let mut from_table = String::new();
            let mut from_column = String::new();
            let mut to_table = String::new();
            let mut to_column = String::new();
            let mut cross = "single".to_string();
            let mut is_active = true;
            let mut from_card = "many";
            let mut to_card = "one";

            i += 1;
            while i < lines.len() {
                let sub = lines[i];
                let sub_trim = sub.trim();
                if sub_trim.is_empty() {
                    i += 1;
                    continue;
                }
                if leading_indent(sub) <= rel_indent {
                    break;
                }
                if let Some(v) = sub_trim.strip_prefix("fromColumn:") {
                    let (t, c) = parse_tmdl_column_ref(v.trim());
                    from_table = t;
                    from_column = c;
                } else if let Some(v) = sub_trim.strip_prefix("toColumn:") {
                    let (t, c) = parse_tmdl_column_ref(v.trim());
                    to_table = t;
                    to_column = c;
                } else if let Some(v) = sub_trim.strip_prefix("crossFilteringBehavior:") {
                    if v.trim().eq_ignore_ascii_case("bothDirections") {
                        cross = "both".to_string();
                    }
                } else if let Some(v) = sub_trim.strip_prefix("isActive:") {
                    is_active = !v.trim().eq_ignore_ascii_case("false");
                } else if let Some(v) = sub_trim.strip_prefix("fromCardinality:") {
                    from_card = if v.trim().eq_ignore_ascii_case("one") { "one" } else { "many" };
                } else if let Some(v) = sub_trim.strip_prefix("toCardinality:") {
                    to_card = if v.trim().eq_ignore_ascii_case("many") { "many" } else { "one" };
                }
                i += 1;
            }

            if !from_table.is_empty() && !to_table.is_empty() {
                let cardinality = match (from_card, to_card) {
                    ("one", "one") => "1:1",
                    ("one", "many") => "1:N",
                    ("many", "many") => "N:N",
                    _ => "N:1",
                }
                .to_string();

                out.push(PbiRelationshipMeta {
                    from_table,
                    from_column,
                    to_table,
                    to_column,
                    cardinality,
                    cross_filter_direction: cross,
                    is_active,
                });
            }
            continue;
        }
        i += 1;
    }

    out
}

fn leading_indent(line: &str) -> usize {
    line.chars().take_while(|c| *c == '\t' || *c == ' ').count()
}

fn unquote_tmdl_name(raw: &str) -> String {
    let t = raw.trim();
    if t.starts_with('\'') && t.ends_with('\'') && t.len() >= 2 {
        t[1..t.len() - 1].replace("''", "'")
    } else if t.starts_with('"') && t.ends_with('"') && t.len() >= 2 {
        t[1..t.len() - 1].to_string()
    } else {
        t.to_string()
    }
}

fn split_tmdl_assignment(input: &str) -> (&str, Option<String>) {
    let mut in_single = false;
    let bytes = input.as_bytes();
    for (idx, &b) in bytes.iter().enumerate() {
        if b == b'\'' {
            in_single = !in_single;
        } else if b == b'=' && !in_single {
            let lhs = input[..idx].trim();
            let rhs = input[idx + 1..].trim().to_string();
            return (lhs, Some(rhs));
        }
    }
    (input.trim(), None)
}

fn parse_tmdl_column_ref(raw: &str) -> (String, String) {
    // Supports: Table.Column, 'Table Name'.Column, 'Table Name'.'Col Name'
    let mut in_single = false;
    for (idx, ch) in raw.char_indices() {
        if ch == '\'' {
            in_single = !in_single;
        } else if ch == '.' && !in_single {
            let tbl = unquote_tmdl_name(&raw[..idx]);
            let col = unquote_tmdl_name(&raw[idx + 1..]);
            return (tbl, col);
        }
    }
    (String::new(), unquote_tmdl_name(raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_table_tmdl_with_measures_and_partition() {
        let sample = r#"table 'f_Vendas Real'
	lineageTag: abc-123

	measure 'Faturamento Total' = SUM('f_Vendas Real'[Valor])
		formatString: "R$ #,0.00"
		displayFolder: Financeiro

	measure 'Ticket Médio' =
			```
			DIVIDE([Faturamento Total], COUNTROWS('f_Vendas Real'))
			```
		formatString: #,0.00

	column DataVenda
		dataType: dateTime
		formatString: dd/MM/yyyy

	column Valor
		dataType: double

	column Margem = [Valor] * 0.2
		dataType: double

	partition 'f_Vendas Real' = m
		mode: import
		source =
			let
			    Fonte = Csv.Document(File.Contents("vendas.csv"))
			in
			    Fonte
"#;

        let tbl = parse_table_tmdl(sample).expect("parse tmdl");
        assert_eq!(tbl.name, "f_Vendas Real");
        assert_eq!(tbl.measures.len(), 2);
        assert_eq!(tbl.measures[0].name, "Faturamento Total");
        assert_eq!(tbl.measures[0].expression, "SUM('f_Vendas Real'[Valor])");
        assert_eq!(tbl.measures[0].display_folder.as_deref(), Some("Financeiro"));
        assert_eq!(tbl.measures[1].name, "Ticket Médio");
        assert!(tbl.measures[1].expression.contains("DIVIDE"));
        assert_eq!(tbl.columns.len(), 3);
        assert_eq!(tbl.columns[0].data_type, "date");
        assert_eq!(tbl.columns[1].data_type, "number");
        assert!(tbl.columns[2].is_calculated);
        assert_eq!(tbl.partitions.len(), 1);
        assert!(tbl.partitions[0].expression.contains("Csv.Document"));
    }

    #[test]
    fn test_parse_relationships_tmdl() {
        let sample = r#"relationship rel_vendas_clientes
	fromColumn: 'f_Vendas Real'.ClienteId
	toColumn: d_Clientes.'ID Cliente'
	crossFilteringBehavior: bothDirections
	isActive: true
"#;
        let rels = parse_relationships_tmdl(sample);
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].from_table, "f_Vendas Real");
        assert_eq!(rels[0].from_column, "ClienteId");
        assert_eq!(rels[0].to_table, "d_Clientes");
        assert_eq!(rels[0].to_column, "ID Cliente");
        assert_eq!(rels[0].cross_filter_direction, "both");
        assert!(rels[0].is_active);
    }
}
