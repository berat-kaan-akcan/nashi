#pragma once

#include <QString>
#include <QStringList>
#include <QVariantList>

// Command line of the `nashi` UI (docs/ARCHITECTURE.md, "UI command line").
struct CommandLine
{
    enum Action {
        Start,        // `nashi`: start or raise Home
        Background,   // `nashi --background`: tray only
        Wheel,        // `nashi --wheel [--mode m] FILE…`
        WheelClipboard, // `nashi --wheel [--mode m]` without files: use the clipboard
        OpenFiles,    // `nashi FILE…`: Home with these files queued
        ShowHelp,
        ShowVersion,
        Error,
    };

    Action action = Start;
    QString mode;        // convert | tools | actions (only for Wheel*)
    QStringList files;   // absolute paths
    QString error;       // for Error
    QString helpText;
};

// How a second instance hands its request to the running one over D-Bus.
struct ForwardCall
{
    QString interface;
    QString method;
    QVariantList args;
    bool valid() const { return !method.isEmpty(); }
};

namespace CliArgs {

inline const QString kService = QStringLiteral("io.github.beratkaanakcan.Nashi");
inline const QString kPath = QStringLiteral("/io/github/beratkaanakcan/Nashi");
inline const QString kInterface = QStringLiteral("io.github.beratkaanakcan.Nashi1");
inline const QString kFdoAppInterface = QStringLiteral("org.freedesktop.Application");

// `args` includes argv[0]. Relative paths resolve against `cwd`;
// `file://` URLs are accepted too.
CommandLine parse(const QStringList &args, const QString &cwd);
ForwardCall forwardCallFor(const CommandLine &cmd);
bool isValidMode(const QString &mode);

} // namespace CliArgs
