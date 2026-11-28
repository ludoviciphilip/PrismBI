#!/usr/bin/env bash
# Atualiza packaging/aur/PKGBUILD para uma release publicada: pkgver e o
# sha256 do .deb, lido do SHA256SUMS.txt depois de verificar a assinatura
# Ed25519 com a chave pública embutida no app.
# Uso: scripts/update-aur-pkgbuild.sh 0.8.1
set -euo pipefail
ver="${1:?versão, ex.: 0.8.1}"
root="$(cd "$(dirname "$0")/.." && pwd)"
pkgbuild="$root/packaging/aur/PKGBUILD"
pub_hex="$(grep -oE 'UPDATE_PUBLIC_KEY_HEX: &str =\s*"[0-9a-f]{64}"' -A1 "$root/src-tauri/src/updater.rs" | grep -oE '[0-9a-f]{64}' | head -1 || true)"
[ -n "$pub_hex" ] || pub_hex="$(grep -A1 'UPDATE_PUBLIC_KEY_HEX' "$root/src-tauri/src/updater.rs" | grep -oE '[0-9a-f]{64}' | head -1)"

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
base="https://github.com/ludoviciphilip/VetroBI/releases/download/v${ver}"
curl -fsSL -o "$tmp/SHA256SUMS.txt" "$base/SHA256SUMS.txt"
curl -fsSL -o "$tmp/SHA256SUMS.txt.sig" "$base/SHA256SUMS.txt.sig"

# Chave pública Ed25519 crua -> PEM (prefixo DER SubjectPublicKeyInfo fixo).
printf '302a300506032b6570032100%s' "$pub_hex" | xxd -r -p > "$tmp/pub.der" 2>/dev/null \
  || python3 -c "import sys;open('$tmp/pub.der','wb').write(bytes.fromhex('302a300506032b6570032100$pub_hex'))"
openssl pkey -pubin -inform DER -in "$tmp/pub.der" -out "$tmp/pub.pem"
openssl pkeyutl -verify -pubin -inkey "$tmp/pub.pem" -rawin \
  -in "$tmp/SHA256SUMS.txt" -sigfile "$tmp/SHA256SUMS.txt.sig" >/dev/null

deb="VetroBI_${ver}_amd64.deb"
sum="$(awk -v f="$deb" '$2==f || $2=="*"f {print $1}' "$tmp/SHA256SUMS.txt")"
[ ${#sum} -eq 64 ] || { echo "sha256 de $deb não encontrado" >&2; exit 1; }

lic="$(curl -fsSL "https://raw.githubusercontent.com/ludoviciphilip/VetroBI/v${ver}/LICENSE.md" | sha256sum | cut -d' ' -f1)"

python3 - "$pkgbuild" "$ver" "$sum" "$lic" <<'PY'
import re, sys
p, ver, s, lic = sys.argv[1:]
t = open(p).read()
t = re.sub(r'^pkgver=.*$', f'pkgver={ver}', t, flags=re.M)
t = re.sub(r'^pkgrel=.*$', 'pkgrel=1', t, flags=re.M)
t = re.sub(r"sha256sums=\('[^']*'", f"sha256sums=('{s}'", t)
t = re.sub(r"(sha256sums=\('[0-9a-f]{64}'\s*\n\s*)'[^']*'", rf"\g<1>'{lic}'", t)
open(p, 'w').write(t)
PY
echo "PKGBUILD -> $ver ($sum), assinatura verificada"
