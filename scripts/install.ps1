<#
.SYNOPSIS
    Installation de RustCast sous Windows : prérequis, compilation et raccourcis.

.DESCRIPTION
    Installe si besoin les outils de compilation C++ (Visual Studio Build Tools), WebView2 et Rust,
    compile RustCast puis l'installe pour l'utilisateur courant, sans droits administrateur.

.EXAMPLE
    .\scripts\install.ps1
    .\scripts\install.ps1 -Uninstall -Purge
#>
[CmdletBinding()]
param(
    # Dossier d'installation.
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA "Programs\RustCast"),
    # N'installe pas les prérequis (Build Tools, WebView2, Rust).
    [switch]$SkipDeps,
    # Produit aussi les installateurs MSI et NSIS.
    [switch]$Bundle,
    # Ne crée pas de raccourci sur le bureau.
    [switch]$NoDesktopShortcut,
    # Ne crée aucun raccourci (menu Démarrer et bureau).
    [switch]$NoShortcuts,
    # Désinstalle RustCast.
    [switch]$Uninstall,
    # Avec -Uninstall : supprime aussi abonnements et téléchargements.
    [switch]$Purge,
    # Répond oui à toutes les questions.
    [switch]$Yes
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch { }

$AppId = "io.github.jeanjacquesserpoul.rustcast"
# Identifiant des premières versions, encore nettoyé par -Purge.
$LegacyAppId = "com.rustcast.app"
$UninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\RustCast"
$StartMenuLink = Join-Path ([Environment]::GetFolderPath("Programs")) "RustCast.lnk"
$DesktopLink = Join-Path ([Environment]::GetFolderPath("Desktop")) "RustCast.lnk"

# ───────────────────────────── Messages bilingues ─────────────────────────────

$IsFrench = (Get-UICulture).TwoLetterISOLanguageName -eq "fr"
function T([string]$fr, [string]$en) { if ($IsFrench) { $fr } else { $en } }
function Step([string]$msg) { Write-Host ""; Write-Host "==> " -ForegroundColor Magenta -NoNewline; Write-Host $msg -ForegroundColor White }
function Info([string]$msg) { Write-Host "    $msg" }
function Ok([string]$msg) { Write-Host "    " -NoNewline; Write-Host ([char]0x2713) -ForegroundColor Green -NoNewline; Write-Host " $msg" }
function Warn([string]$msg) { Write-Host "    ! $msg" -ForegroundColor Yellow }
function Fail([string]$msg) { Write-Host ""; Write-Host "x $msg" -ForegroundColor Red; exit 1 }

function Confirm-Action([string]$question) {
    if ($Yes) { return $true }
    $answer = Read-Host "    $question $(T '[O/n]' '[Y/n]')"
    return ($answer -eq "" -or $answer -match "^[oOyY]")
}

# Lance une commande externe et s'arrête si elle échoue.
function Invoke-Native([string]$exe, [string[]]$arguments) {
    & $exe @arguments
    if ($LASTEXITCODE -ne 0) { Fail "$(T 'Échec de la commande' 'Command failed') : $exe $($arguments -join ' ') (code $LASTEXITCODE)" }
}

function Update-SessionPath {
    $machine = [Environment]::GetEnvironmentVariable("Path", "Machine")
    $user = [Environment]::GetEnvironmentVariable("Path", "User")
    $cargo = Join-Path $env:USERPROFILE ".cargo\bin"
    $env:Path = "$cargo;$user;$machine"
}

function Test-Command([string]$name) { [bool](Get-Command $name -ErrorAction SilentlyContinue) }

function Stop-RustCast {
    Get-Process -Name "rustcast" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 300
}

# ───────────────────────────── Désinstallation ─────────────────────────────

if ($Uninstall) {
    Step (T "Désinstallation de RustCast" "Uninstalling RustCast")
    Stop-RustCast
    # Le script peut s'exécuter depuis le dossier qu'il supprime.
    Set-Location $env:TEMP
    foreach ($link in @($StartMenuLink, $DesktopLink)) {
        if (Test-Path $link) { Remove-Item $link -Force; Ok $link }
    }
    if (Test-Path $InstallDir) { Remove-Item $InstallDir -Recurse -Force; Ok $InstallDir }
    if (Test-Path $UninstallKey) { Remove-Item $UninstallKey -Recurse -Force; Ok (T "Entrée « Applications installées » supprimée" "Removed from Installed apps") }

    $dataDirs = foreach ($id in @($AppId, $LegacyAppId)) { Join-Path $env:APPDATA $id; Join-Path $env:LOCALAPPDATA $id }
    $dataDirs = @($dataDirs | Where-Object { Test-Path $_ })
    if ($Purge) {
        foreach ($dir in $dataDirs) { Remove-Item $dir -Recurse -Force; Ok "$(T 'Données supprimées' 'Data removed') : $dir" }
    } elseif ($dataDirs) {
        Info "$(T 'Vos abonnements et téléchargements sont conservés dans' 'Your subscriptions and downloads are kept in') $($dataDirs[0])"
        Info (T "(relancez avec -Purge pour les effacer)" "(run again with -Purge to delete them)")
    }
    Write-Host ""
    Write-Host (T "RustCast a été désinstallé." "RustCast has been uninstalled.") -ForegroundColor Green
    exit 0
}

