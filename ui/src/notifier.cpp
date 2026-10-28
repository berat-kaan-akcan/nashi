#include "notifier.h"

#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QPointer>
#include <QVariantMap>

namespace {
const QString kService = QStringLiteral("org.freedesktop.Notifications");
const QString kPath = QStringLiteral("/org/freedesktop/Notifications");
const QString kIface = QStringLiteral("org.freedesktop.Notifications");
} // namespace

Notifier::Notifier(QObject *parent)
    : QObject(parent)
{
    auto bus = QDBusConnection::sessionBus();
    if (!bus.isConnected())
        return;
    bus.connect(kService, kPath, kIface, QStringLiteral("ActionInvoked"), this, SLOT(onActionInvoked(uint, QString)));
    bus.connect(kService, kPath, kIface, QStringLiteral("NotificationClosed"), this, SLOT(onClosed(uint, uint)));
}

void Notifier::notifyFinished(const QString &summary, const QString &body, const QStringList &outputs, bool failed)
{
    auto bus = QDBusConnection::sessionBus();
    if (!bus.isConnected())
        return;
    QStringList actions;
    if (!outputs.isEmpty()) {
        if (outputs.size() == 1)
            actions << QStringLiteral("default") << tr("Open") << QStringLiteral("open") << tr("Open");
        actions << QStringLiteral("reveal") << tr("Show in folder");
    }
    const QVariantMap hints{
        {QStringLiteral("desktop-entry"), QStringLiteral("io.github.beratkaanakcan.Nashi")},
        {QStringLiteral("urgency"), QVariant::fromValue<uchar>(failed ? 2 : 1)},
    };
    QDBusMessage msg = QDBusMessage::createMethodCall(kService, kPath, kIface, QStringLiteral("Notify"));
    msg << QStringLiteral("Nashi") << uint(0) << QStringLiteral("io.github.beratkaanakcan.Nashi") << summary << body
        << actions << hints << int(-1);
    auto *watcher = new QDBusPendingCallWatcher(bus.asyncCall(msg), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this, outputs](QDBusPendingCallWatcher *w) {
        const QDBusPendingReply<uint> reply = *w;
        if (!reply.isError() && !outputs.isEmpty())
            m_outputs.insert(reply.value(), outputs);
        w->deleteLater();
    });
}

void Notifier::onActionInvoked(uint id, const QString &key)
{
    const auto it = m_outputs.constFind(id);
    if (it == m_outputs.constEnd())
        return;
    if ((key == QLatin1String("open") || key == QLatin1String("default")) && !it->isEmpty())
        Q_EMIT openRequested(it->constFirst());
    else if (key == QLatin1String("reveal"))
        Q_EMIT revealRequested(*it);
}

void Notifier::onClosed(uint id, uint)
{
    m_outputs.remove(id);
}
