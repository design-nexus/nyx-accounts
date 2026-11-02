#!/usr/bin/env bash
# Build a normal-session installation, or stage it for a coordinated rollout.
set -euo pipefail
source_dir="$(cd -- "$(dirname -- "$0")" && pwd)"
projects="$(dirname "$source_dir")"
stage="${NYX_ACCOUNTS_STAGE:-$HOME/.local/opt/nyx-online-accounts-release}"
mode="${1:---stage}"
case "$mode" in --stage|--activate|accounts) ;; *) echo 'Usage: install.sh --stage|--activate|accounts' >&2; exit 2;; esac
if [[ "$mode" == accounts ]]; then
    # Standalone/suite service installation has no sibling-repository dependency.
    data="${XDG_DATA_HOME:-$HOME/.local/share}"
    if [[ -f "$HOME/.local/share/nyx-online-accounts-preview/data/nyx-accounts-preview/accounts.sqlite3" && ! -f "$data/nyx-accounts-preview/promotion.json" ]]; then
        echo 'Existing preview accounts require explicit promotion before normal service installation.' >&2
        exit 1
    fi
    cargo build --release --locked --manifest-path "$source_dir/Cargo.toml"
    install -d "$HOME/.local/bin" "$data/dbus-1/services"
    for binary in nyx-accountsd nyx-accountsctl; do
        install -m755 "$source_dir/target/release/$binary" "$HOME/.local/bin/$binary.new"
        mv -f "$HOME/.local/bin/$binary.new" "$HOME/.local/bin/$binary"
    done
    cat > "$data/dbus-1/services/io.github.design_nexus.nyx.Accounts.Preview.service" <<SERVICE
[D-BUS Service]
Name=io.github.design_nexus.nyx.Accounts.Preview
Exec=$HOME/.local/bin/nyx-accountsd
SERVICE
    echo 'Installed Accounts service; activation occurs when an application requests it.'
    exit
fi
if [[ "$mode" == --stage ]]; then
    install -d "$stage/bin" "$stage/share/doc/nyx-accounts"
    cargo build --release --locked --manifest-path "$source_dir/Cargo.toml"
    install -m755 "$source_dir/target/release/nyx-accountsd" "$source_dir/target/release/nyx-accountsctl" "$stage/bin/"
    for app in settings mail calendar contacts reminders; do
        packages=(-p "nyx-$app")
        case "$app" in mail|calendar|reminders) packages+=(-p "nyx-${app}d");; esac
        cargo build --release --locked --manifest-path "$projects/nyx-$app/Cargo.toml" "${packages[@]}"
        install -m755 "$projects/nyx-$app/target/release/nyx-$app" "$stage/bin/"
        case "$app" in mail|calendar|reminders) install -m755 "$projects/nyx-$app/target/release/nyx-${app}d" "$stage/bin/";; esac
    done
    cargo build --release --locked --manifest-path "$projects/nyx-cloud/Cargo.toml"
    install -m755 "$projects/nyx-cloud/target/release/nyx-cloudd" "$stage/bin/"
    cargo build --release --locked --manifest-path "$projects/nyx-core/Cargo.toml" -p nyx-pim-migrate
    install -m755 "$projects/nyx-core/target/release/nyx-pim-migrate" "$stage/bin/"
    install -m755 "$source_dir/packaging/nyx-accounts-promote" "$stage/bin/"
    install -m644 "$source_dir/docs/"*.md "$stage/share/doc/nyx-accounts/"
    echo "Staged normal-session binaries: $stage"
    exit
fi
# Activation is deliberately separate from builds and migration. Never start a GUI.
data="${XDG_DATA_HOME:-$HOME/.local/share}"
preview="$HOME/.local/share/nyx-online-accounts-preview"
if [[ -f "$preview/data/nyx-accounts-preview/accounts.sqlite3" && ! -f "$data/nyx-accounts-preview/promotion.json" ]]; then
    echo 'Promote the existing accounts first; activation stopped without changing the installation.' >&2
    exit 1
fi
install -d "$HOME/.local/bin" "$data/dbus-1/services" "$data/applications" "$data/icons/hicolor/scalable/apps"
for binary in nyx-accountsd nyx-accountsctl nyx-pim-migrate nyx-accounts-promote nyx-settings nyx-mail nyx-maild nyx-calendar nyx-calendard nyx-contacts nyx-reminders nyx-remindersd nyx-cloudd; do
    [[ -x "$stage/bin/$binary" ]] || { echo "Missing staged binary: $binary" >&2; exit 1; }
done
for binary in "$stage/bin/"*; do
    name="$(basename "$binary")"
    install -m755 "$binary" "$HOME/.local/bin/$name.new"
    mv -f "$HOME/.local/bin/$name.new" "$HOME/.local/bin/$name"
done
cat > "$data/dbus-1/services/io.github.design_nexus.nyx.Accounts.Preview.service" <<SERVICE
[D-BUS Service]
Name=io.github.design_nexus.nyx.Accounts.Preview
Exec=$HOME/.local/bin/nyx-accountsd
SERVICE
for app in settings mail calendar contacts reminders; do
    desktop="$projects/nyx-$app/apps/$app/data"
    case "$app" in settings) display=Settings;; mail) display=Mail;; calendar) display=Calendar;; contacts) display=Contacts;; reminders) display=Reminders;; esac
    install -m644 "$desktop/io.github.design_nexus.nyx.$display.desktop" "$data/applications/"
    install -m644 "$projects/nyx-design/crates/nyx-icons/apps/io.github.design_nexus.nyx.$display.svg" "$data/icons/hicolor/scalable/apps/"
done
install -d "$data/doc/nyx-accounts"
cp -R "$stage/share/doc/nyx-accounts/." "$data/doc/nyx-accounts/"
command -v update-desktop-database >/dev/null && update-desktop-database "$data/applications" || true
echo 'Installed normal Nyx applications. No graphical applications were opened or restarted.'
