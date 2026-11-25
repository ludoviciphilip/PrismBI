#!/usr/bin/env bash
# Pós-processa o AppImage gerado pelo Tauri: adiciona o hook
# packaging/appimage/prismbi-system-webkit.sh (usar o WebKitGTK do sistema
# quando compatível) e reempacota com appimagetool.
#
# Uso: scripts/appimage-postprocess.sh <arquivo.AppImage> [appimagetool]
set -euo pipefail

appimage="$(readlink -f "$1")"
tool="${2:-appimagetool}"
root="$(cd "$(dirname "$0")/.." && pwd)"
hook_src="$root/packaging/appimage/prismbi-system-webkit.sh"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cd "$work"
APPIMAGE_EXTRACT_AND_RUN=1 "$appimage" --appimage-extract >/dev/null
appdir="$work/squashfs-root"

install -Dm755 "$hook_src" "$appdir/apprun-hooks/00-prismbi-system-webkit.sh"

apprun="$appdir/AppRun"
if ! grep -q '00-prismbi-system-webkit.sh' "$apprun"; then
    # Insere o nosso hook antes do primeiro hook existente (o do GTK).
    python3 - "$apprun" <<'PY'
import sys, re
p = sys.argv[1]
s = open(p).read()
line = 'source "$this_dir"/apprun-hooks/"00-prismbi-system-webkit.sh"\n'
m = re.search(r'^source "\$this_dir"/apprun-hooks/', s, re.M)
if not m:
    m = re.search(r'^exec "\$this_dir"/AppRun.wrapped', s, re.M)
if not m:
    sys.exit("AppRun com formato inesperado: hook não inserido")
s = s[:m.start()] + line + s[m.start():]
open(p, "w").write(s)
PY
fi
grep -q '00-prismbi-system-webkit.sh' "$apprun"

chmod -R a+rX "$appdir"

out="$work/out.AppImage"
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$tool" --no-appstream "$appdir" "$out" >/dev/null
chmod +x "$out"
mv -f "$out" "$appimage"
echo "AppImage pós-processado: $appimage"
