#!/usr/bin/env bash
set -euo pipefail
source_dir="$(cd -- "$(dirname -- "$0")" && pwd)"
prefix="$HOME/.local/opt/nyx-online-accounts-preview"
install -d "$prefix/bin" "$prefix/share/dbus-1/services" "$prefix/share/doc/nyx-accounts"
cargo build --release --locked --manifest-path "$source_dir/Cargo.toml"
install -m755 "$source_dir/target/release/nyx-accountsd" "$source_dir/target/release/nyx-accountsctl" "$prefix/bin/"
install -m755 "$source_dir/packaging/nyx-online-accounts-preview" "$source_dir/packaging/nyx-local-copy" "$source_dir/packaging/nyx-preview-session" "$prefix/bin/"
install -m644 "$source_dir/docs/"*.md "$prefix/share/doc/nyx-accounts/"
cat > "$prefix/share/dbus-1/services/io.github.design_nexus.nyx.Accounts.Preview.service" <<SERVICE
[D-BUS Service]
Name=io.github.design_nexus.nyx.Accounts.Preview
Exec=$prefix/bin/nyx-accountsd
SERVICE
install -d "$HOME/.local/share/applications"
cat > "$HOME/.local/share/applications/nyx-online-accounts-preview.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Online Accounts Preview
Comment=Isolated Nyx native accounts development preview
Exec=/usr/bin/systemd-run --user --collect --quiet --property=Type=exec $prefix/bin/nyx-online-accounts-preview nyx-settings --section online-accounts
Icon=preferences-system
Categories=Settings;
StartupWMClass=io.github.design_nexus.nyx.Settings
Terminal=false
DESKTOP

# Install only into the isolated preview; never enable user units or launch apps.
projects="$(dirname "$source_dir")"
for app in settings mail calendar contacts reminders; do
    manifest="$projects/nyx-$app/Cargo.toml"
    packages=(-p "nyx-$app")
    case "$app" in mail) packages+=(-p nyx-maild);; calendar) packages+=(-p nyx-calendard);; reminders) packages+=(-p nyx-remindersd);; esac
    cargo build --release --locked --manifest-path "$manifest" "${packages[@]}"
    for binary in "nyx-$app"; do install -m755 "$projects/nyx-$app/target/release/$binary" "$prefix/bin/"; done
    if [[ "$app" == settings && -f "$projects/nyx-settings/target/release/nyx-settings-helper" ]]; then install -m755 "$projects/nyx-settings/target/release/nyx-settings-helper" "$prefix/bin/"; fi
    case "$app" in mail|calendar|reminders) install -m755 "$projects/nyx-$app/target/release/nyx-${app}d" "$prefix/bin/";; esac
done
cargo build --release --locked --manifest-path "$projects/nyx-cloud/Cargo.toml"
install -m755 "$projects/nyx-cloud/target/release/nyx-cloudd" "$prefix/bin/"
cargo build --release --locked --manifest-path "$projects/nyx-core/Cargo.toml" -p nyx-pim-migrate
install -m755 "$projects/nyx-core/target/release/nyx-pim-migrate" "$prefix/bin/"
cat > "$prefix/share/dbus-1/services/io.github.design_nexus.nyx.Cloud1.service" <<SERVICE
[D-BUS Service]
Name=io.github.design_nexus.nyx.Cloud1
Exec=$prefix/bin/nyx-cloudd
SERVICE
install -d "$prefix/share/icons/hicolor/scalable/apps"
install -m644 "$projects/nyx-design/crates/nyx-icons/apps/io.github.design_nexus.nyx.Settings.svg" "$prefix/share/icons/hicolor/scalable/apps/nyx-online-accounts-preview.svg"
# Absolute icon path keeps the private prefix discoverable by the desktop launcher.
sed -i "s|Icon=preferences-system|Icon=$prefix/share/icons/hicolor/scalable/apps/nyx-online-accounts-preview.svg|" "$HOME/.local/share/applications/nyx-online-accounts-preview.desktop"

# Files' existing Cloud1 client is protocol compatible; keep its own asset/runtime path.
files_binary="$(command -v nyx-files || true)"
if [[ -n "$files_binary" && "$files_binary" != "$prefix/bin/nyx-files" ]]; then
    { printf '#!/usr/bin/env bash\n'; printf 'exec %q "$@"\n' "$files_binary"; } > "$prefix/bin/nyx-files"
    chmod 755 "$prefix/bin/nyx-files"
fi
for app in Settings Mail Calendar Contacts Reminders Files; do
    icon="$projects/nyx-design/crates/nyx-icons/apps/io.github.design_nexus.nyx.$app.svg"
    if [[ -f "$icon" ]]; then install -m644 "$icon" "$prefix/share/icons/hicolor/scalable/apps/"; fi
done

install -d "$prefix/share/doc/nyx-accounts/licenses"
install -m644 "$projects/nyx-core/crates/nyx-pim/data/ICAL-APACHE-2.0.txt" "$projects/nyx-core/crates/nyx-pim/data/UNICODE-LICENSE.txt" "$prefix/share/doc/nyx-accounts/licenses/"

if [[ -f "$projects/nyx/docs/online-accounts-preview.toml" ]]; then
    install -m644 "$projects/nyx/docs/online-accounts-preview.toml" "$prefix/share/doc/nyx-accounts/"
fi
