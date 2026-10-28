#include "app.h"
#include "cliargs.h"
#include "iconprovider.h"

#include <QApplication>
#include <QDBusConnection>
#include <QDBusConnectionInterface>
#include <QDBusMessage>
#include <QDir>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QtQml/qqmlextensionplugin.h>

#include <cstdio>

Q_IMPORT_QML_PLUGIN(NashiAppPlugin)

namespace {

int forwardToRunningInstance(const CommandLine &cmd)
{
    const ForwardCall call = CliArgs::forwardCallFor(cmd);
    if (!call.valid())
        return 0; // e.g. `--background` while already running
    QDBusMessage msg = QDBusMessage::createMethodCall(CliArgs::kService, CliArgs::kPath, call.interface, call.method);
    msg.setArguments(call.args);
    const QDBusMessage reply = QDBusConnection::sessionBus().call(msg, QDBus::Block, 5000);
    if (reply.type() == QDBusMessage::ErrorMessage) {
        std::fprintf(stderr, "nashi: forwarding to the running instance failed: %s\n", qPrintable(reply.errorMessage()));
        return 1;
    }
    return 0;
}

} // namespace

int main(int argc, char *argv[])
{
    QApplication::setApplicationName(QStringLiteral("nashi"));
    QApplication::setApplicationDisplayName(QStringLiteral("Nashi"));
    QApplication::setOrganizationName(QStringLiteral("nashi"));
    QApplication::setApplicationVersion(QStringLiteral(NASHI_VERSION));
    QApplication::setDesktopFileName(QStringLiteral("io.github.beratkaanakcan.Nashi"));
    QApplication qapp(argc, argv);
    QApplication::setQuitOnLastWindowClosed(false);
    QApplication::setWindowIcon(IconProvider::appIcon());

    const CommandLine cmd = CliArgs::parse(QApplication::arguments(), QDir::currentPath());
    switch (cmd.action) {
    case CommandLine::ShowHelp:
        std::fputs(qPrintable(cmd.helpText), stdout);
        return 0;
    case CommandLine::ShowVersion:
        std::printf("nashi %s\n", NASHI_VERSION);
        return 0;
    case CommandLine::Error:
        std::fprintf(stderr, "nashi: %s\n%s", qPrintable(cmd.error), qPrintable(cmd.helpText));
        return 2;
    default:
        break;
    }

    App app(App::Options{});

    // Single instance: export the object first, then try to own the name.
    auto bus = QDBusConnection::sessionBus();
    if (bus.isConnected()) {
        app.registerDBusObject();
        if (!bus.registerService(CliArgs::kService)) {
            const QString owner = bus.interface() ? bus.interface()->serviceOwner(CliArgs::kService).value() : QString();
            if (!owner.isEmpty() && owner != bus.baseService())
                return forwardToRunningInstance(cmd);
            std::fprintf(stderr, "nashi: could not register %s; continuing without single-instance support\n",
                         qPrintable(CliArgs::kService));
        }
    } else {
        std::fprintf(stderr, "nashi: no session D-Bus; running without single-instance support\n");
    }

    QQmlApplicationEngine engine;
    app.init(&engine);

    switch (cmd.action) {
    case CommandLine::Background:
        break;
    case CommandLine::Wheel:
        app.showWheel(cmd.files, cmd.mode);
        break;
    case CommandLine::WheelClipboard:
        app.showWheelFromClipboard(cmd.mode);
        break;
    case CommandLine::OpenFiles:
        app.openFiles(cmd.files);
        break;
    default:
        app.showHome();
        break;
    }
    return QApplication::exec();
}
