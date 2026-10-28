#pragma once

#include <QHash>
#include <QObject>
#include <QStringList>

// org.freedesktop.Notifications client (QtDBus). Notifications carry
// "Open" / "Show in folder" actions for finished jobs.
class Notifier : public QObject
{
    Q_OBJECT

public:
    explicit Notifier(QObject *parent = nullptr);

    void notifyFinished(const QString &summary, const QString &body, const QStringList &outputs, bool failed);

Q_SIGNALS:
    void openRequested(const QString &path);
    void revealRequested(const QStringList &paths);

private Q_SLOTS:
    void onActionInvoked(uint id, const QString &key);
    void onClosed(uint id, uint reason);

private:
    QHash<uint, QStringList> m_outputs;
};
