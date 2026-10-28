#include "cliargs.h"

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
    parser.addOption(helpOpt);
    parser.addOption(versionOpt);
    parser.addOption(backgroundOpt);
    parser.addOption(wheelOpt);
    parser.addOption(modeOpt);
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
