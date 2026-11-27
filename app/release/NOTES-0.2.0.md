## hmb 0.2.0 – Hugo Microblogger

Neue Version für macOS (Apple Silicon).

**Installieren:** DMG öffnen und **hmb** auf „Programme“ ziehen (eine vorhandene Version ersetzen).
Beim ersten Start einmal *Rechtsklick › Öffnen* (die App ist nicht über den Apple-Store signiert).
Deine bisherigen Einstellungen werden automatisch zum ersten Profil.

### Neu

- **Profile:** mehrere Hugo-Seiten in einer App. Jedes Profil hat einen Titel (bis 32 Zeichen),
  einen eigenen Datenordner, einen eigenen Sync-Ordner, einen eigenen Sync-Stand und ein eigenes
  Git-Repository. Gewechselt wird über das Auswahlfeld oben links; „+ Neues Profil …“ legt ein
  weiteres an.
- **Git direkt in der App:** Der Knopf „Git“ oben rechts öffnet Pull, Commit und Push für den Blog.
  Das Fenster zeigt, was seit dem letzten Commit neu, geändert oder gelöscht ist, und schlägt die
  Commit-Nachricht vor – erste Zeile ist der zuletzt geänderte Eintrag. Die Meldungen von Git
  (auch vom Push) stehen unten im Fenster. Standardmäßig werden nur Blog-Inhalte committet.
- Einstellung „Git-Repository“ (leer = Repository des Sync-Ordners).

### Behoben

- Liegt der Sync-Ordner in einem Unterordner von `content` (z. B. `content/post`), landeten
  Transaktionen, `snippets.json` und `tags.json` in `content/`. Sie liegen jetzt immer neben `content`
  im Ordner der Hugo-Seite; vorhandene Dateien werden beim ersten Start verschoben.

Alle Änderungen: `app/changelog.md`
