#!/bin/bash
# Desinstalación completa de BDJ Studio Audio Analyzer en macOS.
#
# macOS no tiene un desinstalador: arrastrar la app a la Papelera deja la
# carpeta de datos en ~/Library/Application Support. Este script elimina la
# app y todos los datos que Audio Analyzer haya creado (base de datos, ajustes,
# almacén cifrado, cachés y preferencias), según la política del producto:
# nada sobrevive a la desinstalación.
#
# Uso:
#     bash tools/macos/uninstall.sh
#     bash tools/macos/uninstall.sh --sin-app   # deja la app; solo borra datos

set -u

APP="/Applications/BDJ Studio Audio Analyzer.app"
BUNDLE_ID="com.bdjstudio.bdjStudioAudioAnalyzer"
SUPPORT="$HOME/Library/Application Support"

KEEP_APP="${1:-}"

DATA_DIRS=(
  "$SUPPORT/BDJ Studio Audio Analyzer"
  "$SUPPORT/BDJ Studio/BDJ Studio Audio Analyzer"
  "$SUPPORT/bdj_studio_audio_analyzer"
  "$SUPPORT/$BUNDLE_ID"
  "$HOME/Library/Caches/BDJ Studio Audio Analyzer"
  "$HOME/Library/Caches/$BUNDLE_ID"
)

remove_if_present() {
  if [ -e "$1" ] || [ -L "$1" ]; then
    echo "  -> borrando $1"
    rm -rf "$1"
  fi
}

echo "Desinstalando BDJ Studio Audio Analyzer..."

if [ "$KEEP_APP" != "--sin-app" ]; then
  echo "1. Eliminando la aplicación..."
  remove_if_present "$APP"
else
  echo "1. Conservando la aplicación (modo --sin-app)."
fi

echo "2. Eliminando datos de usuario..."
for dir in "${DATA_DIRS[@]}"; do
  remove_if_present "$dir"
done

echo "Desinstalación de BDJ Studio Audio Analyzer completada."
