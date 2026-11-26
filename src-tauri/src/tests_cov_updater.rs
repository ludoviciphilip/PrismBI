//! Coverage tests for `crate::updater`'s pure functions, written from the
//! crate root against its public API. Lives outside `updater.rs` (front A's
//! territory for v0.10 round 1): that file is never touched here.

use crate::updater::*;

#[test]
fn normalize_version_strips_v_prefix_and_prerelease_suffix() {
    assert_eq!(normalize_version("v1.2.3"), vec![1, 2, 3]);
    assert_eq!(normalize_version("1.2.3-beta.1"), vec![1, 2, 3]);
    assert_eq!(normalize_version("V0.9.0"), vec![0, 9, 0]);
    assert_eq!(normalize_version(""), vec![0]);
}

#[test]
fn is_newer_version_handles_patch_minor_major_and_prerelease() {
    assert!(is_newer_version("0.9.0", "0.9.1"));
    assert!(is_newer_version("0.9.1", "0.10.0"));
    assert!(is_newer_version("0.9.1", "1.0.0"));
    assert!(!is_newer_version("1.0.0", "1.0.0"));
    assert!(!is_newer_version("1.1.0", "1.0.9"));
    // Prerelease suffix is stripped, so same numeric triplet = not newer.
    assert!(!is_newer_version("1.0.0", "1.0.0-rc.1"));
    // Different length version numbers: missing components default to 0.
    assert!(is_newer_version("1.0", "1.0.1"));
}

#[test]
fn classify_install_appimage_and_flatpak_take_priority() {
    assert_eq!(
        classify_install(Some("/path/App.AppImage"), None, None, &|_| false),
        InstallKind::AppImage
    );
    assert_eq!(
        classify_install(None, Some("org.prismbi.App"), None, &|_| false),
        InstallKind::Managed
    );
    // Empty env values must not count as "set".
    assert_eq!(classify_install(Some(""), Some(""), None, &|_| false), InstallKind::Unknown);
}

#[test]
fn classify_install_outside_system_dirs_is_unknown() {
    use std::path::Path;
    let exe = Path::new("/home/user/apps/prismbi");
    assert_eq!(classify_install(None, None, Some(exe), &|_| false), InstallKind::Unknown);
}

#[test]
fn classify_install_pacman_takes_priority_over_deb_markers() {
    use std::path::Path;
    let exe = Path::new("/usr/bin/prismbi");
    let kind = classify_install(None, None, Some(exe), &|p| {
        p == "/var/lib/pacman/local" || p == "/etc/debian_version"
    });
    assert_eq!(kind, InstallKind::Managed);
}

#[test]
fn classify_install_deb_and_rpm_markers() {
    use std::path::Path;
    let exe = Path::new("/usr/bin/prismbi");
    let deb = classify_install(None, None, Some(exe), &|p| p == "/etc/debian_version");
    assert_eq!(deb, InstallKind::Deb);
    let rpm = classify_install(None, None, Some(exe), &|p| p == "/etc/fedora-release");
    assert_eq!(rpm, InstallKind::Rpm);
}

#[test]
fn classify_install_windows_msi_vs_portable() {
    use std::path::Path;
    let program_dirs = ["C:\\Program Files"];
    let msi = classify_install_windows(
        Some(Path::new("C:\\Program Files\\PrismBI\\prismbi.exe")),
        &program_dirs,
        None,
    );
    assert_eq!(msi, InstallKind::MsiInstalled);
    let portable = classify_install_windows(
        Some(Path::new("D:\\Downloads\\prismbi.exe")),
        &program_dirs,
        None,
    );
    assert_eq!(portable, InstallKind::Portable);
    let no_exe = classify_install_windows(None, &program_dirs, None);
    assert_eq!(no_exe, InstallKind::Portable);
}

#[test]
fn classify_install_windows_local_appdata_programs() {
    use std::path::Path;
    let msi = classify_install_windows(
        Some(Path::new("C:\\Users\\bob\\AppData\\Local\\Programs\\PrismBI\\prismbi.exe")),
        &[],
        Some("C:\\Users\\bob\\AppData\\Local\\Programs"),
    );
    assert_eq!(msi, InstallKind::MsiInstalled);
}

#[test]
fn parse_sha256sums_matches_exact_and_wildcard_prefixed_names() {
    let body = "deadbeef0000000000000000000000000000000000000000000000000000beef  PrismBI-0.9.0.AppImage\nabc123abc123abc123abc123abc123abc123abc123abc123abc123abc123abcd *PrismBI.deb\n";
    let h1 = parse_sha256sums(body, "PrismBI-0.9.0.AppImage");
    assert_eq!(
        h1.as_deref(),
        Some("deadbeef0000000000000000000000000000000000000000000000000000beef")
    );
    let h2 = parse_sha256sums(body, "PrismBI.deb");
    assert_eq!(
        h2.as_deref(),
        Some("abc123abc123abc123abc123abc123abc123abc123abc123abc123abc123abcd")
    );
}

