# hmb – Hugo Microblogger

**hmb** steht für **Hugo Microblogger**: eine kleine Desktop-App, um kurze Markdown-Artikel für einen
Hugo-Blog zu schreiben, mit Tags zu ordnen und zwischen Ordnern abzugleichen.

**Einführung mit Screenshots:** [doc/README.md](doc/README.md)

## Was hmb kann

- **Liste mit 10 Einträgen pro Seite** – Überschrift, Datum, Tags und ein Abstract von höchstens 3 Zeilen.
- **Haupttags links**, aufklappbar nach Ober- und Unterbegriffen („Tags ordnen“); oben Suche nach Tags.
- **Editor mit Vorschau** – Titel, Datum, Tags, Entwurf, Abstract und Artikel in Markdown.
- **Bilder** über „🖼 Bild“ oder per Drag-and-drop: Sie werden in den Ordner des Artikels kopiert, und
  der passende Hugo-Tag (`{{< figure … >}}`) oder Markdown-Bild-Tag wird eingefügt.
- **Snippets** – kleine Textbausteine, auch systemweit per Rechtsklick → Dienste → „Als hmb-Snippet speichern“.
- **Menüleisten-Symbol**, um hmb jederzeit aufzurufen.

## Profile (mehrere Hugo-Seiten)

Oben links wählst du das **Profil** – eine Hugo-Seite mit Titel (bis 32 Zeichen), eigenem Datenordner,
eigenem Sync-Ordner, eigenem Sync-Stand und eigenem Git-Repository. Beim Wechsel liest hmb die Artikel
aus dem Datenordner dieses Profils und gleicht mit dessen Sync-Ordner ab. „+ Neues Profil …“ legt ein
Profil mit eigenem Datenordner an; entfernen lässt es sich in den Einstellungen (die Ordner bleiben).

## Git: Pull, Commit, Push

Der Knopf **Git** oben rechts öffnet das Git-Fenster für das Repository des Blogs (das Repository, in
dem der Sync-Ordner liegt, oder das in den Einstellungen eingetragene):

1. **Pull** holt Neues vom Server (`git pull --rebase --autostash`); danach „Jetzt synchronisieren“.
2. **Commit**: hmb listet, was seit dem letzten Commit neu, geändert und gelöscht ist, und schlägt
   die Nachricht vor – erste Zeile ist der zuletzt geänderte Eintrag, darunter die drei Listen.
   Standardmäßig nur Blog-Inhalte (content, transactions, Snippets, Tags).
3. **Push** schickt die Commits zum Server; die Meldung von Git steht unten im Fenster.

Git nutzt die Anmeldung dieses Rechners (SSH-Schlüssel oder Schlüsselbund) und fragt nie nach
einem Passwort – fehlt die Anmeldung, steht die Fehlermeldung in der Ausgabe.

## Ordnerstruktur (einstellbar)

- **Nach Datum:** `JJJJ/MM/TT/<name>/index.md`, Abstract im Frontmatter (`abstract:`).
- **Nach Bereichen (Hugo):** `<bereich>/<unterbereich>/<name>/index.md`, Abstract vor `<!--more-->`.
  Weitere Frontmatter-Felder (categories, author, toc …) bleiben erhalten.

Jeder Artikel ist ein Hugo Page Bundle: `index.md` und seine Bilder liegen im selben Ordner.
Die ID besteht aus Jahr, Monat, Tag und dem Ordnerpfad, z. B. `2026-10-11-aws-mock-overcast`.

## Synchronisieren (nur lokale Ordner)

hmb gleicht den Datenordner mit einem zweiten lokalen **Sync-Ordner** ab (z. B. dem content-Ordner des
Hugo-Blogs). Um den Sync-Ordner selbst kümmert sich Git oder ein anderes Werkzeug – hmb macht keine
Netzwerkzugriffe.

- **Synchronisieren:** liest im Ordner `transactions/` **neben** dem Sync-Ordner (auf derselben Ebene
  wie `content`, nie darin) nur die neuen Dateien `transaction-JJJJ-MM-TT-HH-mm.json`, beginnend beim zuletzt verarbeiteten Zeitstempel (es kann mehrere
  Dateien mit demselben Zeitstempel geben). Die dort genannten Artikelordner werden übernommen: neue
  importiert, vorhandene überschrieben. Danach werden die eigenen Änderungen in den Sync-Ordner kopiert
  und in einer neuen Transaktionsdatei beschrieben. Wurde ein Artikel auf beiden Seiten geändert,
  gewinnt die lokale Fassung.
- **Vollsynchronisierung** (in den Einstellungen): vergleicht alle Artikel in beiden Ordnern, holt
  und schreibt Fehlendes, bei Unterschieden gewinnt die neuere Fassung.

Aufbau neben dem Content-Ordner (gilt für die App und für den Sync-Ordner):

```
blog/
├── content/                 nur Artikel (Page Bundles)
├── transactions/            nur flache Dateien, keine Unterordner
│   ├── transaction-2026-11-23-09-41.json
│   └── transaction-2026-11-23-09-41-laptop.json   (zweiter Rechner, gleiche Minute)
├── snippets.json
└── tags.json
```

Ältere Versionen haben diese Dateien in `content/` abgelegt (teils in Jahr/Monat-Ordnern); hmb räumt
sie beim Start und vor jedem Abgleich automatisch dorthin um.

Eine Transaktionsdatei sieht so aus:

```json
{
  "created": "2026-11-23T09:41:00+01:00",
  "client": "mac",
  "entries": [
    { "time": "…", "id": "2026-11-20-build-gplan", "action": "updated",
      "path": "build/gplan", "title": "GPlan", "client": "mac" }
  ]
}
```

## Kommandozeile

```
hmb                         Fenster öffnen
hmb --sync                  einmal synchronisieren (HMB_PROFILE=<Titel> wählt ein Profil)
hmb --full-sync             Vollsynchronisierung
hmb --snippet [Text]        Snippet speichern (ohne Text: von der Standardeingabe)
hmb --screenshot bild.png   Bild des Fensters speichern und beenden
```

Einstellungen liegen unter `~/Library/Application Support/hmb/config.json`.

## Installieren (Mac)

DMG aus dem GitHub-Release laden, öffnen und **hmb** auf „Programme“ ziehen. Selbst bauen:
`./release/make-dmg.sh` – Details in [release/README.md](release/README.md).

## Bauen

```
cargo build --release
```
