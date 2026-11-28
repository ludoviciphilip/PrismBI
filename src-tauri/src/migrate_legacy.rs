//! Migração de dados do PrismBI (identifier antigo `dev.prismbi.app`) para o
//! VetroBI (`dev.vetrobi.app`), feita no commit a6d3774.
//!
//! O Tauri v2 (via WebKitGTK no Linux e WebView2 no Windows) guarda
//! localStorage, IndexedDB e cache em pastas cujo caminho é derivado do
//! `identifier` do app, não da origem do webview:
//!
//! - Linux: `~/.local/share/<identifier>/{localstorage,databases,storage,
//!   CacheStorage,WebKitCache,mediakeys,hsts-storage.sqlite}`, e também
//!   `~/.config/<identifier>` e `~/.cache/<identifier>` quando existirem.
//! - Windows: `%APPDATA%\<identifier>` (dados do Tauri/WebView2 "roaming")
//!   e `%LOCALAPPDATA%\<identifier>\EBWebView` (perfil do WebView2 —
//!   IndexedDB, localStorage, cache).
//!
//! O NOME dos arquivos de localStorage/IndexedDB dentro dessas pastas é
//! derivado da ORIGEM do webview (ex.: `tauri_localhost_0.localstorage`
//! para a origem `tauri://localhost`, `http_localhost_1420.localstorage`
//! para o dev server), e essa origem NÃO muda com o identifier — ela é
//! fixa (`tauri://localhost` em produção, `http://localhost:<porta>` em
//! dev). Confirmado inspecionando `~/.local/share/dev.prismbi.app/
//! localstorage/` numa instalação real: os arquivos lá dentro chamam-se
//! `tauri_localhost_0.localstorage` e `http_localhost_1420.localstorage`,
//! sem qualquer referência a "prismbi". Ou seja: trocar o identifier não
//! muda o nome dos arquivos, só a pasta-base onde eles vivem — por isso
//! basta copiar a pasta inteira para o novo identifier, recursivamente,
//! para o app voltar a achar os mesmos dados.
//!
//! Por isso copiamos (nunca movemos — o PrismBI antigo pode continuar
//! instalado ao lado, como `.deb`/`.rpm` separado, e mover quebraria ele)
//! cada `<base>/dev.prismbi.app` para `<base>/dev.vetrobi.app` quando o
//! destino não existir ou estiver vazio, em cada pasta-base relevante
//! (data, config, cache, local data).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Nome do marcador gravado no destino depois de uma migração bem-sucedida,
/// para nunca repetir o trabalho (e nunca sobrescrever dados novos do
/// VetroBI com uma cópia antiga do PrismBI).
pub const MIGRATION_MARKER: &str = ".migrated-from-prismbi";

pub const LEGACY_IDENTIFIER: &str = "dev.prismbi.app";
pub const CURRENT_IDENTIFIER: &str = "dev.vetrobi.app";

/// Verdadeiro se `dir` não existe ou existe mas está vazio (sem contar o
/// próprio marcador, que não deveria existir ainda nesse ponto, mas não
/// custa ser robusto).
fn is_missing_or_empty(dir: &Path) -> bool {
    match fs::read_dir(dir) {
        Err(_) => true, // não existe (ou inacessível) -> trata como "pode migrar"
        Ok(mut entries) => entries.next().is_none(),
    }
}

/// Copia recursivamente `src` para `dst`, criando diretórios conforme
/// necessário. Arquivos binários são copiados byte a byte (fs::copy já faz
/// isso). Links simbólicos são ignorados defensivamente (não esperamos
/// nenhum nesses diretórios de dados do webview, mas não queremos que um
/// link quebrado derrube a migração).
fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            fs::copy(&src_path, &dst_path)?;
        }
        // symlinks e outros tipos especiais: ignorados de propósito.
    }
    Ok(())
}

