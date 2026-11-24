use crate::pbi::{
    decode_utf8_or_utf16, parse_legacy_report_json, parse_tmsl_model_json, PbiPowerQueryScript,
    PbiProjectSummary,
};
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;
use zip::ZipArchive;

pub fn open_pbix_file(path: &Path) -> Result<PbiProjectSummary, String> {
    if !path.exists() {
        return Err(format!("Arquivo .pbix não encontrado: {}", path.display()));
    }

    let file = File::open(path).map_err(|e| format!("Erro ao abrir .pbix: {}", e))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("Arquivo .pbix inválido (não é ZIP): {}", e))?;

    let project_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Relatório Power BI")
        .to_string();

    let mut tables = Vec::new();
    let mut relationships = Vec::new();
    let mut pages = Vec::new();
    let mut power_query_scripts = Vec::new();
    let mut warnings = Vec::new();

    let mut has_data_model_bin = false;
    let mut has_data_model_schema = false;

    let entry_names: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string()))
        .collect();

    for name in &entry_names {
        let lower = name.to_lowercase();
        if lower == "report/layout" {
            if let Ok(bytes) = read_zip_entry_bytes(&mut archive, name) {
                let text = decode_utf8_or_utf16(&bytes);
                match parse_legacy_report_json(&text) {
                    Ok(p) => pages = p,
                    Err(e) => warnings.push(format!("Report/Layout: {}", e)),
                }
            }
        } else if lower == "datamodelschema" {
            has_data_model_schema = true;
            if let Ok(bytes) = read_zip_entry_bytes(&mut archive, name) {
                let text = decode_utf8_or_utf16(&bytes);
                match parse_tmsl_model_json(&text) {
                    Ok((t, r)) => {
                        tables = t;
                        relationships = r;
                    }
                    Err(e) => warnings.push(format!("DataModelSchema: {}", e)),
                }
            }
        } else if lower == "datamodel" {
            has_data_model_bin = true;
        } else if lower == "datamashup" {
            if let Ok(bytes) = read_zip_entry_bytes(&mut archive, name) {
                power_query_scripts.extend(extract_power_query_from_datamashup(&bytes));
            }
        }
    }

    if has_data_model_bin && !has_data_model_schema {
        warnings.push(
            "Este .pbix usa modelo VertiPaq binário compactado (DataModel sem DataModelSchema). O layout das páginas/visuais e scripts Power Query (M) foram extraídos; para importar tabelas, relacionamentos e medidas DAX completos, salve o projeto como .pbip (TMDL) no Power BI Desktop."
                .to_string(),
        );
    }

    Ok(PbiProjectSummary {
        source_type: "pbix".to_string(),
        project_name,
        root_path: path.to_string_lossy().to_string(),
        tables,
        relationships,
        pages,
        power_query_scripts,
        loaded_tables: Vec::new(),
        warnings,
    })
}

fn read_zip_entry_bytes<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<Vec<u8>, String> {
    let mut entry = archive
        .by_name(name)
        .map_err(|e| format!("Entrada '{}' ausente no .pbix: {}", name, e))?;
    let mut buf = Vec::new();
    entry
        .read_to_end(&mut buf)
        .map_err(|e| format!("Erro ao ler '{}': {}", name, e))?;
    Ok(buf)
}

fn extract_power_query_from_datamashup(bytes: &[u8]) -> Vec<PbiPowerQueryScript> {
    let mut scripts = Vec::new();
    // DataMashup embeds a ZIP payload starting with PK\x03\x04
    let Some(zip_offset) = bytes.windows(4).position(|w| w == [0x50, 0x4B, 0x03, 0x04]) else {
        return scripts;
    };

    let cursor = Cursor::new(&bytes[zip_offset..]);
    let Ok(mut inner_zip) = ZipArchive::new(cursor) else {
        return scripts;
    };

    let inner_names: Vec<String> = (0..inner_zip.len())
        .filter_map(|i| inner_zip.by_index(i).ok().map(|f| f.name().to_string()))
        .collect();

    for name in inner_names {
        if name.to_lowercase().ends_with(".m") {
            if let Ok(m_bytes) = read_zip_entry_bytes(&mut inner_zip, &name) {
                let text = decode_utf8_or_utf16(&m_bytes);
                if !text.trim().is_empty() {
                    scripts.push(PbiPowerQueryScript {
                        name,
                        script: text,
                    });
                }
            }
        }
    }

    scripts
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn encode_utf16le_no_bom(s: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(s.len() * 2);
        for u in s.encode_utf16() {
            out.extend_from_slice(&u.to_le_bytes());
        }
        out
    }

    #[test]
    fn test_open_synthetic_pbix_utf16le_layout_and_schema() {
        let tmp = NamedTempFile::new().unwrap();
        {
            let mut zw = ZipWriter::new(tmp.reopen().unwrap());
            let opts = SimpleFileOptions::default();

            let layout_json = r#"{"sections":[{"name":"ReportSection","displayName":"Resumo","width":1280,"height":720,"visualContainers":[{"x":10,"y":20,"width":400,"height":250,"config":"{\"name\":\"v1\",\"singleVisual\":{\"visualType\":\"clusteredBarChart\",\"projections\":{\"Category\":[{\"queryRef\":\"f_Vendas.Produto\"}],\"Y\":[{\"queryRef\":\"f_Vendas.Receita\"}]}}}"}]}]}"#;
            zw.start_file("Report/Layout", opts).unwrap();
            zw.write_all(&encode_utf16le_no_bom(layout_json)).unwrap();

            let schema_json = r#"{"model":{"tables":[{"name":"f_Vendas","columns":[{"name":"Produto","dataType":"string"}],"measures":[{"name":"Receita","expression":"SUM(f_Vendas[Valor])"}]}]}}"#;
            zw.start_file("DataModelSchema", opts).unwrap();
            zw.write_all(&encode_utf16le_no_bom(schema_json)).unwrap();

            zw.finish().unwrap();
        }

        let summary = open_pbix_file(tmp.path()).expect("open pbix");
        assert_eq!(summary.source_type, "pbix");
        assert_eq!(summary.pages.len(), 1);
        assert_eq!(summary.pages[0].name, "Resumo");
        assert_eq!(summary.pages[0].visuals[0].visual_type, "bar");
        assert_eq!(summary.pages[0].visuals[0].category_field.as_deref(), Some("Produto"));
        assert_eq!(summary.tables.len(), 1);
        assert_eq!(summary.tables[0].measures[0].name, "Receita");
    }
}
