# Release für GitHub

## 1. DMG bauen

```sh
./release/make-dmg.sh
```

Ergebnis in `dist/`:

- `hmb-<version>-macos-arm64.dmg` – das DMG mit **hmb.app** und einer Verknüpfung „Programme“
- `hmb-<version>-macos-arm64.dmg.sha256` – Prüfsumme

Die Version steht in `Cargo.toml` (`version = "…"`). Für ein neues Release dort erhöhen und das Skript
erneut ausführen.

**Auch für Intel-Macs (Universal):** einmalig `rustup target add x86_64-apple-darwin`, danach baut das
Skript automatisch ein Universal-DMG (`…-macos-universal.dmg`).

## 2. Auf GitHub veröffentlichen

Im Browser: Repository › **Releases** › **Draft a new release** › Tag `v0.1.0` anlegen › DMG und
`.sha256`-Datei hineinziehen › Text aus `release/NOTES-0.1.0.md` einfügen › **Publish release**.

Oder im Terminal mit der GitHub-CLI:

```sh
gh release create v0.1.0 \
  dist/hmb-0.1.0-macos-arm64.dmg dist/hmb-0.1.0-macos-arm64.dmg.sha256 \
  --title "hmb 0.1.0" --notes-file release/NOTES-0.1.0.md
```

## 3. Signatur und erster Start

Ohne Apple-Entwicklerzertifikat ist die App nur „ad hoc“ signiert. Sie läuft, aber macOS fragt beim
ersten Start nach, weil sie aus dem Internet kommt:

- **Rechtsklick auf hmb.app › Öffnen › Öffnen**, oder
- **Systemeinstellungen › Datenschutz & Sicherheit › „Trotzdem öffnen“**, oder im Terminal:
  `xattr -dr com.apple.quarantine /Applications/hmb.app`

Mit einem „Developer ID“-Zertifikat entfällt das (zusätzlich beglaubigen mit `xcrun notarytool`):

```sh
SIGN_ID="Developer ID Application: Name (TEAMID)" ./release/make-dmg.sh
```