/// Migra um único par (pasta antiga, pasta nova) se aplicável. Retorna
/// `true` se copiou algo, `false` se não havia nada a fazer (origem
/// ausente, destino já tem dados, ou já migrado antes).
fn migrate_one(old_dir: &Path, new_dir: &Path) -> bool {
    if !old_dir.is_dir() {
        return false; // nada do PrismBI nessa pasta-base
    }

    let marker = new_dir.join(MIGRATION_MARKER);
    if marker.exists() {
        eprintln!(
            "[migrate_legacy] {} já migrado antes, pulando.",
            new_dir.display()
        );
        return false;
    }

    if !is_missing_or_empty(new_dir) {
        eprintln!(
            "[migrate_legacy] {} já tem dados do VetroBI, não sobrescrevendo.",
            new_dir.display()
        );
        return false;
    }

    eprintln!(
        "[migrate_legacy] copiando dados do PrismBI de {} para {}...",
        old_dir.display(),
        new_dir.display()
    );

    match copy_dir_recursive(old_dir, new_dir) {
        Ok(()) => {
            if let Err(e) = fs::write(&marker, b"") {
                eprintln!(
                    "[migrate_legacy] aviso: falha ao gravar marcador {}: {e}",
                    marker.display()
                );
            }
            eprintln!("[migrate_legacy] migração concluída: {}", new_dir.display());
            true
        }
        Err(e) => {
            eprintln!(
                "[migrate_legacy] erro ao copiar {} -> {}: {e}. Continuando sem migrar esta pasta.",
                old_dir.display(),
                new_dir.display()
            );
            false
        }
    }
}

