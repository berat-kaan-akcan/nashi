#include "inputdclient.h"

#include <QDBusConnection>
#include <QDBusConnectionInterface>
#include <QDBusMessage>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QDBusServiceWatcher>
#include <QDBusVariant>
#include <QLoggingCategory>

Q_LOGGING_CATEGORY(lcInputd, "nashi.inputd")

namespace {
const QString kProps = QStringLiteral("org.freedesktop.DBus.Properties");
}

InputdClient::InputdClient(QObject *parent)
    : QObject(parent)
{
    auto bus = QDBusConnection::sessionBus();
    if (!bus.isConnected())
        return;

    // Matches on the well-known name keep working across daemon restarts.
    bus.connect(kService, kPath, kInterface, QStringLiteral("DragStarted"), this, SLOT(onDragStarted()));
    bus.connect(kService, kPath, kInterface, QStringLiteral("ChordStarted"), this, SLOT(onChordStarted(QString)));
    bus.connect(kService, kPath, kInterface, QStringLiteral("ChordReleased"), this, SLOT(onChordReleased()));
    bus.connect(kService, kPath, kInterface, QStringLiteral("DragEnded"), this, SLOT(onDragEnded()));
    bus.connect(kService, kPath, kProps, QStringLiteral("PropertiesChanged"), this,
                SLOT(onPropertiesChanged(QString, QVariantMap, QStringList)));

    m_watcher = new QDBusServiceWatcher(kService, bus,
                                        QDBusServiceWatcher::WatchForRegistration | QDBusServiceWatcher::WatchForUnregistration,
                                        this);
    connect(m_watcher, &QDBusServiceWatcher::serviceRegistered, this, [this] {
        qCInfo(lcInputd) << "nashi-inputd appeared";
        setAvailable(true);
        fetchProperties();
    });
    connect(m_watcher, &QDBusServiceWatcher::serviceUnregistered, this, [this] {
        qCInfo(lcInputd) << "nashi-inputd went away";
        setAvailable(false);
        if (m_dragging || !m_mode.isEmpty()) {
            m_dragging = false;
            m_mode.clear();
            Q_EMIT stateChanged();
        }
        Q_EMIT daemonLost();
    });
    refresh();
}

void InputdClient::refresh()
{
    auto bus = QDBusConnection::sessionBus();
    if (!bus.isConnected() || !bus.interface())
        return;
    auto *w = new QDBusPendingCallWatcher(bus.interface()->asyncCall(QStringLiteral("NameHasOwner"), kService), this);
    connect(w, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *watcher) {
        const QDBusPendingReply<bool> reply = *watcher;
        const bool has = !reply.isError() && reply.value();
        setAvailable(has);
        if (has)
            fetchProperties();
        watcher->deleteLater();
    });
}

void InputdClient::setEnabled(bool on)
{
    if (!m_available)
        return;
    QDBusMessage msg = QDBusMessage::createMethodCall(kService, kPath, kProps, QStringLiteral("Set"));
    msg << kInterface << QStringLiteral("Enabled") << QVariant::fromValue(QDBusVariant(on));
    auto *w = new QDBusPendingCallWatcher(QDBusConnection::sessionBus().asyncCall(msg), this);
    connect(w, &QDBusPendingCallWatcher::finished, this, [this, on](QDBusPendingCallWatcher *watcher) {
        if (watcher->isError()) {
            qCWarning(lcInputd) << "Setting Enabled failed:" << watcher->error().message();
        } else if (m_enabled != on) {
            m_enabled = on;
            Q_EMIT enabledChanged();
        }
        watcher->deleteLater();
    });
}

void InputdClient::setAvailable(bool on)
{
    if (on == m_available)
        return;
    m_available = on;
    Q_EMIT availableChanged();
}

void InputdClient::fetchProperties()
{
    QDBusMessage msg = QDBusMessage::createMethodCall(kService, kPath, kProps, QStringLiteral("GetAll"));
    msg << kInterface;
    auto *w = new QDBusPendingCallWatcher(QDBusConnection::sessionBus().asyncCall(msg), this);
    connect(w, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher *watcher) {
        const QDBusPendingReply<QVariantMap> reply = *watcher;
        if (!reply.isError())
            onPropertiesChanged(kInterface, reply.value(), {});
        watcher->deleteLater();
    });
}

void InputdClient::onPropertiesChanged(const QString &iface, const QVariantMap &changed, const QStringList &)
{
    if (iface != kInterface)
        return;
    if (changed.contains(QStringLiteral("Enabled"))) {
        const bool on = changed.value(QStringLiteral("Enabled")).toBool();
        if (on != m_enabled) {
            m_enabled = on;
            Q_EMIT enabledChanged();
        }
    }
    if (changed.contains(QStringLiteral("Version"))) {
        m_version = changed.value(QStringLiteral("Version")).toString();
        Q_EMIT availableChanged();
    }
}

void InputdClient::onDragStarted()
{
    m_dragging = true;
    Q_EMIT stateChanged();
    Q_EMIT dragStarted();
}

void InputdClient::onChordStarted(const QString &mode)
{
    m_dragging = true;
    m_mode = mode;
    Q_EMIT stateChanged();
    Q_EMIT chordStarted(mode);
}

void InputdClient::onChordReleased()
{
    m_mode.clear();
    Q_EMIT stateChanged();
    Q_EMIT chordReleased();
}

void InputdClient::onDragEnded()
{
    m_dragging = false;
    m_mode.clear();
    Q_EMIT stateChanged();
    Q_EMIT dragEnded();
}
