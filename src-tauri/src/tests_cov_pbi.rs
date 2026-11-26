//! Coverage tests for `crate::pbi` (TMDL parser), written against its public
//! API from the crate root. Lives outside `src/pbi/**` (front B's territory
//! for v0.10 round 1): no files there are touched.

use crate::pbi::tmdl::{parse_relationships_tmdl, parse_table_tmdl};

#[test]
fn parse_table_tmdl_basic_columns_and_measure() {
    let src = r##"
table Vendas
    lineageTag: abc-123

    column Data
        dataType: dateTime
        formatString: "dd/mm/yyyy"
        sortByColumn: DataOrdem

    column Valor
        dataType: double
        isHidden

    column ValorDobrado = Vendas[Valor] * 2
        dataType: double

    measure 'Total Vendas' = SUM(Vendas[Valor])
        formatString: "#,##0.00"
        displayFolder: Financeiro

    partition Vendas = m
        mode: import
        source =
            let
                Origem = Csv.Document(File.Contents("vendas.csv"))
            in
                Origem
"##;
    let meta = parse_table_tmdl(src).expect("valid TMDL table");
    assert_eq!(meta.name, "Vendas");
    assert!(!meta.is_hidden);

    let data_col = meta.columns.iter().find(|c| c.name == "Data").unwrap();
    assert_eq!(data_col.format_string.as_deref(), Some("dd/mm/yyyy"));
    assert_eq!(data_col.sort_by_column.as_deref(), Some("DataOrdem"));
    assert!(!data_col.is_calculated);

    let valor_col = meta.columns.iter().find(|c| c.name == "Valor").unwrap();
    assert!(valor_col.is_hidden);

    let calc_col = meta.columns.iter().find(|c| c.name == "ValorDobrado").unwrap();
    assert!(calc_col.is_calculated);
    assert_eq!(calc_col.expression.as_deref(), Some("Vendas[Valor] * 2"));

    assert_eq!(meta.measures.len(), 1);
    let m = &meta.measures[0];
    assert_eq!(m.name, "Total Vendas");
    assert_eq!(m.expression, "SUM(Vendas[Valor])");
    assert_eq!(m.format_string.as_deref(), Some("#,##0.00"));

    assert_eq!(meta.partitions.len(), 1);
}

#[test]
fn parse_table_tmdl_hidden_table_flag() {
    // TMDL uses tab indentation; a table-level isHidden sits one tab in.
    let src = "table OcultaInterna\n\tisHidden\n\n\tcolumn X\n\t\tdataType: int64\n";
    let meta = parse_table_tmdl(src).unwrap();
    assert!(meta.is_hidden);
    assert_eq!(meta.columns.len(), 1);
}

#[test]
fn parse_table_tmdl_measure_with_code_fence() {
    let src = r#"
table T
    measure Fenced = ```
        CALCULATE(
            SUM(T[X]),
            T[Y] > 0
        )
        ```
"#;
    let meta = parse_table_tmdl(src).unwrap();
    assert_eq!(meta.measures.len(), 1);
    let m = &meta.measures[0];
    assert_eq!(m.name, "Fenced");
    assert!(m.expression.contains("CALCULATE"));
    assert!(m.expression.contains("SUM(T[X])"));
}

#[test]
fn parse_table_tmdl_empty_input_is_an_error() {
    // No `table ` declaration at all: the parser rejects it rather than
    // silently returning an empty/nameless table.
    let err = parse_table_tmdl("").unwrap_err();
    assert!(err.contains("table"));
}

#[test]
fn parse_table_tmdl_ignores_doc_comments() {
    let src = "/// a doc comment about this table\ntable Doc\n    column A\n        dataType: string\n";
    let meta = parse_table_tmdl(src).unwrap();
    assert_eq!(meta.name, "Doc");
    assert_eq!(meta.columns.len(), 1);
}

#[test]
fn parse_relationships_tmdl_one_to_many_active() {
    let src = r#"
relationship r1
    fromColumn: Vendas.ClienteId
    toColumn: Clientes.Id
    fromCardinality: many
    toCardinality: one
"#;
    let rels = parse_relationships_tmdl(src);
    assert_eq!(rels.len(), 1);
    let r = &rels[0];
    assert_eq!(r.from_table, "Vendas");
    assert_eq!(r.from_column, "ClienteId");
    assert_eq!(r.to_table, "Clientes");
    assert_eq!(r.to_column, "Id");
    // fromCardinality=many, toCardinality=one: the "from" side is the fact
    // table (many rows per Clientes.Id), so cardinality is "N:1".
    assert_eq!(r.cardinality, "N:1");
    assert!(r.is_active);
    assert_eq!(r.cross_filter_direction, "single");
}

#[test]
fn parse_relationships_tmdl_inactive_bidirectional() {
    let src = r#"
relationship r2
    fromColumn: A.K
    toColumn: B.K
    crossFilteringBehavior: bothDirections
    isActive: false
"#;
    let rels = parse_relationships_tmdl(src);
    assert_eq!(rels.len(), 1);
    assert!(!rels[0].is_active);
    assert_eq!(rels[0].cross_filter_direction, "both");
}

#[test]
fn parse_relationships_tmdl_many_to_many_and_one_to_one() {
    let src = r#"
relationship rA
    fromColumn: X.K
    toColumn: Y.K
    fromCardinality: many
    toCardinality: many

relationship rB
    fromColumn: P.K
    toColumn: Q.K
    fromCardinality: one
    toCardinality: one
"#;
    let rels = parse_relationships_tmdl(src);
    assert_eq!(rels.len(), 2);
    assert_eq!(rels[0].cardinality, "N:N");
    assert_eq!(rels[1].cardinality, "1:1");
}

#[test]
fn parse_relationships_tmdl_skips_incomplete_blocks() {
    // No fromColumn/toColumn at all -> no relationship recorded.
    let src = "relationship broken\n    crossFilteringBehavior: bothDirections\n";
    let rels = parse_relationships_tmdl(src);
    assert!(rels.is_empty());
}

#[test]
fn parse_relationships_tmdl_empty_input() {
    assert!(parse_relationships_tmdl("").is_empty());
}
