#!/usr/bin/env bash
# Publica packaging/aur/PKGBUILD no AUR como prismbi-bin.
# Pré-requisitos (uma vez): conta em https://aur.archlinux.org com a chave
# pública ~/.ssh/aur.pub cadastrada e ~/.ssh/config apontando para ela.
# Uso: scripts/publish-aur.sh   (depois de scripts/update-aur-pkgbuild.sh <versão>)
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
work="${AUR_WORKDIR:-$HOME/.cache/prismbi-aur}"
[ -d "$work/.git" ] || git clone ssh://aur@aur.archlinux.org/prismbi-bin.git "$work"
cp "$root/packaging/aur/PKGBUILD" "$work/PKGBUILD"
cd "$work"
makepkg -f --noconfirm >/dev/null        # garante que o pacote compila
makepkg --printsrcinfo > .SRCINFO
ver="$(sed -n 's/^\tpkgver = //p' .SRCINFO)-$(sed -n 's/^\tpkgrel = //p' .SRCINFO)"
git add PKGBUILD .SRCINFO
git -c user.name="Luis Felipe de Almeida Vieira" -c user.email="ludoviciphilip@users.noreply.github.com" \
  commit -qm "Update to $ver" || { echo "nada a publicar"; exit 0; }
git push -q origin HEAD:master
echo "publicado prismbi-bin $ver — https://aur.archlinux.org/packages/prismbi-bin"
