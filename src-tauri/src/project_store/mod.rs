//! v0.11 Frente B: cache colunar de projeto.
//!
//! Ao lado dos metadados do projeto (`.insight`/IndexedDB), guarda uma
//! cópia materializada das tabelas num único arquivo `.duckdb` anexado
//! (ver `docs/V011.md` §"Medições B" pela escolha do formato). Na
//! reabertura, se o cache existir e for válido, as tabelas voltam do
//! cache sem reler as fontes originais; "Atualizar" continua lendo as
//! fontes e regrava o cache.
//!
//! Layout em disco (uma pasta por projeto):
//! ```text
//! <cache_dir>/
//!   manifest.json   # versão, formato, hash por tabela, contagem de linhas
//!   data.duckdb      # uma tabela por tabela do modelo
//! ```
//!
//! Gravação atômica: todo o conteúdo é escrito numa pasta temporária
//! (`<cache_dir>.tmp-<pid>-<nanos>`) e só promovido com um `rename` da
//! pasta temporária para o nome final depois que todo o conteúdo
//! (incluindo o manifesto) já está no disco. Isso evita que uma queda a
//! meio da gravação deixe o cache anterior corrompido: se o processo
//! morrer antes do rename, o cache antigo (se havia um) continua intacto;
//! o `rename` de diretório no Linux é atômico quando origem e destino
//! estão no mesmo sistema de arquivos.
use duckdb::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::engine::duckdb_store::{
    build_table_payload_pub, quote_ident, sanitize_identifier,
};
use crate::engine::ImportedTablePayload;

/// Formato atual do manifesto. Bump sempre que o layout em disco mudar de
/// forma incompatível (então `format_version != CACHE_FORMAT_VERSION`
/// invalida o cache em vez de tentar lê-lo errado).
pub const CACHE_FORMAT_VERSION: u32 = 1;
const DATA_FILE_NAME: &str = "data.duckdb";
const MANIFEST_FILE_NAME: &str = "manifest.json";

