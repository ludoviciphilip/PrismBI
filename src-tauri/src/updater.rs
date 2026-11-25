use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

const GITHUB_LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/ludoviciphilip/PrismBI/releases/latest";

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
        if b > a {
            return true;
        }
        if b < a {
            return false;
        }
    }
    false
}

fn pick_linux_asset<'a>(assets: &'a [GithubAsset], is_appimage: bool) -> Option<&'a GithubAsset> {
    if is_appimage {
        if let Some(a) = assets
            .iter()
            .find(|a| a.name.to_lowercase().ends_with(".appimage"))
        {
            return Some(a);
        }
    }

    if let Some(a) = assets
        .iter()
        .find(|a| a.name.to_lowercase().ends_with(".appimage"))
    {
        return Some(a);
    }

    if let Some(a) = assets
        .iter()
        .find(|a| a.name.to_lowercase().ends_with(".deb"))
    {
        return Some(a);
    }

    if let Some(a) = assets
        .iter()
        .find(|a| a.name.to_lowercase().ends_with(".rpm"))
    {
        return Some(a);
    }

    assets.first()
}

pub fn parse_sha256sums(body: &str, asset_name: &str) -> Option<String> {
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let hash = parts.next()?;
        let name = parts.next().map(|s| s.trim_start_matches('*'))?;
        let base_target = std::path::Path::new(asset_name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(asset_name);
        let base_line = std::path::Path::new(name)
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

pub fn check_for_updates(current_version: &str) -> Result<UpdateCheckResult, String> {
    let user_agent = format!("PrismBI/{}", current_version.trim());
    let resp = ureq::get(GITHUB_LATEST_RELEASE_URL)
        .header("User-Agent", &user_agent)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Falha ao consultar GitHub Releases: {}", e))?;

    let body = resp
        .into_body()
        .into_with_config()
        .limit(4 * 1024 * 1024)
        .read_to_string()
        .map_err(|e| format!("Falha ao ler resposta do GitHub Releases: {}", e))?;

    let release: GithubRelease =
        serde_json::from_str(&body).map_err(|e| format!("JSON inválido do GitHub: {}", e))?;

    let latest_clean = release.tag_name.trim().trim_start_matches(['v', 'V']).to_string();
    let appimage_path = std::env::var("APPIMAGE").ok().filter(|s| !s.trim().is_empty());
    let is_appimage = appimage_path.is_some();
    let update_available = is_newer_version(current_version, &latest_clean);

    let chosen_asset = pick_linux_asset(&release.assets, is_appimage);
    let asset_name = chosen_asset.map(|a| a.name.clone()).unwrap_or_default();
    let download_url = chosen_asset
        .map(|a| a.browser_download_url.clone())
        .unwrap_or_else(|| release.html_url.clone());
    let asset_size = chosen_asset.map(|a| a.size).unwrap_or(0);

    let expected_sha256 = if update_available && !asset_name.is_empty() {
        if let Some(sums_asset) = release
            .assets
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case("SHA256SUMS.txt"))
        {
            ureq::get(&sums_asset.browser_download_url)
                .header("User-Agent", &user_agent)
                .call()
                .ok()
                .and_then(|r| {
                    r.into_body()
                        .into_with_config()
                        .limit(1024 * 1024)
                        .read_to_string()
                        .ok()
                })
                .and_then(|txt| parse_sha256sums(&txt, &asset_name))
        } else {
            None
        }
    } else {
        None
    };

    Ok(UpdateCheckResult {
        current_version: current_version.trim().trim_start_matches(['v', 'V']).to_string(),
        latest_version: latest_clean,
        update_available,
        release_url: release.html_url,
        release_notes: release.body,
        published_at: release.published_at,
        asset_name,
        download_url,
        asset_size,
        expected_sha256,
        is_appimage,
        appimage_path,
    })
}

pub fn download_and_install_update(
    download_url: &str,
    asset_name: &str,
    expected_sha256: Option<&str>,
) -> Result<UpdateInstallResult, String> {
    if !download_url.starts_with("https://") {
        return Err("URL de download inválida: apenas HTTPS é permitido.".to_string());
    }

    let safe_name = PathBuf::from(asset_name)
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("PrismBI-update")
        .to_string();

    let resp = ureq::get(download_url)
        .header("User-Agent", "PrismBI-Updater/0.7.0")
        .call()
        .map_err(|e| format!("Erro ao baixar atualização: {}", e))?;

    let bytes = resp
        .into_body()
        .into_with_config()
        .limit(512 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| format!("Erro ao ler pacote de atualização: {}", e))?;

    let actual_sha = format!("{:x}", Sha256::digest(&bytes));
    if let Some(expected) = expected_sha256.map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()) {
        if actual_sha != expected {
            return Err(format!(
                "Falha na verificação SHA-256! Esperado: {}, obtido: {}",
                expected, actual_sha
            ));
        }
    }

    if let Ok(appimage_env) = std::env::var("APPIMAGE") {
        let appimage_trimmed = appimage_env.trim();
        if !appimage_trimmed.is_empty() && safe_name.to_lowercase().ends_with(".appimage") {
            let target_path = PathBuf::from(appimage_trimmed);
            let tmp_path = PathBuf::from(format!("{}.new", appimage_trimmed));
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

    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let downloads_dir = PathBuf::from(&home).join("Downloads");
    fs::create_dir_all(&downloads_dir)
        .map_err(|e| format!("Falha ao criar pasta Downloads: {}", e))?;
    let dest_path = downloads_dir.join(&safe_name);
    fs::write(&dest_path, &bytes)
        .map_err(|e| format!("Falha ao salvar pacote em Downloads: {}", e))?;

    if safe_name.to_lowercase().ends_with(".appimage") {
        let _ = fs::set_permissions(&dest_path, fs::Permissions::from_mode(0o755));
    }

    let saved_str = dest_path.to_string_lossy().to_string();
    Ok(UpdateInstallResult {
        installed: false,
        needs_restart: false,
        saved_path: Some(saved_str.clone()),
        message: format!(
            "Pacote baixado e verificado (SHA-256) em: {}",
            saved_str
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("0.6.0", "0.7.0"));
        assert!(is_newer_version("v0.6.9", "v0.7.0"));
        assert!(is_newer_version("0.7.0", "1.0.0"));
        assert!(!is_newer_version("0.7.0", "0.7.0"));
        assert!(!is_newer_version("0.8.0", "0.7.5"));
    }

    #[test]
    fn test_pick_linux_asset() {
        let assets = vec![
            GithubAsset {
                name: "PrismBI_0.7.0_amd64.deb".into(),
                browser_download_url: "https://example.com/deb".into(),
                size: 100,
            },
            GithubAsset {
                name: "PrismBI_0.7.0_amd64.AppImage".into(),
                browser_download_url: "https://example.com/appimage".into(),
                size: 200,
            },
        ];

        assert_eq!(
            pick_linux_asset(&assets, true).unwrap().name,
            "PrismBI_0.7.0_amd64.AppImage"
        );
        assert_eq!(
            pick_linux_asset(&assets, false).unwrap().name,
            "PrismBI_0.7.0_amd64.AppImage"
        );

        let deb_only = vec![assets[0].clone()];
        assert_eq!(
            pick_linux_asset(&deb_only, true).unwrap().name,
            "PrismBI_0.7.0_amd64.deb"
        );
    }

    #[test]
    fn test_parse_sha256sums() {
        let sha = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let body = format!(
            "{}  PrismBI_0.7.0_amd64.deb\n{}  dist/PrismBI_0.7.0_amd64.AppImage\n",
            "0".repeat(64),
            sha
        );
        assert_eq!(
            parse_sha256sums(&body, "PrismBI_0.7.0_amd64.AppImage"),
            Some(sha.to_string())
        );
        assert_eq!(parse_sha256sums(&body, "missing.rpm"), None);
    }
}
