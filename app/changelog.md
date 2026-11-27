# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]


## [0.2.0] - 2026-11-27

### Added

- Profile: mehrere Hugo-Seiten mit eigenem Titel (bis 32 Zeichen), eigenem Datenordner, Sync-Ordner,
  Sync-Stand und Git-Repository. Wechsel über das Auswahlfeld oben links; neue Profile über
  „+ Neues Profil …“, Entfernen in den Einstellungen (Ordner bleiben erhalten).
  Bestehende Einstellungen werden automatisch zum ersten Profil.
- Git-Fenster (Knopf „Git“ oben rechts): Pull, Commit und Push für das Repository des Blogs.
  Zeigt Branch, was seit dem letzten Commit neu, geändert oder gelöscht ist, und die Ausgabe von Git.
  Die Commit-Nachricht wird vorgeschlagen: erste Zeile ist der zuletzt geänderte Eintrag, darunter
  die Listen „Neu“, „Geändert“, „Gelöscht“. Standardmäßig werden nur Blog-Inhalte committet
  (content, transactions, Snippets, Tags).
- Einstellung „Git-Repository“ (leer = Repository des Sync-Ordners).

### Fixed

- Liegt der Sync-Ordner in einem Unterordner von `content` (z. B. `content/post`), landeten
  Transaktionen, `snippets.json` und `tags.json` in `content/`. Sie liegen jetzt immer neben `content`
  im Ordner der Hugo-Seite; vorhandene Dateien werden beim Start automatisch dorthin verschoben.
- Bestehende Einstellungen bekommen als Profil-Titel den Namen der Hugo-Seite.


## [0.1.1] - 2026-11-23

### Fixed

- Crash when you drag and drop an image directly into the app. 

## [0.1.0] - 2026-11-23

### Added

- Microblogging basic functionality 
