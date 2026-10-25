#!/usr/bin/env bash
# Installation de RustCast sous Linux : dépendances, compilation et intégration au bureau.
#
#   ./scripts/install.sh              installe pour l'utilisateur courant (~/.local)
#   ./scripts/install.sh --system     installe pour tous (/usr/local, nécessite sudo)
#   ./scripts/install.sh --uninstall  désinstalle (ajouter --purge pour effacer les données)
#   ./scripts/install.sh --help       toutes les options

set -euo pipefail

APP_ID="io.github.jeanjacquesserpoul.rustcast"
# Identifiant des premières versions, encore nettoyé par --purge.
LEGACY_APP_ID="com.rustcast.app"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

PREFIX="$HOME/.local"
SYSTEM=0
SKIP_DEPS=0
UNINSTALL=0
PURGE=0
BUNDLE=0

# ───────────────────────────── Messages bilingues ─────────────────────────────

case "${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}" in
  fr*) FR=1 ;;
  *) FR=0 ;;
esac
t() { if [ "$FR" = 1 ]; then printf '%s' "$1"; else printf '%s' "$2"; fi; }

if [ -t 1 ]; then
  BOLD=$'\e[1m'; DIM=$'\e[2m'; PURPLE=$'\e[35m'; GREEN=$'\e[32m'; YELLOW=$'\e[33m'; RED=$'\e[31m'; RESET=$'\e[0m'
else
  BOLD=""; DIM=""; PURPLE=""; GREEN=""; YELLOW=""; RED=""; RESET=""
fi
step() { printf '\n%s==>%s %s%s%s\n' "$PURPLE" "$RESET" "$BOLD" "$1" "$RESET"; }
info() { printf '    %s\n' "$1"; }
ok()   { printf '    %s✓%s %s\n' "$GREEN" "$RESET" "$1"; }
warn() { printf '    %s!%s %s\n' "$YELLOW" "$RESET" "$1" >&2; }
die()  { printf '\n%s✗ %s%s\n' "$RED" "$1" "$RESET" >&2; exit 1; }

usage() {
  cat <<EOF
$(t "Installation de RustCast" "RustCast installer")

  --system       $(t "installe dans /usr/local pour tous les utilisateurs" "install into /usr/local for all users")
  --prefix DIR   $(t "dossier d'installation (défaut : ~/.local)" "install prefix (default: ~/.local)")
  --skip-deps    $(t "n'installe pas les paquets système" "don't install system packages")
  --bundle       $(t "produit aussi les paquets .deb, .rpm et AppImage" "also build .deb, .rpm and AppImage packages")
  --uninstall    $(t "désinstalle RustCast" "uninstall RustCast")
  --purge        $(t "avec --uninstall : efface aussi abonnements et téléchargements" "with --uninstall: also delete subscriptions and downloads")
  -h, --help     $(t "affiche cette aide" "show this help")
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --system) SYSTEM=1; PREFIX="/usr/local" ;;
    --prefix) [ $# -ge 2 ] || die "--prefix ?"; PREFIX="$2"; shift ;;
    --prefix=*) PREFIX="${1#*=}" ;;
    --skip-deps) SKIP_DEPS=1 ;;
    --bundle) BUNDLE=1 ;;
    --uninstall) UNINSTALL=1 ;;
    --purge) PURGE=1 ;;
    -h|--help) usage; exit 0 ;;
    *) usage; die "$(t "Option inconnue" "Unknown option") : $1" ;;
  esac
  shift
done

[ "$(uname -s)" = "Linux" ] || die "$(t "Ce script est réservé à Linux (utilisez install.ps1 sous Windows)." "This script is for Linux only (use install.ps1 on Windows).")"

SUDO=""
if [ "$(id -u)" -ne 0 ]; then
  command -v sudo >/dev/null 2>&1 && SUDO="sudo"
fi
# Écrit dans PREFIX, avec sudo si le dossier n'est pas accessible.
as_prefix() {
  if [ -w "$PREFIX" ] || { [ ! -e "$PREFIX" ] && [ -w "$(dirname "$PREFIX")" ]; }; then "$@"; else $SUDO "$@"; fi
}

BIN="$PREFIX/bin/rustcast"
DESKTOP="$PREFIX/share/applications/rustcast.desktop"
ICONS="$PREFIX/share/icons/hicolor"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/$APP_ID"
LEGACY_DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/$LEGACY_APP_ID"

refresh_desktop() {
  command -v update-desktop-database >/dev/null 2>&1 && as_prefix update-desktop-database -q "$PREFIX/share/applications" 2>/dev/null || true
  command -v gtk-update-icon-cache >/dev/null 2>&1 && as_prefix gtk-update-icon-cache -q -t "$ICONS" 2>/dev/null || true
}

# ───────────────────────────── Désinstallation ─────────────────────────────