/// Migra os dados legados do PrismBI (`dev.prismbi.app`) para o VetroBI
/// (`dev.vetrobi.app`) em cada pasta-base fornecida (tipicamente: data dir,
/// config dir, cache dir e local-data dir — algumas coincidem no Linux).
///
/// Recebe as bases por parâmetro (em vez de resolvê-las com `dirs::*()`
/// internamente) para ser testável com diretórios temporários. Nunca
/// entra em pânico: qualquer erro de I/O é logado com `eprintln!` e a
/// função segue para a próxima base, sem derrubar o app.
///
/// Bases duplicadas (comum no Linux, onde `data_dir` == `data_local_dir`)
/// são deduplicadas para não fazer trabalho em dobro nem logar duas vezes.
pub fn migrate_legacy_app_dirs(bases: &[PathBuf]) {
    let mut seen = std::collections::HashSet::new();
    for base in bases {
        if !seen.insert(base.clone()) {
            continue;
        }
        let old_dir = base.join(LEGACY_IDENTIFIER);
        let new_dir = base.join(CURRENT_IDENTIFIER);
        migrate_one(&old_dir, &new_dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_base(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "vetrobi-migrate-test-{name}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).expect("criar base temporária");
        dir
    }

    fn write_file(path: &Path, contents: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn copies_when_new_dir_does_not_exist() {
        let base = temp_base("copy-new");
        let old = base.join(LEGACY_IDENTIFIER);
        write_file(
            &old.join("localstorage/tauri_localhost_0.localstorage"),
            b"fake-sqlite-bytes",
        );
        write_file(&old.join("databases/indexeddb/marker.db"), b"idb-data");

        migrate_legacy_app_dirs(&[base.clone()]);

        let new_dir = base.join(CURRENT_IDENTIFIER);
        assert!(new_dir.join(MIGRATION_MARKER).exists());
        assert_eq!(
            fs::read(new_dir.join("localstorage/tauri_localhost_0.localstorage")).unwrap(),
            b"fake-sqlite-bytes"
        );
        assert_eq!(
            fs::read(new_dir.join("databases/indexeddb/marker.db")).unwrap(),
            b"idb-data"
        );

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn does_not_overwrite_when_new_dir_already_has_data() {
        let base = temp_base("no-overwrite");
        let old = base.join(LEGACY_IDENTIFIER);
        write_file(&old.join("localstorage/x.localstorage"), b"old-data");

        let new_dir = base.join(CURRENT_IDENTIFIER);
        write_file(&new_dir.join("localstorage/x.localstorage"), b"fresh-vetrobi-data");

        migrate_legacy_app_dirs(&[base.clone()]);

        // Não sobrescreveu, e não gravou marcador (não migrou de fato).
        assert_eq!(
            fs::read(new_dir.join("localstorage/x.localstorage")).unwrap(),
            b"fresh-vetrobi-data"
        );
        assert!(!new_dir.join(MIGRATION_MARKER).exists());

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn is_idempotent_second_run_does_nothing() {
        let base = temp_base("idempotent");
        let old = base.join(LEGACY_IDENTIFIER);
        write_file(&old.join("localstorage/x.localstorage"), b"original");

        migrate_legacy_app_dirs(&[base.clone()]);

        // Simula o usuário usando o VetroBI depois da migração: dado novo.
        let new_dir = base.join(CURRENT_IDENTIFIER);
        fs::write(new_dir.join("localstorage/x.localstorage"), b"edited-by-vetrobi").unwrap();

        // Segunda chamada (ex.: próxima abertura do app) não deve tocar nada.
        migrate_legacy_app_dirs(&[base.clone()]);

        assert_eq!(
            fs::read(new_dir.join("localstorage/x.localstorage")).unwrap(),
            b"edited-by-vetrobi"
        );

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn preserves_subfolders_and_binary_files() {
        let base = temp_base("subfolders");
        let old = base.join(LEGACY_IDENTIFIER);
        let binary: Vec<u8> = (0u8..=255).collect();
        write_file(&old.join("storage/default/x/file"), &binary);
        write_file(&old.join("CacheStorage/a/b/c.bin"), &binary);
        write_file(&old.join("hsts-storage.sqlite"), b"sqlite-header-bytes");

        migrate_legacy_app_dirs(&[base.clone()]);

        let new_dir = base.join(CURRENT_IDENTIFIER);
        assert_eq!(fs::read(new_dir.join("storage/default/x/file")).unwrap(), binary);
        assert_eq!(fs::read(new_dir.join("CacheStorage/a/b/c.bin")).unwrap(), binary);
        assert_eq!(
            fs::read(new_dir.join("hsts-storage.sqlite")).unwrap(),
            b"sqlite-header-bytes"
        );

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn does_nothing_when_legacy_dir_does_not_exist() {
        let base = temp_base("no-legacy");
        migrate_legacy_app_dirs(&[base.clone()]);
        assert!(!base.join(CURRENT_IDENTIFIER).exists());
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn deduplicates_identical_bases() {
        // No Linux, data_dir() == data_local_dir(); garantimos que passar a
        // mesma base duas vezes não duplica trabalho nem quebra nada.
        let base = temp_base("dedup");
        let old = base.join(LEGACY_IDENTIFIER);
        write_file(&old.join("localstorage/x.localstorage"), b"data");

        migrate_legacy_app_dirs(&[base.clone(), base.clone()]);

        let new_dir = base.join(CURRENT_IDENTIFIER);
        assert_eq!(
            fs::read(new_dir.join("localstorage/x.localstorage")).unwrap(),
            b"data"
        );

        fs::remove_dir_all(&base).ok();
    }

    /// Verificação manual, NÃO roda em CI: aponta `VETROBI_MIGRATE_VERIFY_BASE`
    /// para uma cópia temporária de `~/.local/share` (nunca para a pasta real
    /// do usuário) e confirma que localStorage e IndexedDB foram copiados.
    /// `cargo test -- --ignored migrate_legacy::tests::manual_real_data_verification`
    #[test]
    #[ignore]
    fn manual_real_data_verification() {
        let base = PathBuf::from(
            std::env::var("VETROBI_MIGRATE_VERIFY_BASE")
                .expect("defina VETROBI_MIGRATE_VERIFY_BASE para uma pasta TEMPORÁRIA"),
        );
        let old = base.join(LEGACY_IDENTIFIER);
        assert!(old.is_dir(), "pasta de origem {} não existe", old.display());

        migrate_legacy_app_dirs(&[base.clone()]);

        let new_dir = base.join(CURRENT_IDENTIFIER);
        assert!(new_dir.join(MIGRATION_MARKER).exists(), "marcador não gravado");
        assert!(
            new_dir.join("localstorage/tauri_localhost_0.localstorage").exists(),
            "localStorage não copiado"
        );
        assert!(
            new_dir.join("databases/indexeddb").exists(),
            "IndexedDB não copiado"
        );
        eprintln!("[manual_real_data_verification] OK: {}", new_dir.display());
    }

    #[test]
    fn old_prismbi_dir_is_left_untouched_copy_not_move() {
        let base = temp_base("copy-not-move");
        let old = base.join(LEGACY_IDENTIFIER);
        write_file(&old.join("localstorage/x.localstorage"), b"data");

        migrate_legacy_app_dirs(&[base.clone()]);

        // PrismBI antigo continua intacto, para não quebrar uma instalação
        // dele que ainda exista ao lado.
        assert!(old.join("localstorage/x.localstorage").exists());
        assert_eq!(
            fs::read(old.join("localstorage/x.localstorage")).unwrap(),
            b"data"
        );

        fs::remove_dir_all(&base).ok();
    }
}