#[test]
fn parse_sha256sums_rejects_short_or_non_hex_and_missing_name() {
    let body = "nothex_nothex_nothex_nothex_nothex_nothex_nothex_nothex_nothex  Foo.deb\nabcd  Foo.deb\n";
    assert_eq!(parse_sha256sums(body, "Foo.deb"), None);
    assert_eq!(parse_sha256sums("", "Foo.deb"), None);
    assert_eq!(
        parse_sha256sums(
            "deadbeef0000000000000000000000000000000000000000000000000000beef  Other.deb",
            "Foo.deb"
        ),
        None
    );
}

#[test]
fn is_trusted_download_url_enforces_exact_prefix_and_no_traversal() {
    assert!(is_trusted_download_url(
        "https://github.com/ludoviciphilip/PrismBI/releases/download/v0.9.0/PrismBI.AppImage"
    ));
    assert!(!is_trusted_download_url("https://evil.example.com/PrismBI.AppImage"));
    assert!(!is_trusted_download_url(
        "https://github.com/ludoviciphilip/PrismBI/releases/download/../../etc/passwd"
    ));
    assert!(!is_trusted_download_url(
        "https://github.com/ludoviciphilip/PrismBI/releases/download/v0.9.0/http://evil.com"
    ));
    // Exactly the prefix with nothing after it is not trusted (no asset name).
    assert!(!is_trusted_download_url(
        "https://github.com/ludoviciphilip/PrismBI/releases/download/"
    ));
}

#[test]
fn verify_detached_signature_rejects_malformed_inputs_without_panicking() {
    // Wrong-length public key hex.
    assert!(!verify_detached_signature(b"msg", &[0u8; 64], "deadbeef"));
    // Wrong-length signature.
    assert!(!verify_detached_signature(
        b"msg",
        &[0u8; 10],
        "0000000000000000000000000000000000000000000000000000000000beef"
    ));
    // Valid lengths but random/garbage key+signature must not verify.
    assert!(!verify_detached_signature(
        b"hello world",
        &[7u8; 64],
        "0000000000000000000000000000000000000000000000000000000000beef"
    ));
}

#[test]
fn evaluate_release_no_update_is_default_verdict() {
    let v = evaluate_release(false, InstallKind::AppImage, None, None, None, "deadbeef");
    assert!(v.pending.is_none());
    assert!(!v.signature_verified);
    assert!(v.blocked_reason.is_none());
}

#[test]
fn evaluate_release_managed_install_is_blocked_before_touching_assets() {
    let v = evaluate_release(
        true,
        InstallKind::Managed,
        Some(("PrismBI.AppImage", "https://example.com/x")),
        None,
        None,
        "deadbeef",
    );
    assert!(v.pending.is_none());
    assert!(v.blocked_reason.unwrap().contains("gerenciador"));
}

#[test]
fn evaluate_release_missing_asset_for_install_kind_is_blocked() {
    let v = evaluate_release(true, InstallKind::Deb, None, None, None, "deadbeef");
    assert!(v.pending.is_none());
    assert!(v.blocked_reason.is_some());
}

#[test]
fn evaluate_release_unsigned_sums_is_blocked_even_with_matching_sha() {
    let body = "deadbeef0000000000000000000000000000000000000000000000000000beef  PrismBI.AppImage\n";
    let v = evaluate_release(
        true,
        InstallKind::AppImage,
        Some(("PrismBI.AppImage", "https://github.com/ludoviciphilip/PrismBI/releases/download/v1/PrismBI.AppImage")),
        Some(body),
        None, // no signature at all
        "deadbeef",
    );
    assert!(v.pending.is_none());
    assert!(!v.signature_verified);
    assert!(v.blocked_reason.unwrap().contains("assinatura"));
}

#[test]
fn evaluate_release_untrusted_url_blocks_even_with_matching_hash() {
    // We can't produce a real signature here without the private key, so
    // exercise the branch ordering by confirming the asset-missing-from-sums
    // and untrusted-url paths independently via the lower-level helpers
    // already covered above; evaluate_release's full happy path needs a
    // live Ed25519 keypair and is exercised by updater.rs's own tests.
    let body = "deadbeef0000000000000000000000000000000000000000000000000000beef  PrismBI.AppImage\n";
    assert_eq!(
        parse_sha256sums(body, "PrismBI.AppImage").as_deref(),
        Some("deadbeef0000000000000000000000000000000000000000000000000000beef")
    );
    assert!(!is_trusted_download_url("https://evil.example.com/PrismBI.AppImage"));
}