if [ "$UNINSTALL" = 1 ]; then
  step "$(t "Désinstallation de RustCast" "Uninstalling RustCast")"
  pkill -x rustcast 2>/dev/null || true
  for f in "$BIN" "$DESKTOP" "$ICONS"/{32x32,64x64,128x128,256x256,512x512}/apps/rustcast.png; do
    if [ -e "$f" ]; then as_prefix rm -f "$f"; ok "$f"; fi
  done
  refresh_desktop
  if [ "$PURGE" = 1 ]; then
    for dir in "$DATA_DIR" "$LEGACY_DATA_DIR"; do
      if [ -d "$dir" ]; then rm -rf "$dir" && ok "$(t "Données supprimées" "Data removed") : $dir"; fi
    done
  elif [ -d "$DATA_DIR" ]; then
    info "$(t "Vos abonnements et téléchargements sont conservés dans" "Your subscriptions and downloads are kept in") $DATA_DIR"
    info "$(t "(relancez avec --purge pour les effacer)" "(run again with --purge to delete them)")"
  fi
  printf '\n%s%s%s\n' "$GREEN" "$(t "RustCast a été désinstallé." "RustCast has been uninstalled.")" "$RESET"
  exit 0
fi

printf '%s%s RustCast %s— %s\n' "$BOLD" "$PURPLE" "$RESET" "$(t "installation" "installation")"
info "$(t "Sources" "Sources") : $ROOT"
info "$(t "Destination" "Destination") : $PREFIX"
[ -f "$ROOT/src-tauri/tauri.conf.json" ] || die "$(t "Dossier du projet introuvable." "Project folder not found.")"

# ───────────────────────────── Dépendances système ─────────────────────────────

install_deps() {
  local pm="" pkgs=() optional=()
  if command -v apt-get >/dev/null 2>&1; then
    pm=apt
    pkgs=(build-essential curl wget file pkg-config libssl-dev libwebkit2gtk-4.1-dev libxdo-dev
          libayatana-appindicator3-dev librsvg2-dev gstreamer1.0-plugins-base gstreamer1.0-plugins-good)
    optional=(gstreamer1.0-libav gstreamer1.0-plugins-bad)
  elif command -v dnf >/dev/null 2>&1; then
    pm=dnf
    pkgs=(gcc gcc-c++ make curl wget file pkgconf-pkg-config openssl-devel webkit2gtk4.1-devel libxdo-devel
          libappindicator-gtk3-devel librsvg2-devel gstreamer1-plugins-base gstreamer1-plugins-good)
    optional=(gstreamer1-plugin-libav gstreamer1-plugins-bad-free)
  elif command -v pacman >/dev/null 2>&1; then
    pm=pacman
    pkgs=(base-devel curl wget file openssl webkit2gtk-4.1 xdotool libappindicator-gtk3 librsvg
          gst-plugins-base gst-plugins-good)
    optional=(gst-libav gst-plugins-bad)
  elif command -v zypper >/dev/null 2>&1; then
    pm=zypper
    pkgs=(gcc gcc-c++ make curl wget file pkg-config libopenssl-devel webkit2gtk3-devel
          libappindicator3-1 librsvg-devel gstreamer-plugins-base gstreamer-plugins-good)
    optional=(gstreamer-plugins-libav gstreamer-plugins-bad)
  else
    warn "$(t "Gestionnaire de paquets non reconnu : installez WebKitGTK 4.1, OpenSSL, librsvg et GStreamer manuellement." \
              "Unknown package manager: install WebKitGTK 4.1, OpenSSL, librsvg and GStreamer manually.")"
    return
  fi
  if [ "$(id -u)" -ne 0 ] && [ -z "$SUDO" ]; then
    die "$(t "sudo est nécessaire pour installer les dépendances (ou utilisez --skip-deps)." "sudo is required to install dependencies (or use --skip-deps).")"
  fi
  info "$(t "Gestionnaire de paquets" "Package manager") : $pm"
  case "$pm" in
    apt)
      $SUDO apt-get update -qq
      DEBIAN_FRONTEND=noninteractive $SUDO apt-get install -y -qq "${pkgs[@]}"
      for p in "${optional[@]}"; do DEBIAN_FRONTEND=noninteractive $SUDO apt-get install -y -qq "$p" >/dev/null 2>&1 || warn "$p"; done ;;
    dnf)
      $SUDO dnf install -y -q "${pkgs[@]}"
      for p in "${optional[@]}"; do $SUDO dnf install -y -q "$p" >/dev/null 2>&1 || warn "$p"; done ;;
    pacman)
      $SUDO pacman -S --needed --noconfirm "${pkgs[@]}"
      for p in "${optional[@]}"; do $SUDO pacman -S --needed --noconfirm "$p" >/dev/null 2>&1 || warn "$p"; done ;;
    zypper)
      $SUDO zypper --non-interactive install "${pkgs[@]}"
      for p in "${optional[@]}"; do $SUDO zypper --non-interactive install "$p" >/dev/null 2>&1 || warn "$p"; done ;;
  esac
  ok "$(t "Dépendances système installées" "System dependencies installed")"
}

