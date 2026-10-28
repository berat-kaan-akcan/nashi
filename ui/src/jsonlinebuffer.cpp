#include "jsonlinebuffer.h"

#include <QJsonDocument>
#include <QJsonParseError>

QList<QJsonObject> JsonLineBuffer::append(const QByteArray &chunk)
{
    QList<QJsonObject> result;
    m_pending.append(chunk);
    qsizetype start = 0;
    while (true) {
        const qsizetype nl = m_pending.indexOf('\n', start);
        if (nl < 0)
            break;
        QJsonObject obj;
        if (parseLine(m_pending.mid(start, nl - start), &obj))
            result.append(obj);
        start = nl + 1;
    }
    m_pending.remove(0, start);
    return result;
}

QList<QJsonObject> JsonLineBuffer::flush()
{
    QList<QJsonObject> result;
    QJsonObject obj;
    if (parseLine(m_pending, &obj))
        result.append(obj);
    m_pending.clear();
    return result;
}

void JsonLineBuffer::clear()
{
    m_pending.clear();
    m_malformed = 0;
}

bool JsonLineBuffer::parseLine(const QByteArray &raw, QJsonObject *out)
{
    const QByteArray line = raw.trimmed();
    if (line.isEmpty())
        return false;
    QJsonParseError err{};
    const QJsonDocument doc = QJsonDocument::fromJson(line, &err);
    if (err.error != QJsonParseError::NoError || !doc.isObject()) {
        ++m_malformed;
        return false;
    }
    *out = doc.object();
    return true;
}