/// Descreve uma tabela do jeito que o chamador (frontend, via o comando
/// Tauri) a conhece: o suficiente para detectar se a fonte mudou desde o
/// último cache, sem precisar reler a fonte para comparar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableCacheMeta {
    pub name: String,
    /// Assinatura estável do schema (nomes+tipos de coluna, já ordenados
    /// pelo chamador) — qualquer coluna adicionada/removida/renomeada
    /// muda isso e invalida o cache dessa tabela.
    pub columns_signature: String,
    /// Assinatura da fonte (caminho do arquivo + mtime, string de conexão
    /// sem credenciais, script M, etc.) — decidida pelo chamador.
    pub source_signature: String,
    pub row_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TableManifestEntry {
    name: String,
    row_count: usize,
    source_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCacheManifest {
    pub format_version: u32,
    pub format: String,
    pub app_version: String,
    pub created_at: String,
    tables: Vec<TableManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheInfo {
    pub exists: bool,
    pub valid: bool,
    pub reason: Option<String>,
    pub format_version: Option<u32>,
    pub total_rows: usize,
    pub table_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveOutcome {
    pub manifest: ProjectCacheManifest,
    pub total_rows: usize,
    pub elapsed_ms: u128,
    pub bytes_on_disk: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadOutcome {
    pub loaded: bool,
    pub reason: Option<String>,
    pub tables: Vec<ImportedTablePayload>,
    pub total_rows: usize,
    pub elapsed_ms: u128,
}

fn hash_hex(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for p in parts {
        hasher.update(p.as_bytes());
        hasher.update(b"\0");
    }
    format!("{:x}", hasher.finalize())
}

fn source_hash_for(meta: &TableCacheMeta) -> String {
    hash_hex(&[&meta.name, &meta.columns_signature, &meta.source_signature])
}

/// Deriva e valida o diretório de cache para um projeto.
///
/// Segurança: `project_key` NUNCA é usado diretamente como caminho — é
/// sempre reduzido a um hash SHA-256 hexadecimal antes de virar nome de
/// pasta, então não existe sequência de caracteres de entrada (`..`,
/// separadores de caminho, bytes nulos, caminhos absolutos de outro SO)
/// que escape do diretório-base. `dir_override` (usado para projetos
/// salvos em arquivo, pasta `<projeto>.vetrobi-data/`) precisa ser um
/// caminho absoluto sem segmentos `..`; caso contrário é rejeitado e cai
/// para `base_dir`.
pub fn resolve_cache_dir(
    project_key: &str,
    base_dir: &Path,
    dir_override: Option<&str>,
) -> Result<PathBuf, String> {
    if project_key.trim().is_empty() {
        return Err("project_key vazio".to_string());
    }
    let key_hash = hash_hex(&[project_key]);
    let subdir = &key_hash[..32.min(key_hash.len())];

    if let Some(raw) = dir_override.map(str::trim).filter(|s| !s.is_empty()) {
        let candidate = Path::new(raw);
        if !candidate.is_absolute() {
            return Err(format!("dir_override precisa ser um caminho absoluto: {}", raw));
        }
        if candidate
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err("dir_override não pode conter '..'".to_string());
        }
        // Nome final sempre deriva do hash, nunca de `project_key` cru,
        // mesmo quando o diretório-base vem de fora.
        return Ok(candidate.join(subdir));
    }

    Ok(base_dir.join("projects").join(subdir))
}

fn now_iso() -> String {
    let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    format!("unix:{}", dur.as_secs())
}

fn tmp_sibling(dir: &Path) -> PathBuf {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "cache".to_string());
    dir.with_file_name(format!("{}.tmp-{}-{}", name, pid, nanos))
}

/// Grava o cache colunar de `tables` (tabelas já carregadas em `conn`,
/// schema `main`) em `cache_dir`, de forma atômica (ver docs no topo do
/// módulo).
pub fn save_project_cache(
    conn: &Connection,
    cache_dir: &Path,
    tables: &[TableCacheMeta],
    app_version: &str,
) -> Result<SaveOutcome, String> {
    let start = Instant::now();
    if let Some(parent) = cache_dir.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Erro ao preparar diretório de cache: {}", e))?;
    }
    let tmp_dir = tmp_sibling(cache_dir);
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).map_err(|e| format!("Erro ao criar pasta temporária do cache: {}", e))?;

    let data_path = tmp_dir.join(DATA_FILE_NAME);
    let data_path_str = data_path.to_string_lossy().replace('\'', "''");

    let result: Result<(usize, Vec<TableManifestEntry>), String> = (|| {
        conn.execute(&format!("ATTACH '{}' AS vetrobi_cache", data_path_str), [])
            .map_err(|e| format!("Erro ao criar arquivo de cache: {}", e))?;

        let mut entries = Vec::with_capacity(tables.len());
        let mut total_rows = 0usize;
        for t in tables {
            let safe = sanitize_identifier(&t.name);
            let copy_sql = format!(
                "CREATE OR REPLACE TABLE vetrobi_cache.{} AS SELECT * FROM {}",
                quote_ident(&safe),
                quote_ident(&safe)
            );
            conn.execute(&copy_sql, []).map_err(|e| {
                format!("Erro ao copiar tabela '{}' para o cache: {}", t.name, e)
            })?;
            total_rows += t.row_count;
            entries.push(TableManifestEntry {
                name: t.name.clone(),
                row_count: t.row_count,
                source_hash: source_hash_for(t),
            });
        }
        conn.execute("DETACH vetrobi_cache", [])
            .map_err(|e| format!("Erro ao finalizar arquivo de cache: {}", e))?;
        Ok((total_rows, entries))
    })();

    let (total_rows, entries) = match result {
        Ok(v) => v,
        Err(e) => {
            let _ = conn.execute("DETACH vetrobi_cache", []);
            let _ = fs::remove_dir_all(&tmp_dir);
            return Err(e);
        }
    };

    let manifest = ProjectCacheManifest {
        format_version: CACHE_FORMAT_VERSION,
        format: "duckdb-attached".to_string(),
        app_version: app_version.to_string(),
        created_at: now_iso(),
        tables: entries,
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Erro ao serializar manifesto do cache: {}", e))?;
    fs::write(tmp_dir.join(MANIFEST_FILE_NAME), manifest_json)
        .map_err(|e| format!("Erro ao gravar manifesto do cache: {}", e))?;

    // Promove atomicamente: remove o cache anterior (se houver) e troca o
    // nome da pasta temporária pelo nome final. Se o processo morrer entre
    // as duas linhas abaixo, o próximo save detecta e limpa a pasta "old"
    // órfã (ver `invalidate_cache`/limpeza em `project_cache_save`).
    let old_dir = cache_dir.with_extension("old-tmp");
    let _ = fs::remove_dir_all(&old_dir);
    if cache_dir.exists() {
        fs::rename(cache_dir, &old_dir).map_err(|e| format!("Erro ao arquivar cache anterior: {}", e))?;
    }
    fs::rename(&tmp_dir, cache_dir).map_err(|e| {
        // tenta reverter para não perder o cache anterior
        let _ = fs::rename(&old_dir, cache_dir);
        format!("Erro ao promover novo cache: {}", e)
    })?;
    let _ = fs::remove_dir_all(&old_dir);

    let bytes_on_disk = fs::metadata(cache_dir.join(DATA_FILE_NAME))
        .map(|m| m.len())
        .unwrap_or(0);

    Ok(SaveOutcome {
        manifest,
        total_rows,
        elapsed_ms: start.elapsed().as_millis(),
        bytes_on_disk,
    })
}

fn read_manifest(cache_dir: &Path) -> Result<ProjectCacheManifest, String> {
    let raw = fs::read_to_string(cache_dir.join(MANIFEST_FILE_NAME))
        .map_err(|e| format!("Manifesto do cache ilegível: {}", e))?;
    serde_json::from_str(&raw).map_err(|e| format!("Manifesto do cache corrompido: {}", e))
}

/// Verifica se o cache em `cache_dir` existe e é compatível com
/// `expected` (mesmas tabelas, mesmo schema/fonte), sem carregar nada.
pub fn cache_info(cache_dir: &Path, expected: &[TableCacheMeta]) -> CacheInfo {
    if !cache_dir.join(MANIFEST_FILE_NAME).exists() {
        return CacheInfo {
            exists: false,
            valid: false,
            reason: Some("cache inexistente".to_string()),
            format_version: None,
            total_rows: 0,
            table_count: 0,
        };
    }
    let manifest = match read_manifest(cache_dir) {
        Ok(m) => m,
        Err(e) => {
            return CacheInfo {
                exists: true,
                valid: false,
                reason: Some(e),
                format_version: None,
                total_rows: 0,
                table_count: 0,
            }
        }
    };
    if manifest.format_version != CACHE_FORMAT_VERSION {
        return CacheInfo {
            exists: true,
            valid: false,
            reason: Some(format!(
                "versão de cache incompatível: {} (esperado {})",
                manifest.format_version, CACHE_FORMAT_VERSION
            )),
            format_version: Some(manifest.format_version),
            total_rows: 0,
            table_count: 0,
        };
    }
    if !cache_dir.join(DATA_FILE_NAME).exists() {
        return CacheInfo {
            exists: true,
            valid: false,
            reason: Some("arquivo de dados do cache ausente (truncado?)".to_string()),
            format_version: Some(manifest.format_version),
            total_rows: 0,
            table_count: 0,
        };
    }
    if let Some(reason) = diff_reason(&manifest, expected) {
        return CacheInfo {
            exists: true,
            valid: false,
            reason: Some(reason),
            format_version: Some(manifest.format_version),
            total_rows: manifest.tables.iter().map(|t| t.row_count).sum(),
            table_count: manifest.tables.len(),
        };
    }
    CacheInfo {
        exists: true,
        valid: true,
        reason: None,
        format_version: Some(manifest.format_version),
        total_rows: manifest.tables.iter().map(|t| t.row_count).sum(),
        table_count: manifest.tables.len(),
    }
}

/// Compara o manifesto salvo com a lista esperada e retorna, se houver
/// divergência, uma razão legível (tabela faltando/sobrando ou hash de
/// fonte diferente — schema ou fonte mudaram desde o último cache).
fn diff_reason(manifest: &ProjectCacheManifest, expected: &[TableCacheMeta]) -> Option<String> {
    if manifest.tables.len() != expected.len() {
        return Some(format!(
            "número de tabelas mudou: cache tem {}, projeto atual tem {}",
            manifest.tables.len(),
            expected.len()
        ));
    }
    for exp in expected {
        let exp_hash = source_hash_for(exp);
        match manifest.tables.iter().find(|t| t.name == exp.name) {
            None => return Some(format!("tabela '{}' não está no cache", exp.name)),
            Some(found) if found.source_hash != exp_hash => {
                return Some(format!("tabela '{}' mudou de schema/fonte desde o último cache", exp.name))
            }
            Some(found) if found.row_count != exp.row_count => {
                return Some(format!(
                    "tabela '{}' tem contagem de linhas diferente (cache: {}, atual: {})",
                    exp.name, found.row_count, exp.row_count
                ))
            }
            _ => {}
        }
    }
    None
}

/// Carrega as tabelas do cache em `conn` (schema `main`), se o cache
/// existir e validar contra `expected`. Se inválido/ausente, não toca em
/// `conn` e retorna `loaded: false` com o motivo — o chamador deve então
/// reler as fontes originais (comportamento atual).
pub fn load_project_cache(
    conn: &Connection,
    cache_dir: &Path,
    expected: &[TableCacheMeta],
) -> Result<LoadOutcome, String> {
    let start = Instant::now();
    let info = cache_info(cache_dir, expected);
    if !info.valid {
        return Ok(LoadOutcome {
            loaded: false,
            reason: info.reason,
            tables: Vec::new(),
            total_rows: 0,
            elapsed_ms: start.elapsed().as_millis(),
        });
    }

    let data_path = cache_dir.join(DATA_FILE_NAME);
    let data_path_str = data_path.to_string_lossy().replace('\'', "''");

    let result: Result<Vec<ImportedTablePayload>, String> = (|| {
        conn.execute(
            &format!("ATTACH '{}' AS vetrobi_cache (READ_ONLY)", data_path_str),
            [],
        )
        .map_err(|e| format!("Erro ao abrir arquivo de cache: {}", e))?;

        let mut payloads = Vec::with_capacity(expected.len());
        for t in expected {
            let safe = sanitize_identifier(&t.name);
            let load_sql = format!(
                "CREATE OR REPLACE TABLE {} AS SELECT * FROM vetrobi_cache.{}",
                quote_ident(&safe),
                quote_ident(&safe)
            );
            conn.execute(&load_sql, []).map_err(|e| {
                format!("Erro ao carregar tabela '{}' do cache: {}", t.name, e)
            })?;
            let payload = build_table_payload_pub(conn, &safe, &t.name)?;
            payloads.push(payload);
        }
        let _ = conn.execute("DETACH vetrobi_cache", []);
        Ok(payloads)
    })();

    let tables = match result {
        Ok(v) => v,
        Err(e) => {
            let _ = conn.execute("DETACH vetrobi_cache", []);
            return Ok(LoadOutcome {
                loaded: false,
                reason: Some(e),
                tables: Vec::new(),
                total_rows: 0,
                elapsed_ms: start.elapsed().as_millis(),
            });
        }
    };

    let total_rows: usize = tables.iter().map(|t| t.total_rows).sum();
    Ok(LoadOutcome {
        loaded: true,
        reason: None,
        tables,
        total_rows,
        elapsed_ms: start.elapsed().as_millis(),
    })
}

/// Remove o cache de `cache_dir` (se existir). Usado quando o projeto é
/// descartado ou quando o chamador decide forçar reconstrução total.
pub fn invalidate_cache(cache_dir: &Path) -> Result<(), String> {
    if cache_dir.exists() {
        fs::remove_dir_all(cache_dir).map_err(|e| format!("Erro ao invalidar cache: {}", e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_conn_with_table(name: &str, n: i64) -> Connection {
        let conn = Connection::open_in_memory().expect("open duckdb");
        conn.execute(
            &format!(
                "CREATE TABLE {} AS SELECT id, id::DOUBLE AS val FROM range({}) t(id)",
                quote_ident(name),
                n
            ),
            [],
        )
        .expect("create table");
        conn
    }

    fn meta_for(name: &str, n: usize) -> TableCacheMeta {
        TableCacheMeta {
            name: name.to_string(),
            columns_signature: "id:number,val:number".to_string(),
            source_signature: "test://fixed".to_string(),
            row_count: n,
        }
    }

    #[test]
    fn round_trip_save_then_load_restores_rows_and_hash() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache_dir = dir.path().join("proj_cache");
        let conn = make_conn_with_table("f_vendas", 500);
        let metas = vec![meta_for("f_vendas", 500)];

        let save = save_project_cache(&conn, &cache_dir, &metas, "0.11.0-test").expect("save");
        assert_eq!(save.total_rows, 500);
        assert!(cache_dir.join("manifest.json").exists());
        assert!(cache_dir.join("data.duckdb").exists());

        // Fresh in-memory connection, as if the app had just restarted.
        let fresh = Connection::open_in_memory().expect("fresh conn");
        let outcome = load_project_cache(&fresh, &cache_dir, &metas).expect("load");
        assert!(outcome.loaded, "reason: {:?}", outcome.reason);
        assert_eq!(outcome.total_rows, 500);
        assert_eq!(outcome.tables.len(), 1);
        assert_eq!(outcome.tables[0].total_rows, 500);

        let sum: f64 = fresh
            .query_row("SELECT SUM(val) FROM f_vendas", [], |r| r.get(0))
            .expect("sum after cache load");
        assert!((sum - (499.0 * 500.0 / 2.0)).abs() < 0.001);
    }

    #[test]
    fn corrupted_manifest_falls_back_instead_of_panicking() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache_dir = dir.path().join("proj_cache");
        let conn = make_conn_with_table("f_vendas", 10);
        let metas = vec![meta_for("f_vendas", 10)];
        save_project_cache(&conn, &cache_dir, &metas, "0.11.0-test").expect("save");

        fs::write(cache_dir.join("manifest.json"), b"{ not valid json ").expect("corrupt manifest");

        let fresh = Connection::open_in_memory().expect("fresh conn");
        let outcome = load_project_cache(&fresh, &cache_dir, &metas).expect("load must not error");
        assert!(!outcome.loaded);
        assert!(outcome.reason.unwrap().contains("corrompido"));
    }

    #[test]
    fn truncated_data_file_falls_back() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache_dir = dir.path().join("proj_cache");
        let conn = make_conn_with_table("f_vendas", 10);
        let metas = vec![meta_for("f_vendas", 10)];
        save_project_cache(&conn, &cache_dir, &metas, "0.11.0-test").expect("save");

        fs::remove_file(cache_dir.join("data.duckdb")).expect("remove data file");

        let fresh = Connection::open_in_memory().expect("fresh conn");
        let outcome = load_project_cache(&fresh, &cache_dir, &metas).expect("load must not error");
        assert!(!outcome.loaded);
        assert!(outcome.reason.unwrap().contains("ausente"));
    }

    #[test]
    fn incompatible_format_version_falls_back() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache_dir = dir.path().join("proj_cache");
        let conn = make_conn_with_table("f_vendas", 10);
        let metas = vec![meta_for("f_vendas", 10)];
        save_project_cache(&conn, &cache_dir, &metas, "0.11.0-test").expect("save");

        let mut manifest: ProjectCacheManifest = read_manifest(&cache_dir).expect("read manifest");
        manifest.format_version = CACHE_FORMAT_VERSION + 999;
        fs::write(
            cache_dir.join("manifest.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .expect("rewrite manifest");

        let fresh = Connection::open_in_memory().expect("fresh conn");
        let outcome = load_project_cache(&fresh, &cache_dir, &metas).expect("load must not error");
        assert!(!outcome.loaded);
        assert!(outcome.reason.unwrap().contains("incompatível"));
    }

    #[test]
    fn schema_change_invalidates_cache() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache_dir = dir.path().join("proj_cache");
        let conn = make_conn_with_table("f_vendas", 10);
        let metas = vec![meta_for("f_vendas", 10)];
        save_project_cache(&conn, &cache_dir, &metas, "0.11.0-test").expect("save");

        let mut changed = metas.clone();
        changed[0].columns_signature = "id:number,val:number,novo:text".to_string();

        let fresh = Connection::open_in_memory().expect("fresh conn");
        let outcome = load_project_cache(&fresh, &cache_dir, &changed).expect("load must not error");
        assert!(!outcome.loaded);
        assert!(outcome.reason.unwrap().contains("mudou de schema"));
    }

    #[test]
    fn save_is_atomic_no_partial_dir_survives_on_attach_failure() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache_dir = dir.path().join("proj_cache");
        // A table that doesn't exist in `conn` makes the copy step fail,
        // so save_project_cache must clean up its temp dir and must not
        // leave anything at `cache_dir`.
        let conn = Connection::open_in_memory().expect("conn");
        let metas = vec![meta_for("tabela_inexistente", 10)];

        let result = save_project_cache(&conn, &cache_dir, &metas, "0.11.0-test");
        assert!(result.is_err());
        assert!(!cache_dir.exists(), "cache_dir não deveria existir após falha");

        let parent = dir.path();
        let leftover_tmp: Vec<_> = fs::read_dir(parent)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp-"))
            .collect();
        assert!(
            leftover_tmp.is_empty(),
            "não deveria sobrar pasta temporária: {:?}",
            leftover_tmp
        );
    }

    #[test]
    fn resolve_cache_dir_rejects_path_traversal_in_project_key() {
        let base = Path::new("/tmp/vetrobi_appdata_base");
        let malicious = "../../etc/passwd";
        let dir = resolve_cache_dir(malicious, base, None).expect("should not error, just hash it");
        // The raw traversal string must never appear as a literal path
        // component — it's folded into a hash first.
        assert!(!dir.to_string_lossy().contains(".."));
        assert!(dir.starts_with(base));
    }

    #[test]
    fn resolve_cache_dir_rejects_relative_or_traversal_override() {
        let base = Path::new("/tmp/vetrobi_appdata_base");
        assert!(resolve_cache_dir("proj1", base, Some("relative/dir")).is_err());
        assert!(resolve_cache_dir("proj1", base, Some("/abs/../escape")).is_err());
        // A clean absolute override is accepted, but the leaf name is
        // still derived from the project key's hash, never the raw key.
        // "/tmp/..." is not absolute on Windows (no drive letter), so build the
        // override from the platform temp dir, which is absolute everywhere.
        let abs_override = std::env::temp_dir().join("vetrobi_file_cache");
        let traversal = abs_override.join("..").join("escape");
        assert!(resolve_cache_dir("proj1", base, Some(&traversal.to_string_lossy())).is_err());
        let dir = resolve_cache_dir("proj1", base, Some(&abs_override.to_string_lossy())).expect("ok");
        assert!(dir.starts_with(&abs_override));
    }

    #[test]
    fn cache_info_reports_missing_cache_without_erroring() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache_dir = dir.path().join("never_saved");
        let info = cache_info(&cache_dir, &[meta_for("x", 1)]);
        assert!(!info.exists);
        assert!(!info.valid);
    }

    #[test]
    fn invalidate_cache_removes_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache_dir = dir.path().join("proj_cache");
        let conn = make_conn_with_table("f_vendas", 5);
        let metas = vec![meta_for("f_vendas", 5)];
        save_project_cache(&conn, &cache_dir, &metas, "0.11.0-test").expect("save");
        assert!(cache_dir.exists());
        invalidate_cache(&cache_dir).expect("invalidate");
        assert!(!cache_dir.exists());
        // Invalidating an already-gone cache is a no-op, not an error.
        invalidate_cache(&cache_dir).expect("invalidate again is a no-op");
    }
}
