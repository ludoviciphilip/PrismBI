#! /usr/bin/env bash
# VetroBI: usar o WebKitGTK do sistema quando ele for compatível.
#
# O WebKitGTK embutido no AppImage vem do Ubuntu 22.04 (onde o CI compila):
# é uma versão antiga, sem o compositor com threads e sem renderização Skia
# por GPU. Em distros atuais isso deixa o scroll e os cliques no canvas
# visivelmente travados. Se o sistema tem um webkit2gtk-4.1 que resolve todos
# os símbolos do binário, executamos o app direto com as bibliotecas do
# sistema e um ambiente GTK limpo. Caso contrário, este hook não faz nada e o
# AppRun segue com o WebKit embutido.
#
# Forçar o embutido:  VETROBI_USE_BUNDLED_WEBKIT=1 ./VetroBI.AppImage
#
# Este arquivo é "sourced" pelo AppRun ANTES do hook do GTK, então ainda não
# há LD_LIBRARY_PATH/GTK_PATH apontando para dentro do AppImage.

vetrobi_try_system_webkit() {
    [ "${VETROBI_USE_BUNDLED_WEBKIT:-${PRISMBI_USE_BUNDLED_WEBKIT:-0}}" = "1" ] && return 0

    local appdir="${APPDIR:-$this_dir}"
    local bin="$appdir/usr/bin/vetrobi"
    [ -x "$bin" ] || return 0

    local sys_proc=""
    local d
    for d in /usr/lib/webkit2gtk-4.1 /usr/lib/x86_64-linux-gnu/webkit2gtk-4.1 \
             /usr/lib64/webkit2gtk-4.1 /usr/libexec/webkit2gtk-4.1; do
        if [ -x "$d/WebKitWebProcess" ]; then sys_proc="$d"; break; fi
    done
    [ -n "$sys_proc" ] || return 0
    command -v ldd >/dev/null 2>&1 || return 0

    # Diretório da libwebkit2gtk do sistema. O binário tem RUNPATH
    # $ORIGIN/../lib (as libs embutidas); LD_LIBRARY_PATH tem prioridade
    # sobre RUNPATH, então apontá-lo para as libs do sistema basta.
    local sys_lib=""
    for d in /usr/lib /usr/lib/x86_64-linux-gnu /usr/lib64; do
        if [ -e "$d/libwebkit2gtk-4.1.so.0" ]; then sys_lib="$d"; break; fi
    done
    [ -n "$sys_lib" ] || return 0

    # Todas as dependências e símbolos precisam resolver no sistema, sem
    # nenhuma lib vindo de dentro do AppImage (mistura de versões).
    local report
    report="$(env -u LD_PRELOAD LD_LIBRARY_PATH="$sys_lib" ldd -r "$bin" 2>&1)" || return 0
    if printf '%s\n' "$report" | grep -qE 'not found|undefined symbol'; then
        return 0
    fi
    printf '%s\n' "$report" | grep -q 'libwebkit2gtk-4.1' || return 0
    if printf '%s\n' "$report" | grep -q "$appdir"; then
        return 0
    fi

    export VETROBI_WEBKIT_SOURCE="system:$sys_proc"
    exec env -u LD_PRELOAD \
        -u GTK_PATH -u GTK_EXE_PREFIX -u GTK_DATA_PREFIX -u GTK_THEME \
        -u GTK_IM_MODULE_FILE -u GDK_PIXBUF_MODULE_FILE -u GIO_MODULE_DIR \
        -u GSETTINGS_SCHEMA_DIR -u GI_TYPELIB_PATH \
        LD_LIBRARY_PATH="$sys_lib" \
        "$bin" "$@"
}

vetrobi_try_system_webkit "$@"
export VETROBI_WEBKIT_SOURCE="bundled"
