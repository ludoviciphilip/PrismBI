//! Atualizador via GitHub Releases.
//!
//! Modelo de confiança (v0.8):
//! 1. O CI assina `SHA256SUMS.txt` com uma chave Ed25519 cuja parte privada
//!    só existe no secret `PRISMBI_UPDATE_SIGNING_KEY`; a assinatura crua
//!    (64 bytes) é publicada como `SHA256SUMS.txt.sig`.
//! 2. O app embute só a chave pública (`UPDATE_PUBLIC_KEY_HEX`) e verifica a
//!    assinatura antes de confiar em qualquer hash.
//! 3. O frontend nunca informa URL nem hash: `check_for_updates` guarda o
//!    pacote verificado em `PendingUpdate` no estado do backend e
//!    `download_and_install_update` só instala esse pacote.
//! 4. A URL de download precisa pertencer às releases deste repositório.
//!
//! Assim, um XSS no webview ou uma release adulterada sem a chave privada não
//! conseguem trocar o binário do usuário.

use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const GITHUB_LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/ludoviciphilip/PrismBI/releases/latest";

/// Prefixo obrigatório para qualquer binário baixado pelo updater.
pub const TRUSTED_DOWNLOAD_PREFIX: &str =
    "https://github.com/ludoviciphilip/PrismBI/releases/download/";

/// Chave pública Ed25519 (32 bytes, hex) que assina `SHA256SUMS.txt`.
pub const UPDATE_PUBLIC_KEY_HEX: &str =
    "efbc7fdbf5234578df4e71300a8e6d16fc6f518b075e6357c4c2b4ee17a2a3f6";

const SUMS_ASSET: &str = "SHA256SUMS.txt";
const SUMS_SIG_ASSET: &str = "SHA256SUMS.txt.sig";

/// Como o PrismBI em execução foi instalado; decide qual pacote baixar e se
/// a substituição in-place é permitida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallKind {
    AppImage,
    Deb,
    Rpm,
    /// Instalado por gerenciador que não deve ser contornado (AUR/pacman, Flatpak).
    Managed,
    Unknown,
    /// Windows: instalado pelo `.msi` (Program Files ou LocalAppData\Programs).
    /// O updater baixa o novo `.msi` e roda `msiexec /i ... /passive`.
    MsiInstalled,
    /// Windows: executável solto (ZIP portátil), fora de uma instalação MSI.
    /// Igual ao caminho "managed/unknown" do Linux: baixa para Downloads e
    /// avisa o usuário, sem autoinstalar.
    Portable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_url: String,
    pub release_notes: String,
    pub published_at: String,
    pub asset_name: String,
    pub download_url: String,
    pub asset_size: u64,
    pub expected_sha256: Option<String>,
    /// `true` só quando `SHA256SUMS.txt` tem assinatura válida da chave oficial.
    pub signature_verified: bool,
    /// `true` quando o backend tem um pacote verificado pronto para instalar.
    pub can_install: bool,
    pub install_kind: InstallKind,
    /// Motivo legível quando há atualização mas `can_install` é falso.
    pub install_blocked_reason: Option<String>,
    pub is_appimage: bool,
    pub appimage_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInstallResult {
    pub installed: bool,
    pub needs_restart: bool,
    pub saved_path: Option<String>,
    pub message: String,
}

/// Pacote verificado aguardando instalação (vive só no backend).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingUpdate {
    pub asset_name: String,
    pub download_url: String,
    pub sha256: String,
    pub install_kind: InstallKind,
}

/// Resultado da avaliação pura de uma release.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReleaseVerdict {
    pub pending: Option<PendingUpdate>,
    pub signature_verified: bool,
    pub sha256: Option<String>,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    published_at: String,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

pub fn normalize_version(v: &str) -> Vec<u64> {
    let trimmed = v.trim().trim_start_matches(['v', 'V']);
    let core = trimmed.split('-').next().unwrap_or(trimmed);
    core.split('.')
        .map(|part| part.trim().parse::<u64>().unwrap_or(0))
        .collect()
}

