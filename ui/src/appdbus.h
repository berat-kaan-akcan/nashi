#pragma once

#include <QDBusAbstractAdaptor>
#include <QStringList>
#include <QVariantMap>

class App;

// io.github.beratkaanakcan.Nashi1 (docs/DBUS.md).
class NashiAdaptor : public QDBusAbstractAdaptor
{
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "io.github.beratkaanakcan.Nashi1")

public:
    explicit NashiAdaptor(App *app);

public Q_SLOTS:
    void ShowWheel(const QStringList &paths, const QString &mode);
    void ShowWheelFromClipboard(const QString &mode);
    void Activate();

private:
    App *m_app;
};

// org.freedesktop.Application (desktop-entry D-Bus activation). Used for
// `nashi FILE…` forwarding, which Nashi1 has no method for.
class FdoApplicationAdaptor : public QDBusAbstractAdaptor
{
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "org.freedesktop.Application")

public:
    explicit FdoApplicationAdaptor(App *app);

public Q_SLOTS:
    void Activate(const QVariantMap &platformData);
    void Open(const QStringList &uris, const QVariantMap &platformData);
    void ActivateAction(const QString &name, const QVariantList &parameter, const QVariantMap &platformData);

private:
    App *m_app;
};
