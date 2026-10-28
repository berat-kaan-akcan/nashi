#pragma once

#include <QByteArray>
#include <QJsonObject>
#include <QList>

// Splits a byte stream into newline-terminated JSON objects.
// Partial lines are kept until the rest arrives; blank lines are ignored and
// lines that are not a JSON object are counted as malformed.
class JsonLineBuffer
{
public:
    QList<QJsonObject> append(const QByteArray &chunk);
    // Parses whatever is left without a trailing newline (call at EOF).
    QList<QJsonObject> flush();

    int malformedCount() const { return m_malformed; }
    bool hasPending() const { return !m_pending.isEmpty(); }
    void clear();

private:
    bool parseLine(const QByteArray &line, QJsonObject *out);

    QByteArray m_pending;
    int m_malformed = 0;
};