pub fn is_newer_version(current: &str, latest: &str) -> bool {
    let cur = normalize_version(current);
    let lat = normalize_version(latest);
    let max_len = cur.len().max(lat.len());
    for i in 0..max_len {
        let a = *cur.get(i).unwrap_or(&0);
        let b = *lat.get(i).unwrap_or(&0);
        if b != a {
            return b > a;
        }
    }
    false
}

/// Detecta o tipo de instalação a partir do ambiente e de marcadores do SO.
#[cfg(unix)]
pub fn detect_install_kind() -> InstallKind {
    let appimage = std::env::var("APPIMAGE").ok();
    let flatpak = std::env::var("FLATPAK_ID").ok();
    let exe = std::env::current_exe().ok();
    classify_install(
        appimage.as_deref(),
        flatpak.as_deref(),
        exe.as_deref(),
        &|p| Path::new(p).exists(),
    )
}

/// Detecta o tipo de instalação no Windows: MSI (Program Files /
/// LocalAppData\Programs) vs. portátil (qualquer outro local).
#[cfg(windows)]
pub fn detect_install_kind() -> InstallKind {
    let exe = std::env::current_exe().ok();
    let program_files = std::env::var("ProgramFiles").ok();
    let program_files_x86 = std::env::var("ProgramFiles(x86)").ok();
    let local_appdata_programs = std::env::var("LOCALAPPDATA")
        .ok()
        .map(|l| format!("{}\\Programs", l.trim_end_matches(['\\', '/'])));
    let program_dirs: Vec<&str> = [program_files.as_deref(), program_files_x86.as_deref()]
        .into_iter()
        .flatten()
        .collect();
    classify_install_windows(exe.as_deref(), &program_dirs, local_appdata_programs.as_deref())
}

/// Versão pura de `detect_install_kind` (testável).
pub fn classify_install(
    appimage_env: Option<&str>,
    flatpak_env: Option<&str>,
    exe: Option<&Path>,
    exists: &dyn Fn(&str) -> bool,
) -> InstallKind {
    if appimage_env.map(|s| !s.trim().is_empty()).unwrap_or(false) {
        return InstallKind::AppImage;
    }
    if flatpak_env.map(|s| !s.trim().is_empty()).unwrap_or(false) {
        return InstallKind::Managed;
    }
    let in_system = exe
        .map(|p| p.starts_with("/usr") || p.starts_with("/opt"))
        .unwrap_or(false);
    if !in_system {
        return InstallKind::Unknown;
    }
    // Pacote do AUR/pacman: atualizar por fora quebraria o banco do pacman.
    if exists("/var/lib/pacman/local") {
        return InstallKind::Managed;
    }
    // O pacote .deb gerado pelo Tauri se chama "prism-bi".
    if exists("/var/lib/dpkg/info/prism-bi.list")
        || exists("/var/lib/dpkg/info/prismbi.list")
        || exists("/etc/debian_version")
    {
        return InstallKind::Deb;
    }
    if exists("/etc/redhat-release") || exists("/etc/fedora-release") || exists("/etc/SuSE-release")
    {
        return InstallKind::Rpm;
    }
    InstallKind::Unknown
}

/// Versão pura de `detect_install_kind` no Windows (testável em qualquer SO):
/// MSI quando o executável está sob Program Files / Program Files (x86) ou
/// `%LOCALAPPDATA%\Programs`; caso contrário, instalação portátil.
pub fn classify_install_windows(
    exe: Option<&Path>,
    program_dirs: &[&str],
    local_appdata_programs: Option<&str>,
) -> InstallKind {
    let Some(exe) = exe else {
        return InstallKind::Portable;
    };
    let exe_str = exe.to_string_lossy().replace('/', "\\").to_lowercase();
    for pf in program_dirs {
        if !pf.is_empty() && exe_str.starts_with(&pf.replace('/', "\\").to_lowercase()) {
            return InstallKind::MsiInstalled;
        }
    }
    if let Some(lad) = local_appdata_programs {
        if !lad.is_empty() && exe_str.starts_with(&lad.replace('/', "\\").to_lowercase()) {
            return InstallKind::MsiInstalled;
        }
    }
    InstallKind::Portable
}