$Root = Split-Path -Parent $PSScriptRoot
$Conf = Join-Path $Root "src-tauri\tauri.conf.json"
if (-not (Test-Path $Conf)) { Fail (T "Dossier du projet introuvable (lancez le script depuis le dépôt RustCast)." "Project folder not found (run the script from the RustCast repository).") }
$Version = (Get-Content $Conf -Raw | ConvertFrom-Json).version

Write-Host "RustCast " -ForegroundColor Magenta -NoNewline
Write-Host "$Version — $(T 'installation' 'installation')"
Info "$(T 'Sources' 'Sources') : $Root"
Info "$(T 'Destination' 'Destination') : $InstallDir"

if (-not [Environment]::Is64BitOperatingSystem) { Fail (T "Windows 64 bits est requis." "64-bit Windows is required.") }

# ───────────────────────────── Prérequis ─────────────────────────────

function Test-BuildTools {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) { return $false }
    $path = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    return [bool]$path
}

function Test-WebView2 {
    $guid = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
    foreach ($key in @(
            "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$guid",
            "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\$guid",
            "HKCU:\Software\Microsoft\EdgeUpdate\Clients\$guid")) {
        $pv = (Get-ItemProperty -Path $key -Name pv -ErrorAction SilentlyContinue).pv
        if ($pv -and $pv -ne "0.0.0.0") { return $true }
    }
    return $false
}

function Install-WithWinget([string]$id, [string[]]$extra = @()) {
    if (-not (Test-Command "winget")) { return $false }
    & winget install --id $id -e --silent --accept-source-agreements --accept-package-agreements @extra
    return ($LASTEXITCODE -eq 0)
}

function Install-Downloaded([string]$url, [string]$file, [string[]]$arguments) {
    $path = Join-Path $env:TEMP $file
    Info "$(T 'Téléchargement de' 'Downloading') $url"
    Invoke-WebRequest -Uri $url -OutFile $path -UseBasicParsing
    Invoke-Native $path $arguments
    Remove-Item $path -Force -ErrorAction SilentlyContinue
}

if ($SkipDeps) {
    Step (T "Prérequis ignorés (-SkipDeps)" "Skipping prerequisites (-SkipDeps)")
} else {
    Step (T "Prérequis" "Prerequisites")

    if (Test-BuildTools) {
        Ok "Visual Studio C++ Build Tools"
    } else {
        Warn (T "Les outils de compilation C++ de Visual Studio sont absents (environ 2 Go)." "Visual Studio C++ build tools are missing (about 2 GB).")
        if (-not (Confirm-Action (T "Les installer maintenant ?" "Install them now?"))) { Fail (T "Installation annulée." "Installation cancelled.") }
        $override = "--quiet --wait --norestart --nocache --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
        if (-not (Install-WithWinget "Microsoft.VisualStudio.2022.BuildTools" @("--override", $override))) {
            Install-Downloaded "https://aka.ms/vs/17/release/vs_BuildTools.exe" "vs_BuildTools.exe" ($override -split " ")
        }
        if (-not (Test-BuildTools)) { Fail (T "Les Build Tools ne sont pas détectés. Redémarrez Windows puis relancez le script." "Build Tools not detected. Restart Windows and run the script again.") }
        Ok "Visual Studio C++ Build Tools"
    }

    if (Test-WebView2) {
        Ok "Microsoft Edge WebView2"
    } else {
        Info (T "Installation de WebView2…" "Installing WebView2…")
        if (-not (Install-WithWinget "Microsoft.EdgeWebView2Runtime")) {
            Install-Downloaded "https://go.microsoft.com/fwlink/p/?LinkId=2124703" "MicrosoftEdgeWebview2Setup.exe" @("/silent", "/install")
        }
        Ok "Microsoft Edge WebView2"
    }
}

# ───────────────────────────── Chaîne Rust ─────────────────────────────

Step (T "Chaîne de compilation Rust" "Rust toolchain")
Update-SessionPath
if (-not (Test-Command "cargo")) {
    if ($SkipDeps) { Fail (T "Rust est introuvable (cargo). Installez-le depuis https://rustup.rs" "Rust not found (cargo). Install it from https://rustup.rs") }
    Info (T "Installation de Rust via rustup…" "Installing Rust with rustup…")
    Install-Downloaded "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe" "rustup-init.exe" @("-y", "--profile", "minimal", "--default-toolchain", "stable")
    Update-SessionPath
    if (-not (Test-Command "cargo")) { Fail (T "cargo reste introuvable après l'installation de Rust." "cargo still not found after installing Rust.") }
}
Ok (& cargo --version)

