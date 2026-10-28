#pragma once

#include <QDateTime>
#include <QHash>
#include <QJsonObject>
#include <QObject>
#include <QStringList>
#include <functional>

class QProcess;

// Runs a one-shot `nashi-cli … --json` command and hands back its JSON object.
// The process is owned by `context` and killed with it.
namespace CliCall {
using Callback = std::function<void(bool ok, const QJsonObject &result, const QString &error)>;
void run(QObject *context, const QString &program, const QStringList &args, Callback cb, int timeoutMs = 30000);
}

// Async `nashi-cli wheel FILE… --json`, cached per file set.
class WheelQuery : public QObject
{
    Q_OBJECT

public:
    explicit WheelQuery(QObject *parent = nullptr);

    void setProgram(const QString &program) { m_program = program; }
    QString program() const { return m_program; }
    void setCacheTtlMs(qint64 ms) { m_ttlMs = ms; }

    static QString keyFor(const QStringList &files);

    // Starts (or answers from cache) a query; returns its key. `ready` or
    // `failed` is always emitted asynchronously, never re-entrantly.
    QString request(const QStringList &files);
    bool cached(const QStringList &files, QJsonObject *out = nullptr) const;
    void invalidate() { m_cache.clear(); }

Q_SIGNALS:
    void ready(const QString &key, const QJsonObject &wheel);
    void failed(const QString &key, const QString &message);

private:
    struct Entry
    {
        QJsonObject wheel;
        qint64 at = 0;
    };
    QString m_program;
    qint64 m_ttlMs = 60000;
    QHash<QString, Entry> m_cache;
    QHash<QString, bool> m_inFlight;
};
