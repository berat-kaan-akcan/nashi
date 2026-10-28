#!/usr/bin/env bash
# Nashi for Linux — from-source installer.
#
#   packaging/install.sh [--prefix DIR | --user] [--no-build] [--dry-run] [--yes]
#   packaging/install.sh --uninstall [--prefix DIR | --user] [--dry-run] [--yes]
#
# Run as your normal user. Privileged steps (system-wide files, udev rule,
# adding you to the `input` group) use sudo and are confirmed first unless
# --yes is given.
set -euo pipefail

readonly APP_ID=io.github.beratkaanakcan.Nashi
readonly UNIT=nashi-inputd.service
readonly UDEV_RULE=70-nashi-inputd.rules

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)
readonly script_dir repo_root

prefix=/usr/local
prefix_given=0
user_mode=0
uninstall=0
do_build=1
dry_run=0
assume_yes=0

usage() {
    cat <<EOF
Usage: ${0##*/} [options]

Build Nashi from this checkout and install it.

Options:
  --prefix DIR   Install prefix for a system-wide install (default: /usr/local)
  --user         Install into ~/.local for the current user (sudo is only used
                 for the udev rule and the input group)
  --uninstall    Remove a previous installation (same --prefix/--user as install)
  --no-build     Do not build; use existing target/release and build/ui-release
  --dry-run      Print what would be done, change nothing
  -y, --yes      Do not ask before sudo steps
  -h, --help     Show this help
EOF
}

die() {
    printf 'install.sh: error: %s\n' "$*" >&2
    exit 1
}
info() { printf '==> %s\n' "$*"; }
note() { printf '    %s\n' "$*"; }
warn() { printf 'install.sh: warning: %s\n' "$*" >&2; }

while (($# > 0)); do
    case "$1" in
        --prefix)
            (($# >= 2)) || die "--prefix needs a directory"
            prefix=$2
            prefix_given=1
            shift 2
            ;;
        --prefix=*)
            prefix=${1#--prefix=}
            prefix_given=1
            shift
            ;;
        --user) user_mode=1; shift ;;
        --uninstall) uninstall=1; shift ;;
        --no-build) do_build=0; shift ;;
        --dry-run) dry_run=1; shift ;;
        -y | --yes) assume_yes=1; shift ;;
        -h | --help) usage; exit 0 ;;
        *) usage >&2; die "unknown option: $1" ;;
    esac
done

if ((EUID == 0)); then
    die "run this script as your normal user, not root; it calls sudo where needed"
fi
if ((user_mode && prefix_given)); then
    die "--user and --prefix cannot be combined"
fi

user_name=${USER:-$(id -un)}
config_home=${XDG_CONFIG_HOME:-$HOME/.config}
data_home=${XDG_DATA_HOME:-$HOME/.local/share}

# ---------------------------------------------------------------- layout ----

if ((user_mode)); then
    prefix=$HOME/.local
    datadir=$data_home
    autostartdir=$config_home/autostart
    unitdir=$config_home/systemd/user
    file_priv=user
    servicemenu_mode=755 # KIO only runs user service menus that are executable
