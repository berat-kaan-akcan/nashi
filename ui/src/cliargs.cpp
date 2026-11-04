#include "cliargs.h"
#include "clipboardimport.h"

#include <QCommandLineParser>
#include <QCoreApplication>
#include <QDir>
#include <QUrl>
#include <QVariantMap>

namespace CliArgs {

bool isValidMode(const QString &mode)
{
    return mode == QLatin1String("convert") || mode == QLatin1String("tools") || mode == QLatin1String("actions");
}

CommandLine parse(const QStringList &args, const QString &cwd)
{
    CommandLine cmd;
    QCommandLineParser parser;
    parser.setApplicationDescription(QCoreApplication::translate("CliArgs", "Drag, hold a modifier, pick a slice: convert files."));
    const QCommandLineOption helpOpt(QStringList{QStringLiteral("h"), QStringLiteral("help")},
                                     QCoreApplication::translate("CliArgs", "Show this help."));
    const QCommandLineOption versionOpt(QStringLiteral("version"), QCoreApplication::translate("CliArgs", "Show the version."));
    const QCommandLineOption backgroundOpt(QStringLiteral("background"),
                                           QCoreApplication::translate("CliArgs", "Start in the tray without opening a window."));
    const QCommandLineOption wheelOpt(QStringLiteral("wheel"),
                                      QCoreApplication::translate("CliArgs", "Show the wheel for FILEs (or the clipboard)."));
    const QCommandLineOption modeOpt(QStringLiteral("mode"),
                                     QCoreApplication::translate("CliArgs", "Wheel mode: convert, tools or actions."),
                                     QStringLiteral("mode"), QStringLiteral("convert"));
    const QCommandLineOption downloadOpt(QStringLiteral("download"),
                                         QCoreApplication::translate("CliArgs", "Open the download dialog for an http(s) URL."),
                                         QStringLiteral("url"));
    const QCommandLineOption downloadClipboardOpt(
        QStringLiteral("download-clipboard"),
        QCoreApplication::translate("CliArgs", "Open the download dialog with the copied link."));
    const QCommandLineOption shelfOpt(
        QStringLiteral("shelf"),
        QCoreApplication::translate("CliArgs", "Toggle the Float shelf, or add FILEs to it."));
    parser.addOption(helpOpt);
    parser.addOption(versionOpt);
    parser.addOption(backgroundOpt);
    parser.addOption(wheelOpt);
    parser.addOption(modeOpt);
    parser.addOption(downloadOpt);
    parser.addOption(downloadClipboardOpt);
    parser.addOption(shelfOpt);
    parser.addPositionalArgument(QStringLiteral("files"), QCoreApplication::translate("CliArgs", "Files to open."),
                                 QStringLiteral("[FILE…]"));
    cmd.helpText = parser.helpText();

    if (!parser.parse(args)) {
        cmd.action = CommandLine::Error;
        cmd.error = parser.errorText();
        return cmd;
    }
    if (parser.isSet(helpOpt)) {
        cmd.action = CommandLine::ShowHelp;
        return cmd;
    }
    if (parser.isSet(versionOpt)) {
        cmd.action = CommandLine::ShowVersion;
        return cmd;
    }

    const QDir base(cwd);
    for (const QString &a : parser.positionalArguments()) {
        if (a.isEmpty())
            continue;
        QString path = a;
        if (a.startsWith(QLatin1String("file:"))) {
            const QUrl url(a);
            if (!url.isLocalFile())
                continue;
            path = url.toLocalFile();
        }
        cmd.files.append(QDir::cleanPath(base.absoluteFilePath(path)));
    }

    if (parser.isSet(modeOpt) && !parser.isSet(wheelOpt)) {
        cmd.action = CommandLine::Error;
        cmd.error = QCoreApplication::translate("CliArgs", "--mode requires --wheel");
        return cmd;
    }

    const int exclusiveCount = int(parser.isSet(backgroundOpt)) + int(parser.isSet(wheelOpt))
                               + int(parser.isSet(downloadOpt)) + int(parser.isSet(downloadClipboardOpt))
                               + int(parser.isSet(shelfOpt));
    if (exclusiveCount > 1) {
        cmd.action = CommandLine::Error;
        cmd.error = QCoreApplication::translate(
            "CliArgs", "--background, --wheel, --download, --download-clipboard and --shelf cannot be combined");
        return cmd;
    }

    if (parser.isSet(downloadOpt)) {
        if (!cmd.files.isEmpty()) {
            cmd.action = CommandLine::Error;
            cmd.error = QCoreApplication::translate("CliArgs", "--download takes only a URL, not files");
            return cmd;
        }
        const QString url = parser.value(downloadOpt).trimmed();
        if (!ClipboardImport::isSingleWebLink(url)) {
            cmd.action = CommandLine::Error;
            cmd.error = QCoreApplication::translate("CliArgs", "--download requires an http:// or https:// URL");
            return cmd;
        }
        cmd.url = url;
        cmd.action = CommandLine::Download;
        return cmd;
    }

    if (parser.isSet(downloadClipboardOpt)) {
        if (!cmd.files.isEmpty()) {
            cmd.action = CommandLine::Error;
            cmd.error = QCoreApplication::translate("CliArgs", "--download-clipboard takes no file arguments");
            return cmd;
        }
        cmd.action = CommandLine::DownloadClipboard;
        return cmd;
    }

    if (parser.isSet(shelfOpt)) {
        cmd.action = cmd.files.isEmpty() ? CommandLine::ToggleShelf : CommandLine::AddToShelf;
        return cmd;
    }

    if (parser.isSet(wheelOpt)) {
        const QString mode = parser.value(modeOpt).trimmed().toLower();
        if (!isValidMode(mode)) {
            cmd.action = CommandLine::Error;
            cmd.error = QCoreApplication::translate("CliArgs", "Unknown mode \"%1\" (expected convert, tools or actions)").arg(mode);
            return cmd;
        }
        cmd.mode = mode;
        cmd.action = cmd.files.isEmpty() ? CommandLine::WheelClipboard : CommandLine::Wheel;
        return cmd;
    }
    if (!cmd.files.isEmpty()) {
        cmd.action = CommandLine::OpenFiles;
        return cmd;
    }
    cmd.action = parser.isSet(backgroundOpt) ? CommandLine::Background : CommandLine::Start;
    return cmd;
}

ForwardCall forwardCallFor(const CommandLine &cmd)
{
    switch (cmd.action) {
    case CommandLine::Start:
        return {kInterface, QStringLiteral("Activate"), {}};
    case CommandLine::Wheel:
        return {kInterface, QStringLiteral("ShowWheel"), {cmd.files, cmd.mode}};
    case CommandLine::WheelClipboard:
        return {kInterface, QStringLiteral("ShowWheelFromClipboard"), {cmd.mode}};
    case CommandLine::Download:
        return {kInterface, QStringLiteral("Download"), {cmd.url}};
    case CommandLine::DownloadClipboard:
        return {kInterface, QStringLiteral("DownloadClipboard"), {}};
    case CommandLine::ToggleShelf:
        return {kInterface, QStringLiteral("ToggleShelf"), {}};
    case CommandLine::AddToShelf:
        return {kInterface, QStringLiteral("AddToShelf"), {cmd.files}};
    case CommandLine::OpenFiles: {
        // Nashi1 has no "open these files in Home" method, so use the standard
        // org.freedesktop.Application.Open(as uris, a{sv} platform_data).
        QStringList uris;
        for (const QString &f : cmd.files)
            uris.append(QUrl::fromLocalFile(f).toString(QUrl::FullyEncoded));
        return {kFdoAppInterface, QStringLiteral("Open"), {uris, QVariantMap{}}};
    }
    case CommandLine::Background: // already running: nothing to do
    case CommandLine::ShowHelp:
    case CommandLine::ShowVersion:
    case CommandLine::Error:
        break;
    }
    return {};
}

} // namespace CliArgs
