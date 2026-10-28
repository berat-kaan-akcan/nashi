#include "wheelquery.h"

#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonParseError>
#include <QPointer>
#include <QProcess>
#include <QTimer>

#include <memory>

namespace CliCall {

void run(QObject *context, const QString &program, const QStringList &args, Callback cb, int timeoutMs)
{
    if (program.isEmpty()) {
        QTimer::singleShot(0, context, [cb] { cb(false, {}, QObject::tr("nashi-cli was not found")); });
        return;
    }
    auto *proc = new QProcess(context);
    proc->setProgram(program);
    proc->setArguments(args);
    auto done = std::make_shared<bool>(false);
    auto finishOnce = [proc, cb, done](bool ok, const QJsonObject &obj, const QString &err) {
        if (*done)
            return;
        *done = true;
        cb(ok, obj, err);
        proc->deleteLater();
    };
    auto *timer = new QTimer(proc);
    timer->setSingleShot(true);
    QObject::connect(timer, &QTimer::timeout, proc, [proc, finishOnce] {
        proc->kill();
        finishOnce(false, {}, QObject::tr("nashi-cli timed out"));
    });
    QObject::connect(proc, &QProcess::errorOccurred, proc, [proc, finishOnce](QProcess::ProcessError e) {
        if (e == QProcess::FailedToStart)
            finishOnce(false, {}, QObject::tr("Could not start nashi-cli: %1").arg(proc->errorString()));
    });
    QObject::connect(proc, &QProcess::finished, proc, [proc, finishOnce](int code, QProcess::ExitStatus) {
        const QByteArray out = proc->readAllStandardOutput();
        QJsonParseError perr{};
        const QJsonDocument doc = QJsonDocument::fromJson(out, &perr);
        if (perr.error == QJsonParseError::NoError && doc.isObject()) {
            finishOnce(true, doc.object(), QString());
            return;
        }
        QString err = QString::fromUtf8(proc->readAllStandardError()).trimmed();
        if (err.isEmpty())
            err = QObject::tr("nashi-cli exited with code %1").arg(code);
        finishOnce(false, {}, err);
    });
    timer->start(timeoutMs);
    proc->start(QIODevice::ReadOnly);
}

} // namespace CliCall

WheelQuery::WheelQuery(QObject *parent)
    : QObject(parent)
{
}

QString WheelQuery::keyFor(const QStringList &files)
{
    QStringList sorted = files;
    sorted.sort();
    return sorted.join(QLatin1Char('\n'));
}

bool WheelQuery::cached(const QStringList &files, QJsonObject *out) const
{
    const auto it = m_cache.constFind(keyFor(files));
    if (it == m_cache.constEnd())
        return false;
    if (QDateTime::currentMSecsSinceEpoch() - it->at > m_ttlMs)
        return false;
    if (out)
        *out = it->wheel;
    return true;
}

QString WheelQuery::request(const QStringList &files)
{
    const QString key = keyFor(files);
    QJsonObject hit;
    if (cached(files, &hit)) {
        QTimer::singleShot(0, this, [this, key, hit] { Q_EMIT ready(key, hit); });
        return key;
    }
    if (m_inFlight.contains(key))
        return key;
    m_inFlight.insert(key, true);

    QStringList args{QStringLiteral("wheel")};
    for (const QString &f : files)
        args.append(QFileInfo(f).absoluteFilePath()); // absolute → never starts with '-'
    args.append(QStringLiteral("--json"));

    QPointer<WheelQuery> self(this);
    CliCall::run(this, m_program, args, [self, key](bool ok, const QJsonObject &obj, const QString &err) {
        if (!self)
            return;
        self->m_inFlight.remove(key);
        if (ok) {
            self->m_cache.insert(key, {obj, QDateTime::currentMSecsSinceEpoch()});
            Q_EMIT self->ready(key, obj);
        } else {
            Q_EMIT self->failed(key, err);
        }
    });
    return key;
}
