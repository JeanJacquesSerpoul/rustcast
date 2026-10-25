# RustCast

Gestionnaire de podcasts de bureau en **Rust + Tauri 2 + Leptos 0.8**, bilingue français / anglais.

## Fonctionnalités

- Recherche Apple Podcasts, classements par pays, ajout direct d'une URL de flux RSS
- Aperçu d'un podcast avant abonnement, actualisation automatique toutes les heures
- Lecture en streaming ou hors ligne, reprise à la position exacte, vitesse 0,75× → 2,5×
- Podcasts vidéo : aperçu dans la barre de lecture, grande vue et plein écran (double-clic ou touche F)
- Téléchargements en tâche de fond (3 en parallèle) avec progression et annulation
- File d'attente réorganisable (glisser-déposer), lecture enchaînée
- Vue « En lecture » plein écran, notes d'épisode, touches multimédia du système
- Import / export OPML, thème sombre ou clair, raccourcis clavier

## Installation

Les scripts installent les prérequis manquants, compilent RustCast et l'intègrent au système. Comptez quelques minutes la première fois.

### Windows

Double-cliquez sur `scripts\install.cmd`, ou dans PowerShell :

```powershell
.\scripts\install.ps1
```

- Installe si besoin Visual Studio C++ Build Tools, WebView2 et Rust (via winget, sinon par téléchargement officiel)
- Installe RustCast dans `%LOCALAPPDATA%\Programs\RustCast`, sans droits administrateur
- Crée les raccourcis du menu Démarrer et du bureau, et l'entrée dans *Paramètres > Applications*

Options : `-SkipDeps`, `-Bundle` (installateurs MSI/NSIS), `-NoDesktopShortcut`, `-NoShortcuts`, `-InstallDir <dossier>`, `-Yes`, `-Uninstall [-Purge]`.

### Linux

```bash
./scripts/install.sh
```

- Installe les dépendances (WebKitGTK 4.1, GStreamer pour l'audio…) avec apt, dnf, pacman ou zypper
- Installe Rust via rustup si besoin, puis RustCast dans `~/.local` avec icône et entrée de menu

Options : `--system` (dans `/usr/local`), `--prefix <dossier>`, `--skip-deps`, `--bundle` (paquets .deb, .rpm, AppImage), `--uninstall [--purge]`.

La désinstallation conserve vos abonnements et téléchargements, sauf avec `--purge` / `-Purge`.

## Développement

Prérequis : `rustup target add wasm32-unknown-unknown`, `cargo install trunk tauri-cli`.

```bash
cargo tauri dev
```

```bash
cargo tauri build
```

Tests (les tests réseau sont marqués `#[ignore]`) :

```bash
cargo test -p rustcast -- --include-ignored
```

## Structure

- `shared/` — types communs (sérialisés entre backend et interface)
- `src-tauri/` — backend : flux RSS (`feed.rs`), stockage JSON (`store.rs`), téléchargements, recherche, OPML
- `src/` — interface Leptos : pages, composants, lecteur audio, traductions (`i18n.rs`)
- `styles/main.css` — design

Les données sont stockées dans `%APPDATA%\io.github.jeanjacquesserpoul.rustcast` sous Windows et `~/.local/share/io.github.jeanjacquesserpoul.rustcast` sous Linux (bibliothèque `library.json` et dossier `downloads`). Les données des premières versions (`com.rustcast.app`) y sont déplacées automatiquement au lancement.

## Licence

Distribué sous licence [MIT](LICENSE).