if (Test-Command "rustup") {
    Invoke-Native "rustup" @("target", "add", "wasm32-unknown-unknown")
    Ok "wasm32-unknown-unknown"
} else {
    Warn (T "rustup absent : vérifiez que la cible wasm32-unknown-unknown est installée." "rustup missing: make sure the wasm32-unknown-unknown target is installed.")
}

function Install-CargoTool([string]$bin, [string]$crate, [string]$version) {
    if (Test-Command $bin) {
        Ok "$crate ($((& $bin --version) | Select-Object -First 1))"
        return
    }
    Info "$(T 'Installation de' 'Installing') $crate…"
    if (Test-Command "cargo-binstall") {
        Invoke-Native "cargo" @("binstall", "-y", "--locked", "$crate@$version")
    } else {
        Invoke-Native "cargo" @("install", "--locked", $crate, "--version", $version)
    }
    Ok $crate
}
Install-CargoTool "trunk" "trunk" "^0.21"
Install-CargoTool "cargo-tauri" "tauri-cli" "^2"

# ───────────────────────────── Compilation ─────────────────────────────

Step (T "Compilation de RustCast (quelques minutes la première fois)" "Building RustCast (a few minutes the first time)")
# trunk n'accepte que true/false pour NO_COLOR, alors que la convention usuelle est NO_COLOR=1.
if ($env:NO_COLOR -and $env:NO_COLOR -notin @("true", "false")) { $env:NO_COLOR = "true" }
Push-Location $Root
try {
    if ($Bundle) {
        Invoke-Native "cargo" @("tauri", "build", "--bundles", "msi,nsis")
    } else {
        Invoke-Native "cargo" @("tauri", "build", "--no-bundle")
    }
} finally {
    Pop-Location
}
$Built = Join-Path $Root "target\release\rustcast.exe"
if (-not (Test-Path $Built)) { Fail "$(T 'Exécutable introuvable après compilation' 'Executable not found after build') : $Built" }
Ok $Built

# ───────────────────────────── Installation ─────────────────────────────

Step "$(T 'Installation dans' 'Installing into') $InstallDir"
Stop-RustCast
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
$Exe = Join-Path $InstallDir "RustCast.exe"
Copy-Item $Built $Exe -Force
Copy-Item (Join-Path $Root "src-tauri\icons\icon.ico") (Join-Path $InstallDir "RustCast.ico") -Force
# Copie du script pour permettre la désinstallation depuis « Applications installées ».
$InstalledScript = Join-Path $InstallDir "install.ps1"
if ($PSCommandPath -ne $InstalledScript) { Copy-Item $PSCommandPath $InstalledScript -Force }
Ok $Exe

function New-Shortcut([string]$path) {
    $shell = New-Object -ComObject WScript.Shell
    $link = $shell.CreateShortcut($path)
    $link.TargetPath = $Exe
    $link.WorkingDirectory = $InstallDir
    $link.IconLocation = "$Exe,0"
    $link.Description = T "Gestionnaire de podcasts" "Podcast manager"
    $link.Save()
    Ok $path
}
if (-not $NoShortcuts) {
    New-Shortcut $StartMenuLink
    if (-not $NoDesktopShortcut) { New-Shortcut $DesktopLink }
}

$sizeKb = [int]((Get-ChildItem $InstallDir -Recurse | Measure-Object Length -Sum).Sum / 1KB)
New-Item -Path $UninstallKey -Force | Out-Null
$props = @{
    DisplayName     = "RustCast"
    DisplayVersion  = $Version
    Publisher       = "RustCast"
    DisplayIcon     = "$Exe,0"
    InstallLocation = $InstallDir
    UninstallString = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$InstalledScript`" -Uninstall -InstallDir `"$InstallDir`""
}
foreach ($name in $props.Keys) { Set-ItemProperty -Path $UninstallKey -Name $name -Value $props[$name] }
foreach ($name in @("NoModify", "NoRepair")) { Set-ItemProperty -Path $UninstallKey -Name $name -Value 1 -Type DWord }
Set-ItemProperty -Path $UninstallKey -Name "EstimatedSize" -Value $sizeKb -Type DWord
Ok (T "Ajouté à « Applications installées »" "Added to Installed apps")

if ($Bundle) { Info "$(T 'Installateurs générés dans' 'Installers written to') $(Join-Path $Root 'target\release\bundle')" }

Write-Host ""
Write-Host (T "RustCast est installé !" "RustCast is installed!") -ForegroundColor Green
Info (T "Lancez-le depuis le menu Démarrer." "Launch it from the Start menu.")
Info "$(T 'Désinstaller : Paramètres > Applications, ou' 'Uninstall: Settings > Apps, or') .\scripts\install.ps1 -Uninstall"
