#include "windowplacer.h"

#include <QDBusConnection>
#include <QDBusConnectionInterface>
#include <QDBusMessage>
#include <QDBusReply>
#include <QDir>
#include <QFile>
#include <QLoggingCategory>
#include <QStandardPaths>
#include <QTimer>
#include <QUuid>

Q_LOGGING_CATEGORY(lcPlacer, "nashi.windowplacer")

namespace {
struct Layout
{
    const char *id;
    double x, y, w, h;
    bool maximize;
};

constexpr Layout kLayouts[] = {
    {"left", 0, 0, 0.5, 1, false},
    {"right", 0.5, 0, 0.5, 1, false},
    {"top", 0, 0, 1, 0.5, false},
    {"bottom", 0, 0.5, 1, 0.5, false},
    {"topLeft", 0, 0, 0.5, 0.5, false},
    {"topRight", 0.5, 0, 0.5, 0.5, false},
    {"bottomLeft", 0, 0.5, 0.5, 0.5, false},
    {"bottomRight", 0.5, 0.5, 0.5, 0.5, false},
    {"leftThird", 0, 0, 1.0 / 3, 1, false},
    {"centerThird", 1.0 / 3, 0, 1.0 / 3, 1, false},
    {"rightThird", 2.0 / 3, 0, 1.0 / 3, 1, false},
    {"maximize", 0, 0, 1, 1, true},
};

const Layout *find(const QString &id)
{
    for (const Layout &l : kLayouts) {
        if (id == QLatin1String(l.id))
            return &l;
    }
    return nullptr;
}

constexpr auto kService = "org.kde.KWin";

QString scriptDir()
{
    return QStandardPaths::writableLocation(QStandardPaths::CacheLocation) + QStringLiteral("/kwin");
}
} // namespace

WindowPlacer::WindowPlacer(QObject *parent)
    : QObject(parent)
{
}

QStringList WindowPlacer::layouts()
{
    QStringList ids;
    for (const Layout &l : kLayouts)
        ids << QLatin1String(l.id);
    return ids;
}

QVariantList WindowPlacer::fractions(const QString &layout)
{
    const Layout *l = find(layout);
    if (!l)
        return {};
    return {l->x, l->y, l->w, l->h};
}

QString WindowPlacer::scriptFor(const QString &layout)
{
    const Layout *l = find(layout);
    if (!l)
        return {};
    // Only numbers from the table above are written into the script.
    const QString frac = QStringLiteral("[%1, %2, %3, %4]")
                             .arg(l->x, 0, 'f', 6)
                             .arg(l->y, 0, 'f', 6)
                             .arg(l->w, 0, 'f', 6)
                             .arg(l->h, 0, 'f', 6);
    return QStringLiteral(R"JS((function () {
  var f = %1;
  var maximize = %2;
  function mine(w) {
    return String(w.resourceClass).toLowerCase().indexOf("nashi") >= 0
        || String(w.desktopFileName).toLowerCase().indexOf("nashi") >= 0;
  }
  function here(w) {
    if (w.onAllDesktops || w.desktops.length === 0)
      return true;
    for (var i = 0; i < w.desktops.length; i++) {
      if (w.desktops[i].id === workspace.currentDesktop.id)
        return true;
    }
    return false;
  }
  var list = workspace.stackingOrder;
  var target = null;
  for (var i = list.length - 1; i >= 0 && !target; i--) {
    var w = list[i];
    if (w.normalWindow && !w.minimized && !w.fullScreen && w.moveable && w.resizeable && !mine(w) && here(w))
      target = w;
  }
  if (!target)
    return;
  if (maximize) {
    target.setMaximize(true, true);
    return;
  }
  var a = workspace.clientArea(KWin.MaximizeArea, target);
  target.setMaximize(false, false);
  target.frameGeometry = {
    x: Math.round(a.x + a.width * f[0]),
    y: Math.round(a.y + a.height * f[1]),
    width: Math.round(a.width * f[2]),
    height: Math.round(a.height * f[3])
  };
})();
)JS")
        .arg(frac, l->maximize ? QStringLiteral("true") : QStringLiteral("false"));
}

bool WindowPlacer::available() const
{
    QDBusConnectionInterface *bus = QDBusConnection::sessionBus().interface();
    return bus && bus->isServiceRegistered(QLatin1String(kService));
}

bool WindowPlacer::place(const QString &layout)
{
    const QString script = scriptFor(layout);
    if (script.isEmpty() || !available()) {
        Q_EMIT placed(layout, false);
        return false;
    }
    QDir().mkpath(scriptDir());
    const QString name = QStringLiteral("nashi-place-") + QUuid::createUuid().toString(QUuid::Id128);
    const QString path = scriptDir() + QLatin1Char('/') + name + QStringLiteral(".js");
    QFile file(path);
    if (!file.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        qCWarning(lcPlacer) << "cannot write" << path;
        Q_EMIT placed(layout, false);
        return false;
    }
    file.write(script.toUtf8());
    file.close();

    QDBusConnection bus = QDBusConnection::sessionBus();
    QDBusMessage load = QDBusMessage::createMethodCall(QLatin1String(kService), QStringLiteral("/Scripting"),
                                                       QStringLiteral("org.kde.kwin.Scripting"),
                                                       QStringLiteral("loadScript"));
    load << path << name;
    const QDBusReply<int> id = bus.call(load, QDBus::Block, 2000);
    bool ok = id.isValid() && id.value() >= 0;
    if (ok) {
        QDBusMessage run = QDBusMessage::createMethodCall(
            QLatin1String(kService), QStringLiteral("/Scripting/Script%1").arg(id.value()),
            QStringLiteral("org.kde.kwin.Script"), QStringLiteral("run"));
        ok = bus.call(run, QDBus::Block, 2000).type() != QDBusMessage::ErrorMessage;
    } else {
        qCWarning(lcPlacer) << "KWin refused the script:" << id.error().message();
    }
    // KWin has compiled the script by now; the file is no longer needed.
    QTimer::singleShot(3000, this, [path] { QFile::remove(path); });
    Q_EMIT placed(layout, ok);
    return ok;
}