else
    [[ "$prefix" == /* ]] || die "--prefix must be an absolute path"
    prefix=${prefix%/}
    [[ -n "$prefix" ]] || prefix=/
    datadir=$prefix/share
    autostartdir=/etc/xdg/autostart
    case "$prefix" in
        /usr | /usr/local) unitdir=$prefix/lib/systemd/user ;;
        *) unitdir=/etc/systemd/user ;; # systemd does not search other prefixes
    esac
    file_priv=root
    servicemenu_mode=644
fi
case "$prefix" in
    /usr) udevdir=/usr/lib/udev/rules.d ;;
    *) udevdir=/etc/udev/rules.d ;; # udev does not search /usr/local
esac
bindir=$prefix/bin
nautilus_dir=$data_home/nautilus/scripts
manifest=$datadir/nashi/install-manifest.txt

case "$bindir" in
    *[[:space:]]*) die "the install prefix must not contain whitespace: $prefix" ;;
esac

cargo_target=${CARGO_TARGET_DIR:-$repo_root/target}
ui_build=$repo_root/build/ui-release

# --------------------------------------------------------------- helpers ----

run() {
    if ((dry_run)); then
        printf '    [dry-run]'
        printf ' %q' "$@"
        printf '\n'
    else
        "$@"
    fi
}

# Runs a command as root via sudo.
as_root() { run sudo "$@"; }

# Runs a command with the privilege needed for installed files.
as_owner() {
    if [[ "$1" == root ]]; then
        shift
        as_root "$@"
    else
        shift
        run "$@"
    fi
}

confirm() {
    if ((assume_yes || dry_run)); then
        return 0
    fi
    if [[ ! -r /dev/tty ]]; then
        warn "no terminal to ask: '$1' skipped (use --yes)"
        return 1
    fi
    local reply
    read -r -p "$1 [y/N] " reply </dev/tty || return 1
    [[ "$reply" == [yY] || "$reply" == [yY][eE][sS] || "$reply" == [eE] ]]
}

have() { command -v "$1" >/dev/null 2>&1; }

sed_escape() { printf '%s' "$1" | sed -e 's/[\\|&]/\\&/g'; }

stage=""
cleanup() {
    if [[ -n "$stage" && -d "$stage" ]]; then
        rm -rf -- "$stage"
    fi
}
trap cleanup EXIT

# Planned files: parallel arrays (source, destination, mode, privilege).
plan_src=()
plan_dst=()
plan_mode=()
plan_priv=()

plan() {
    local i
    for i in "${!plan_dst[@]}"; do
        [[ "${plan_dst[$i]}" == "$2" ]] && return 0
    done
    plan_src+=("$1")
    plan_dst+=("$2")
    plan_mode+=("$3")
    plan_priv+=("$4")
}

# Copies $1 to the staging directory as $2, rewriting /usr/bin/nashi* and the
# bare `nashi` command to the real bindir when it is not /usr/bin.
stage_rewritten() {
    local src=$1 out=$stage/$2
    if [[ ! -f "$src" ]]; then
        printf '%s\n' "$src" # dry run before the file exists
        return 0
    fi
    mkdir -p -- "$(dirname -- "$out")"
    if [[ "$bindir" == /usr/bin ]]; then
        cp -- "$src" "$out"
    else
        local b
        b=$(sed_escape "$bindir")
        sed -e "s|/usr/bin/nashi|$b/nashi|g" \
            -e "s|^Exec=nashi |Exec=$b/nashi |" \
            -e "s|^TryExec=nashi\$|TryExec=$b/nashi|" \
            -e "s|^nashi_bin=\"\${NASHI_BIN:-nashi}\"|nashi_bin=\"\${NASHI_BIN:-$b/nashi}\"|" \
            -- "$src" >"$out"
    fi
    printf '%s\n' "$out"
}

# Static part of the layout (everything except the UI's own install tree).
plan_static() {
    local placeholder=${1:-0} s
    local inputd_dist=$repo_root/crates/nashi-inputd/dist

    plan "$cargo_target/release/nashi-cli" "$bindir/nashi-cli" 755 "$file_priv"
    plan "$cargo_target/release/nashi-inputd" "$bindir/nashi-inputd" 755 "$file_priv"

    if ((placeholder)); then
        s=/dev/null
    fi
    plan "${s:-$(stage_rewritten "$repo_root/data/$APP_ID.desktop" app.desktop)}" \
        "$datadir/applications/$APP_ID.desktop" 644 "$file_priv"
    plan "${s:-$(stage_rewritten "$repo_root/data/$APP_ID-autostart.desktop" autostart.desktop)}" \
        "$autostartdir/$APP_ID-autostart.desktop" 644 "$file_priv"
    plan "${s:-$(stage_rewritten "$repo_root/data/$APP_ID.service" dbus.service)}" \
        "$datadir/dbus-1/services/$APP_ID.service" 644 "$file_priv"
    plan "${s:-$(stage_rewritten "$repo_root/packaging/dolphin/nashi.desktop" servicemenu.desktop)}" \
        "$datadir/kio/servicemenus/nashi.desktop" "$servicemenu_mode" "$file_priv"
    plan "$repo_root/data/$APP_ID.metainfo.xml" "$datadir/metainfo/$APP_ID.metainfo.xml" 644 "$file_priv"
    plan "$repo_root/data/icons/hicolor/scalable/apps/$APP_ID.svg" \
        "$datadir/icons/hicolor/scalable/apps/$APP_ID.svg" 644 "$file_priv"
    plan "$repo_root/LICENSE" "$datadir/licenses/nashi/LICENSE" 644 "$file_priv"
    plan "$repo_root/NOTICE" "$datadir/licenses/nashi/NOTICE" 644 "$file_priv"

    plan "${s:-$(stage_rewritten "$inputd_dist/$UNIT" "$UNIT")}" "$unitdir/$UNIT" 644 "$file_priv"
    plan "$inputd_dist/$UDEV_RULE" "$udevdir/$UDEV_RULE" 644 root

    if have nautilus || [[ -d "$nautilus_dir" ]]; then
        local ns=${s:-$(stage_rewritten "$repo_root/packaging/nautilus/nashi-nautilus-script" nautilus-script)}
        plan "$ns" "$nautilus_dir/Nashi – Convert" 755 user
        plan "$ns" "$nautilus_dir/Nashi – Tools" 755 user
    fi
}

# ----------------------------------------------------------------- build ----

build() {
    have cargo || die "cargo not found (install rust: sudo pacman -S rust)"
    have cmake || die "cmake not found (sudo pacman -S cmake)"
    local -a gen=()
    if have ninja; then
        gen=(-G Ninja)
    fi

    info "Building Rust workspace (release)"
    (cd -- "$repo_root" && run cargo build --release --workspace)

    info "Building Qt UI"
    run cmake -S "$repo_root/ui" -B "$ui_build" "${gen[@]}" \
        -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$prefix"
    run cmake --build "$ui_build"
}

# Stages the UI's own install tree and plans every file in it.
plan_ui() {
    local ui_stage=$stage/ui
    if [[ ! -d "$ui_build" ]]; then
        if ((dry_run)); then
            warn "UI not built yet ($ui_build); showing the binary only"
            plan "$ui_build/nashi" "$bindir/nashi" 755 "$file_priv"
            return 0
        fi
        die "UI build directory not found: $ui_build (run without --no-build)"
    fi

    cmake --install "$ui_build" --prefix "$ui_stage" >/dev/null
    local f rel
    if [[ -d "$ui_stage" ]]; then
        while IFS= read -r -d '' f; do
            rel=${f#"$ui_stage"/}
            if [[ "$rel" == bin/* ]]; then
                plan "$f" "$bindir/${rel#bin/}" 755 "$file_priv"
            elif [[ -x "$f" ]]; then
                plan "$f" "$prefix/$rel" 755 "$file_priv"
            else
                plan "$f" "$prefix/$rel" 644 "$file_priv"
            fi
        done < <(find "$ui_stage" -type f -print0)
    fi

    if [[ ! -f "$ui_stage/bin/nashi" ]]; then
        # No install() rule for the binary: take it from the build tree.
        f=$(find "$ui_build" -type f -name nashi -perm -u+x -print -quit)
        [[ -n "$f" ]] || die "UI binary 'nashi' not found under $ui_build"
        plan "$f" "$bindir/nashi" 755 "$file_priv"
    fi
}

# --------------------------------------------------------------- actions ----

refresh_caches() {
    info "Refreshing desktop caches"
    local p=run
    [[ "$file_priv" == root ]] && p=as_root
    if have update-desktop-database && [[ -d "$datadir/applications" ]]; then
        $p update-desktop-database -q "$datadir/applications" || true
    fi
    if have gtk-update-icon-cache && [[ -f "$datadir/icons/hicolor/index.theme" ]]; then
        $p gtk-update-icon-cache -q -t -f "$datadir/icons/hicolor" || true
    fi
    if have kbuildsycoca6; then
        run kbuildsycoca6 >/dev/null 2>&1 || true
    fi
}

reload_udev() {
    have udevadm || return 0
    as_root udevadm control --reload || true
    as_root udevadm trigger --subsystem-match=input || true
}

ensure_input_group() {
    if ! getent group input >/dev/null; then
        warn "group 'input' does not exist on this system; nashi-inputd may not be able to read /dev/input"
        return 0
    fi
    if id -nG "$user_name" | tr ' ' '\n' | grep -qx input; then
        info "User '$user_name' is already in the 'input' group"
        return 0
    fi
    info "nashi-inputd reads /dev/input/event* to detect drag + modifier chords."
    note "It only reports chord state (never key codes), but it needs the 'input' group."
    if confirm "Add '$user_name' to the 'input' group with sudo?"; then
        as_root usermod -aG input "$user_name"
        warn "log out and back in for the new group to take effect"
    else
        warn "skipped; run later: sudo usermod -aG input $user_name"
    fi
}

enable_service() {
    if ! have systemctl; then
        warn "systemctl not found; start nashi-inputd yourself"
        return 0
    fi
    info "Enabling the nashi-inputd user service"
    run systemctl --user daemon-reload || warn "systemctl --user daemon-reload failed"
    run systemctl --user enable --now "$UNIT" ||
        warn "could not start $UNIT now; after re-login run: systemctl --user enable --now $UNIT"
}

do_install() {
    stage=$(mktemp -d "${TMPDIR:-/tmp}/nashi-install.XXXXXX")

    if ((do_build)); then
        build
    fi

    local missing=0 f
    for f in "$cargo_target/release/nashi-cli" "$cargo_target/release/nashi-inputd" \
        "$repo_root/crates/nashi-inputd/dist/$UNIT" "$repo_root/crates/nashi-inputd/dist/$UDEV_RULE"; do
        if [[ ! -f "$f" ]]; then
            if ((dry_run)); then
                warn "missing (not built yet?): $f"
            else
                printf 'install.sh: missing: %s\n' "$f" >&2
                missing=1
            fi
        fi
    done
    ((missing == 0)) || die "required files are missing; build first (drop --no-build)"

    plan_static 0
    plan_ui

    local needs_root=0 i
    for i in "${!plan_dst[@]}"; do
        [[ "${plan_priv[$i]}" == root ]] && needs_root=1
    done

    info "Installing into $prefix"
    if ((needs_root)) && [[ "$file_priv" == root ]]; then
        confirm "Install system-wide files into $prefix and /etc with sudo?" ||
            die "aborted"
    fi

    local -a installed=()
    for i in "${!plan_dst[@]}"; do
        if [[ "${plan_priv[$i]}" == root && "$file_priv" != root ]]; then
            continue # udev rule in --user mode: handled below
        fi
        note "${plan_dst[$i]}"
        as_owner "${plan_priv[$i]}" install -Dm"${plan_mode[$i]}" -- "${plan_src[$i]}" "${plan_dst[$i]}"
        installed+=("${plan_dst[$i]}")
    done

    local udev_dst=$udevdir/$UDEV_RULE udev_src=""
    for i in "${!plan_dst[@]}"; do
        [[ "${plan_dst[$i]}" == "$udev_dst" ]] && udev_src=${plan_src[$i]}
    done
    if [[ "$file_priv" != root ]]; then
        info "The udev rule must be installed system-wide: $udev_dst"
        if confirm "Install the udev rule with sudo?"; then
            as_root install -Dm644 -- "$udev_src" "$udev_dst"
            installed+=("$udev_dst")
        else
            warn "skipped udev rule; nashi-inputd may not see input devices"
        fi
    fi
    if [[ " ${installed[*]} " == *" $udev_dst "* ]]; then
        reload_udev
    fi

    # Manifest for --uninstall (includes the UI's own files).
    local mf=$stage/install-manifest.txt
    printf '%s\n' "${installed[@]}" "$manifest" >"$mf"
    as_owner "$file_priv" install -Dm644 -- "$mf" "$manifest"

    ensure_input_group
    enable_service
    refresh_caches

    info "Done."
    if ((user_mode)); then
        case ":$PATH:" in
            *":$bindir:"*) ;;
            *) note "Note: $bindir is not in PATH; the desktop files use absolute paths." ;;
        esac
    fi
    note "If you were just added to the 'input' group, log out and back in."
    note "Check the daemon with: systemctl --user status $UNIT"
}

do_uninstall() {
    if have systemctl; then
        info "Stopping the nashi-inputd user service"
        run systemctl --user disable --now "$UNIT" 2>/dev/null || true
    fi

    local -a files=()
    if [[ -f "$manifest" ]]; then
        mapfile -t files <"$manifest"
    else
        warn "no install manifest at $manifest; removing the known file list"
        plan_static 1
        files=("${plan_dst[@]}" "$bindir/nashi")
    fi

    local -a root_files=() user_files=() f
    for f in "${files[@]}"; do
        [[ -n "$f" ]] || continue
        [[ -e "$f" || -L "$f" ]] || continue
        if [[ "$file_priv" == root || "$f" == "$udevdir/"* ]] && [[ "$f" != "$nautilus_dir/"* ]]; then
            root_files+=("$f")
        else
            user_files+=("$f")
        fi
    done

    if ((${#files[@]} > 0)) && ((${#root_files[@]} + ${#user_files[@]} == 0)); then
        info "Nothing to remove"
    fi
    for f in "${user_files[@]}"; do
        note "remove $f"
        run rm -f -- "$f"
    done
    if ((${#root_files[@]} > 0)); then
        printf '    remove %s\n' "${root_files[@]}"
        if confirm "Remove these files with sudo?"; then
            as_root rm -f -- "${root_files[@]}"
        else
            warn "skipped removing system files"
        fi
    fi

    local d p=run
    [[ "$file_priv" == root ]] && p=as_root
    for d in "$datadir/nashi" "$datadir/licenses/nashi"; do
        if [[ -d "$d" ]]; then
            $p rmdir --ignore-fail-on-non-empty -- "$d" || true
        fi
    done

    for f in "${root_files[@]}"; do
        if [[ "$f" == "$udevdir/"* ]]; then
            reload_udev
            break
        fi
    done
    if have systemctl; then
        run systemctl --user daemon-reload || true
    fi
    refresh_caches

    info "Nashi removed."
    note "Your settings in ~/.config/nashi were kept."
    note "You were not removed from the 'input' group; to do so: sudo gpasswd -d $user_name input"
}

if ((dry_run)); then
    info "Dry run: nothing will be changed"
fi
if ((uninstall)); then
    do_uninstall
else
    do_install
fi