if [ "$SKIP_DEPS" = 1 ]; then
  step "$(t "Dépendances système ignorées (--skip-deps)" "Skipping system dependencies (--skip-deps)")"
else
  step "$(t "Dépendances système" "System dependencies")"
  install_deps
fi

# ───────────────────────────── Chaîne Rust ─────────────────────────────

step "$(t "Chaîne de compilation Rust" "Rust toolchain")"
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
if ! command -v cargo >/dev/null 2>&1; then
  info "$(t "Installation de Rust via rustup…" "Installing Rust with rustup…")"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  . "$HOME/.cargo/env"
fi
ok "$(cargo --version)"

if command -v rustup >/dev/null 2>&1; then
  rustup target add wasm32-unknown-unknown >/dev/null
  ok "wasm32-unknown-unknown"
else
  warn "$(t "rustup absent : vérifiez que la cible wasm32-unknown-unknown est installée." "rustup missing: make sure the wasm32-unknown-unknown target is installed.")"
fi

cargo_tool() {
  local bin="$1" crate="$2" version="$3"
  if command -v "$bin" >/dev/null 2>&1; then
    ok "$crate ($("$bin" --version 2>/dev/null | head -n1))"
    return
  fi
  info "$(t "Installation de" "Installing") $crate…"
  if command -v cargo-binstall >/dev/null 2>&1; then
    cargo binstall -y --locked "$crate@$version"
  else
    cargo install --locked "$crate" --version "$version"
  fi
  ok "$crate"
}
cargo_tool trunk trunk "^0.21"
cargo_tool cargo-tauri tauri-cli "^2"

# ───────────────────────────── Compilation ─────────────────────────────

step "$(t "Compilation de RustCast (quelques minutes la première fois)" "Building RustCast (a few minutes the first time)")"
cd "$ROOT"
# trunk n'accepte que true/false pour NO_COLOR, alors que la convention usuelle est NO_COLOR=1.
case "${NO_COLOR:-}" in ""|true|false) ;; *) export NO_COLOR=true ;; esac
if [ "$BUNDLE" = 1 ]; then
  cargo tauri build --bundles deb,rpm,appimage
else
  cargo tauri build --no-bundle
fi
BUILT="$ROOT/target/release/rustcast"
[ -x "$BUILT" ] || die "$(t "Binaire introuvable après compilation" "Binary not found after build") : $BUILT"
ok "$BUILT"

# ───────────────────────────── Installation ─────────────────────────────

step "$(t "Installation dans" "Installing into") $PREFIX"
pkill -x rustcast 2>/dev/null || true
as_prefix install -Dm755 "$BUILT" "$BIN"
ok "$BIN"

for spec in 32x32:32x32 64x64:64x64 128x128:128x128 256x256:128x128@2x 512x512:icon; do
  size="${spec%%:*}" file="${spec#*:}"
  as_prefix install -Dm644 "$ROOT/src-tauri/icons/$file.png" "$ICONS/$size/apps/rustcast.png"
done
ok "$(t "Icônes" "Icons")"

tmp_desktop="$(mktemp)"
cat >"$tmp_desktop" <<EOF
[Desktop Entry]
Type=Application
Name=RustCast
GenericName=Podcast Player
GenericName[fr]=Lecteur de podcasts
Comment=Subscribe, download and listen to podcasts
Comment[fr]=Abonnez-vous, téléchargez et écoutez vos podcasts
Exec=$BIN
Icon=rustcast
Terminal=false
Categories=AudioVideo;Audio;Player;
Keywords=podcast;rss;audio;player;baladodiffusion;
StartupWMClass=rustcast
EOF
as_prefix install -Dm644 "$tmp_desktop" "$DESKTOP"
rm -f "$tmp_desktop"
refresh_desktop
ok "$DESKTOP"

if [ "$BUNDLE" = 1 ]; then
  info "$(t "Paquets générés dans" "Packages written to") $ROOT/target/release/bundle/"
fi

printf '\n%s%s%s\n' "$GREEN$BOLD" "$(t "RustCast est installé !" "RustCast is installed!")" "$RESET"
info "$(t "Lancez-le depuis le menu des applications, ou avec :" "Launch it from your applications menu, or with:") $BIN"
case ":$PATH:" in
  *":$PREFIX/bin:"*) ;;
  *) warn "$(t "$PREFIX/bin n'est pas dans votre PATH." "$PREFIX/bin is not in your PATH.")" ;;
esac
info "${DIM}$(t "Fenêtre vide avec un pilote NVIDIA ? Lancez :" "Blank window with an NVIDIA driver? Run:") WEBKIT_DISABLE_DMABUF_RENDERER=1 rustcast${RESET}"
info "${DIM}$(t "Désinstaller :" "Uninstall:") $ROOT/scripts/install.sh --uninstall$([ "$SYSTEM" = 1 ] && echo " --system" || true)${RESET}"