fn extension_for(kind: InstallKind) -> Option<&'static str> {
    match kind {
        InstallKind::AppImage | InstallKind::Unknown => Some(".appimage"),
        InstallKind::Deb => Some(".deb"),
        InstallKind::Rpm => Some(".rpm"),
        InstallKind::MsiInstalled => Some(".msi"),
        InstallKind::Managed | InstallKind::Portable => None,
    }
}

fn pick_asset(assets: &[GithubAsset], kind: InstallKind) -> Option<&GithubAsset> {
    if kind == InstallKind::Portable {
        return assets.iter().find(|a| {
            let n = a.name.to_lowercase();
            n.ends_with(".zip") && n.contains("portable")
        });
    }
    let ext = extension_for(kind)?;
    assets.iter().find(|a| a.name.to_lowercase().ends_with(ext))
}

pub fn parse_sha256sums(body: &str, asset_name: &str) -> Option<String> {
    let base_target = Path::new(asset_name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(asset_name);
    for line in body.lines() {
        let mut parts = line.split_whitespace();
        let (Some(hash), Some(name)) = (parts.next(), parts.next()) else {
            continue;
        };
        let name = name.trim_start_matches('*');
        let base_line = Path::new(name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(name);
        if (name.eq_ignore_ascii_case(asset_name) || base_line.eq_ignore_ascii_case(base_target))
            && hash.len() == 64
            && hash.chars().all(|c| c.is_ascii_hexdigit())
        {
            return Some(hash.to_lowercase());
        }
    }
    None
}

fn decode_hex<const N: usize>(hex: &str) -> Option<[u8; N]> {
    let hex = hex.trim();
    if hex.len() != N * 2 || !hex.is_ascii() {
        return None;
    }
    let mut out = [0u8; N];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// Verifica a assinatura Ed25519 crua (64 bytes, saída de
/// `openssl pkeyutl -sign -rawin`) de `message` com a chave pública em hex.
pub fn verify_detached_signature(message: &[u8], signature: &[u8], public_key_hex: &str) -> bool {
    let Some(pk_bytes) = decode_hex::<32>(public_key_hex) else {
        return false;
    };
    let Ok(key) = VerifyingKey::from_bytes(&pk_bytes) else {
        return false;
    };
    let Ok(sig_bytes) = <[u8; 64]>::try_from(signature) else {
        return false;
    };
    key.verify_strict(message, &Signature::from_bytes(&sig_bytes))
        .is_ok()
}

pub fn is_trusted_download_url(url: &str) -> bool {
    match url.strip_prefix(TRUSTED_DOWNLOAD_PREFIX) {
        Some(rest) => !rest.is_empty() && !rest.contains("..") && !rest.contains("://"),
        None => false,
    }
}

/// Decide se dá para instalar a partir dos dados já baixados (pura/testável).
pub fn evaluate_release(
    update_available: bool,
    install_kind: InstallKind,
    asset: Option<(&str, &str)>,
    sums: Option<&str>,
    sums_sig: Option<&[u8]>,
    public_key_hex: &str,
) -> ReleaseVerdict {
    let blocked = |reason: &str| ReleaseVerdict {
        blocked_reason: Some(reason.to_string()),
        ..Default::default()
    };
    if !update_available {
        return ReleaseVerdict::default();
    }
    if install_kind == InstallKind::Managed {
        return blocked("Instalação gerenciada (AUR/pacman ou Flatpak): atualize pelo gerenciador de pacotes.");
    }
    let Some((asset_name, url)) = asset else {
        return blocked("A release não tem pacote para este tipo de instalação.");
    };
    let signature_verified = match (sums, sums_sig) {
        (Some(s), Some(sig)) => verify_detached_signature(s.as_bytes(), sig, public_key_hex),
        _ => false,
    };
    let sha256 = sums.and_then(|s| parse_sha256sums(s, asset_name));
    let reason = if !signature_verified {
        Some("Release sem assinatura válida (SHA256SUMS.txt.sig): baixe manualmente pela página da release.")
    } else if sha256.is_none() {
        Some("SHA256SUMS.txt não lista este pacote.")
    } else if !is_trusted_download_url(url) {
        Some("URL de download fora das releases oficiais.")
    } else {
        None
    };
    let pending = match (reason, &sha256) {
        (None, Some(sha)) => Some(PendingUpdate {
            asset_name: asset_name.to_string(),
            download_url: url.to_string(),
            sha256: sha.clone(),
            install_kind,
        }),
        _ => None,
    };
    ReleaseVerdict {
        pending,
        signature_verified,
        sha256,
        blocked_reason: reason.map(str::to_string),
    }
}

fn http_get_bytes(url: &str, user_agent: &str, limit: u64) -> Result<Vec<u8>, String> {
    ureq::get(url)
        .header("User-Agent", user_agent)
        .call()
        .map_err(|e| format!("Falha ao baixar {}: {}", url, e))?
        .into_body()
        .into_with_config()
        .limit(limit)
        .read_to_vec()
        .map_err(|e| format!("Falha ao ler {}: {}", url, e))
}

pub fn check_for_updates(
    current_version: &str,
) -> Result<(UpdateCheckResult, Option<PendingUpdate>), String> {
    let user_agent = format!("PrismBI/{}", current_version.trim());
    let body = ureq::get(GITHUB_LATEST_RELEASE_URL)
        .header("User-Agent", &user_agent)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Falha ao consultar GitHub Releases: {}", e))?
        .into_body()
        .into_with_config()
        .limit(4 * 1024 * 1024)
        .read_to_string()
        .map_err(|e| format!("Falha ao ler resposta do GitHub Releases: {}", e))?;

    let release: GithubRelease =
        serde_json::from_str(&body).map_err(|e| format!("JSON inválido do GitHub: {}", e))?;

    let latest_clean = release.tag_name.trim().trim_start_matches(['v', 'V']).to_string();
    let appimage_path = std::env::var("APPIMAGE").ok().filter(|s| !s.trim().is_empty());
    let install_kind = detect_install_kind();
    let update_available = is_newer_version(current_version, &latest_clean);

    let chosen = pick_asset(&release.assets, install_kind);
    let find = |name: &str| {
        release
            .assets
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
            .map(|a| a.browser_download_url.clone())
    };

    let (sums, sums_sig) = if update_available && chosen.is_some() {
        let sums = find(SUMS_ASSET)
            .and_then(|u| http_get_bytes(&u, &user_agent, 1024 * 1024).ok())
            .and_then(|b| String::from_utf8(b).ok());
        let sig = find(SUMS_SIG_ASSET).and_then(|u| http_get_bytes(&u, &user_agent, 4096).ok());
        (sums, sig)
    } else {
        (None, None)
    };

    let verdict = evaluate_release(
        update_available,
        install_kind,
        chosen.map(|a| (a.name.as_str(), a.browser_download_url.as_str())),
        sums.as_deref(),
        sums_sig.as_deref(),
        UPDATE_PUBLIC_KEY_HEX,
    );

    let result = UpdateCheckResult {
        current_version: current_version.trim().trim_start_matches(['v', 'V']).to_string(),
        latest_version: latest_clean,
        update_available,
        release_url: release.html_url.clone(),
        release_notes: release.body,
        published_at: release.published_at,
        asset_name: chosen.map(|a| a.name.clone()).unwrap_or_default(),
        download_url: chosen
            .map(|a| a.browser_download_url.clone())
            .unwrap_or_else(|| release.html_url.clone()),
        asset_size: chosen.map(|a| a.size).unwrap_or(0),
        expected_sha256: verdict.sha256,
        signature_verified: verdict.signature_verified,
        can_install: verdict.pending.is_some(),
        install_kind,
        install_blocked_reason: verdict.blocked_reason,
        is_appimage: install_kind == InstallKind::AppImage,
        appimage_path,
    };
    Ok((result, verdict.pending))
}

pub fn download_and_install_update(pending: &PendingUpdate) -> Result<UpdateInstallResult, String> {
    // Defesa em profundidade: o PendingUpdate já foi validado, mas revalida.
    if !is_trusted_download_url(&pending.download_url) {
        return Err("URL de download fora das releases oficiais do PrismBI.".to_string());
    }
    let safe_name = Path::new(&pending.asset_name)
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|s| !s.is_empty() && !s.starts_with('.'))
        .ok_or_else(|| "Nome de pacote inválido.".to_string())?
        .to_string();

    let bytes = http_get_bytes(
        &pending.download_url,
        concat!("PrismBI-Updater/", env!("CARGO_PKG_VERSION")),
        512 * 1024 * 1024,
    )?;

    let actual_sha = format!("{:x}", Sha256::digest(&bytes));
    if actual_sha != pending.sha256 {
        return Err(format!(
            "Falha na verificação SHA-256! Esperado: {}, obtido: {}",
            pending.sha256, actual_sha
        ));
    }

    if pending.install_kind == InstallKind::AppImage {
        #[cfg(unix)]
        if let Some(target) = std::env::var("APPIMAGE").ok().filter(|s| !s.trim().is_empty()) {
            let target_path = PathBuf::from(target.trim());
            let tmp_path = PathBuf::from(format!("{}.new", target.trim()));
            fs::write(&tmp_path, &bytes)
                .map_err(|e| format!("Falha ao gravar AppImage temporário: {}", e))?;
            fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("Falha ao definir permissão de execução no AppImage: {}", e))?;
            fs::rename(&tmp_path, &target_path)
                .map_err(|e| format!("Falha ao substituir AppImage atual: {}", e))?;
            return Ok(UpdateInstallResult {
                installed: true,
                needs_restart: true,
                saved_path: Some(target_path.to_string_lossy().to_string()),
                message: "Atualização aplicada ao AppImage com sucesso! Reinicie o PrismBI para carregar a nova versão.".to_string(),
            });
        }
    }

    #[cfg(windows)]
    if pending.install_kind == InstallKind::MsiInstalled {
        let temp_dir = std::env::temp_dir();
        fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("Falha ao preparar pasta temporária: {}", e))?;
        let msi_path = temp_dir.join(&safe_name);
        fs::write(&msi_path, &bytes)
            .map_err(|e| format!("Falha ao salvar instalador MSI: {}", e))?;
        std::process::Command::new("msiexec")
            .args(["/i", &msi_path.to_string_lossy(), "/passive", "/norestart"])
            .spawn()
            .map_err(|e| format!("Falha ao iniciar o instalador MSI: {}", e))?;
        return Ok(UpdateInstallResult {
            installed: true,
            needs_restart: true,
            saved_path: Some(msi_path.to_string_lossy().to_string()),
            message: "Instalador iniciado (msiexec /passive). O PrismBI será fechado para concluir a atualização.".to_string(),
        });
    }

    let downloads_dir = dirs::download_dir()
        .ok_or_else(|| "Não foi possível localizar a pasta Downloads do usuário.".to_string())?;
    fs::create_dir_all(&downloads_dir)
        .map_err(|e| format!("Falha ao criar pasta Downloads: {}", e))?;
    let dest_path = downloads_dir.join(&safe_name);
    fs::write(&dest_path, &bytes)
        .map_err(|e| format!("Falha ao salvar pacote em Downloads: {}", e))?;
    #[cfg(unix)]
    if safe_name.to_lowercase().ends_with(".appimage") {
        let _ = fs::set_permissions(&dest_path, fs::Permissions::from_mode(0o755));
    }

    let saved_str = dest_path.to_string_lossy().to_string();
    let hint = match pending.install_kind {
        InstallKind::Deb => format!(" Instale com: sudo apt install '{}'", saved_str),
        InstallKind::Rpm => format!(" Instale com: sudo dnf install '{}'", saved_str),
        InstallKind::Portable => {
            " Descompacte o ZIP e substitua o executável antigo pelo novo.".to_string()
        }
        _ => String::new(),
    };
    Ok(UpdateInstallResult {
        installed: false,
        needs_restart: false,
        saved_path: Some(saved_str.clone()),
        message: format!(
            "Pacote baixado e verificado (assinatura + SHA-256) em: {}.{}",
            saved_str, hint
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Vetores reais gerados com a chave oficial via
    // `openssl pkeyutl -sign -inkey update-signing.pem -rawin -in <arquivo>`.
    const ABC_SIG_HEX: &str = "0412a1c0cf16899e76c93ffd1decbc52311ddf3660f736412b82b9e7392be9712d5483e0f4ea2c8666ddb7c06edf8157658c3257701537702914bf1679b72403";
    /// Assina exatamente `"a"*64 + "  P.AppImage\n"`.
    const LISTED_SIG_HEX: &str = "16a028abdaf87244ffe6b1193be3029f0c629182899a411134d000131a0955cf983f651b727d74086a0516890e950663ef27b0e468659ebe434c833d1908a805";

    fn sig(hex: &str) -> Vec<u8> {
        decode_hex::<64>(hex).unwrap().to_vec()
    }

    fn listed_sums() -> String {
        format!("{}  P.AppImage\n", "a".repeat(64))
    }

    fn trusted(name: &str) -> String {
        format!("{}v0.8.0/{}", TRUSTED_DOWNLOAD_PREFIX, name)
    }

    fn asset(name: &str) -> GithubAsset {
        GithubAsset {
            name: name.into(),
            browser_download_url: trusted(name),
            size: 1,
        }
    }

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("0.6.0", "0.7.0"));
        assert!(is_newer_version("v0.6.9", "v0.7.0"));
        assert!(is_newer_version("0.7.0", "1.0.0"));
        assert!(is_newer_version("0.9.0", "0.10.0"));
        assert!(!is_newer_version("0.7.0", "0.7.0"));
        assert!(!is_newer_version("0.8.0", "0.7.5"));
    }

    #[test]
    fn signature_from_openssl_verifies_with_embedded_key() {
        assert!(verify_detached_signature(b"abc", &sig(ABC_SIG_HEX), UPDATE_PUBLIC_KEY_HEX));
    }

    #[test]
    fn signature_rejects_tampered_message_wrong_key_or_bad_sizes() {
        let s = sig(ABC_SIG_HEX);
        assert!(!verify_detached_signature(b"abd", &s, UPDATE_PUBLIC_KEY_HEX));
        assert!(!verify_detached_signature(b"abc", &s[..63], UPDATE_PUBLIC_KEY_HEX));
        assert!(!verify_detached_signature(b"abc", &s, "00"));
        let other_key = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
        assert!(!verify_detached_signature(b"abc", &s, other_key));
    }

    #[test]
    fn trusted_url_allowlist() {
        assert!(is_trusted_download_url(&trusted("PrismBI_0.8.0_amd64.AppImage")));
        assert!(!is_trusted_download_url("https://evil.example/PrismBI.AppImage"));
        assert!(!is_trusted_download_url(
            "https://github.com/ludoviciphilip/PrismBI-fork/releases/download/x/a"
        ));
        assert!(!is_trusted_download_url(
            "https://github.com/ludoviciphilip/PrismBI/releases/download/../../evil/a"
        ));
        assert!(!is_trusted_download_url(
            "http://github.com/ludoviciphilip/PrismBI/releases/download/v1/a"
        ));
        assert!(!is_trusted_download_url(TRUSTED_DOWNLOAD_PREFIX));
    }

    #[test]
    fn pick_asset_matches_install_kind() {
        let assets = vec![
            asset("PrismBI_0.8.0_amd64.deb"),
            asset("PrismBI_0.8.0_amd64.AppImage"),
            asset("PrismBI-0.8.0-1.x86_64.rpm"),
            asset("PrismBI_0.9.1_x64.msi"),
            asset("PrismBI_0.9.1_x64-portable.zip"),
        ];
        let pick = |k| pick_asset(&assets, k).map(|a| a.name.clone());
        assert!(pick(InstallKind::AppImage).unwrap().ends_with(".AppImage"));
        assert!(pick(InstallKind::Unknown).unwrap().ends_with(".AppImage"));
        assert!(pick(InstallKind::Deb).unwrap().ends_with(".deb"));
        assert!(pick(InstallKind::Rpm).unwrap().ends_with(".rpm"));
        assert!(pick(InstallKind::MsiInstalled).unwrap().ends_with(".msi"));
        assert!(pick(InstallKind::Portable).unwrap().ends_with("-portable.zip"));
        assert!(pick(InstallKind::Managed).is_none());
        // .deb instalado nunca recebe AppImage por engano (bug da v0.7).
        assert!(pick_asset(&assets[1..2], InstallKind::Deb).is_none());
    }

    #[test]
    fn classify_install_kinds() {
        let none = |_: &str| false;
        let usr = Path::new("/usr/bin/prismbi");
        assert_eq!(
            classify_install(Some("/home/u/P.AppImage"), None, Some(usr), &none),
            InstallKind::AppImage
        );
        assert_eq!(
            classify_install(None, Some("dev.prismbi.app"), Some(usr), &none),
            InstallKind::Managed
        );
        assert_eq!(
            classify_install(None, None, Some(usr), &|p| p == "/var/lib/pacman/local"),
            InstallKind::Managed
        );
        assert_eq!(
            classify_install(None, None, Some(usr), &|p| p == "/etc/debian_version"),
            InstallKind::Deb
        );
        assert_eq!(
            classify_install(None, None, Some(usr), &|p| p == "/var/lib/dpkg/info/prism-bi.list"),
            InstallKind::Deb
        );
        assert_eq!(
            classify_install(None, None, Some(usr), &|p| p == "/etc/fedora-release"),
            InstallKind::Rpm
        );
        assert_eq!(
            classify_install(None, None, Some(Path::new("/home/u/dev/target/debug/prismbi")), &|_| true),
            InstallKind::Unknown
        );
    }

    #[test]
    fn classify_install_windows_msi_vs_portable() {
        let program_files = r"C:\Program Files";
        let program_files_x86 = r"C:\Program Files (x86)";
        let local_appdata_programs = r"C:\Users\ana\AppData\Local\Programs";
        let dirs = [program_files, program_files_x86];

        let msi_exe = Path::new(r"C:\Program Files\PrismBI\prismbi.exe");
        assert_eq!(
            classify_install_windows(Some(msi_exe), &dirs, Some(local_appdata_programs)),
            InstallKind::MsiInstalled
        );

        let msi_exe_localappdata =
            Path::new(r"C:\Users\ana\AppData\Local\Programs\PrismBI\prismbi.exe");
        assert_eq!(
            classify_install_windows(Some(msi_exe_localappdata), &dirs, Some(local_appdata_programs)),
            InstallKind::MsiInstalled
        );

        // Case-insensitive, e com '/' em vez de '\\'.
        let msi_exe_mixed_case = Path::new(r"c:/program files/PrismBI/prismbi.exe");
        assert_eq!(
            classify_install_windows(Some(msi_exe_mixed_case), &dirs, Some(local_appdata_programs)),
            InstallKind::MsiInstalled
        );

        let portable_exe = Path::new(r"D:\Downloads\PrismBI\prismbi.exe");
        assert_eq!(
            classify_install_windows(Some(portable_exe), &dirs, Some(local_appdata_programs)),
            InstallKind::Portable
        );

        assert_eq!(
            classify_install_windows(None, &dirs, Some(local_appdata_programs)),
            InstallKind::Portable
        );
    }

    #[test]
    fn test_parse_sha256sums() {
        let sha = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let body = format!(
            "{}  PrismBI_0.7.0_amd64.deb\n\n{}  dist/PrismBI_0.7.0_amd64.AppImage\n",
            "0".repeat(64),
            sha
        );
        assert_eq!(parse_sha256sums(&body, "PrismBI_0.7.0_amd64.AppImage"), Some(sha.to_string()));
        assert_eq!(parse_sha256sums(&body, "missing.rpm"), None);
    }

    #[test]
    fn evaluate_release_happy_path_produces_pending_update() {
        let sums = listed_sums();
        let url = trusted("P.AppImage");
        let v = evaluate_release(
            true,
            InstallKind::AppImage,
            Some(("P.AppImage", &url)),
            Some(&sums),
            Some(&sig(LISTED_SIG_HEX)),
            UPDATE_PUBLIC_KEY_HEX,
        );
        assert!(v.signature_verified);
        assert_eq!(v.blocked_reason, None);
        let p = v.pending.expect("deveria haver pacote pendente");
        assert_eq!(p.sha256, "a".repeat(64));
        assert_eq!(p.download_url, url);
    }

    #[test]
    fn evaluate_release_without_signature_never_installs() {
        // Caso da v0.7.0: SHA256SUMS.txt existe, mas não há .sig.
        let sums = listed_sums();
        let url = trusted("P.AppImage");
        let v = evaluate_release(true, InstallKind::AppImage, Some(("P.AppImage", &url)), Some(&sums), None, UPDATE_PUBLIC_KEY_HEX);
        assert!(!v.signature_verified && v.pending.is_none());
        assert_eq!(v.sha256.as_deref(), Some("a".repeat(64).as_str()));
        assert!(v.blocked_reason.unwrap().contains("assinatura"));

        // Hash trocado por atacante: assinatura deixa de bater.
        let forged = format!("{}  P.AppImage\n", "b".repeat(64));
        let v = evaluate_release(true, InstallKind::AppImage, Some(("P.AppImage", &url)), Some(&forged), Some(&sig(LISTED_SIG_HEX)), UPDATE_PUBLIC_KEY_HEX);
        assert!(!v.signature_verified && v.pending.is_none());
    }

    #[test]
    fn evaluate_release_rejects_untrusted_url_even_if_signed() {
        let sums = listed_sums();
        let v = evaluate_release(
            true,
            InstallKind::AppImage,
            Some(("P.AppImage", "https://evil.example/P.AppImage")),
            Some(&sums),
            Some(&sig(LISTED_SIG_HEX)),
            UPDATE_PUBLIC_KEY_HEX,
        );
        assert!(v.signature_verified && v.pending.is_none());
        assert!(v.blocked_reason.unwrap().contains("URL"));
    }

    #[test]
    fn evaluate_release_blocks_managed_and_handles_no_update() {
        let v = evaluate_release(true, InstallKind::Managed, None, None, None, UPDATE_PUBLIC_KEY_HEX);
        assert!(v.pending.is_none() && v.blocked_reason.unwrap().contains("gerenciador"));
        assert_eq!(
            evaluate_release(false, InstallKind::AppImage, None, None, None, UPDATE_PUBLIC_KEY_HEX),
            ReleaseVerdict::default()
        );
    }
}
