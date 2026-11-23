#!/bin/sh
# Erzeugt die Bilder für doc/README.md neu.
# Baut dafür in einem Temp-Ordner einen Beispiel-Blog und startet hmb mit diesem Ordner als HOME –
# deine echten Einstellungen und Artikel werden nicht angefasst.
set -e
cd "$(dirname "$0")/.."
cargo build -q
BIN="$(pwd)/target/debug/hmb"
IMG="$(pwd)/doc/img"
for view in main settings snippets editor sync; do
  T="$(mktemp -d)"
  HOME="$T" "$BIN" --doc-fixture "$T" > /dev/null
  case "$view" in
    main) name=00-uebersicht ;;
    settings) name=01-einstellungen ;;
    snippets) name=02-snippets ;;
    editor) name=03-editor ;;
    sync) name=04-synchronisieren ;;
  esac
  HOME="$T" "$BIN" --screenshot "$IMG/$name.png" --doc-view "$view"
  rm -rf "$T"
done
