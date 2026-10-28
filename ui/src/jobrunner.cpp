#include "jobrunner.h"

#include <QFileInfo>
#include <QJsonArray>
#include <QJsonObject>

namespace {
constexpr int kDefaultKillTimeoutMs = 5000; // the CLI's own grace is ~1.5 s
constexpr int kDestructorGraceMs = 5000;
constexpr qsizetype kStderrTailBytes = 2048;
} // namespace

JobRunner::JobRunner(const QString &title, QObject *parent)
    : QObject(parent)
    , m_title(title)
{
    m_killTimer.setSingleShot(true);
    m_killTimer.setInterval(kDefaultKillTimeoutMs);
    connect(&m_killTimer, &QTimer::timeout, this, [this] {
        if (m_process && m_process->state() != QProcess::NotRunning)
            m_process->kill();
    });
}

JobRunner::~JobRunner()
{
    if (m_process && m_process->state() != QProcess::NotRunning) {
        // Give the CLI the chance to stop its children and delete temp files.
        m_process->disconnect(this);
        m_process->terminate();
        if (!m_process->waitForFinished(kDestructorGraceMs)) {
            m_process->kill();
            m_process->waitForFinished(1000);
        }
    }
}

void JobRunner::start(const QString &program, const QStringList &args)
{
    if (m_state != Idle)
        return;
    m_args = args;
    m_state = Running;
    m_clock.start();
    Q_EMIT stateChanged();

    if (program.isEmpty()) {
        addError(QString(), QStringLiteral("missing_cli"), tr("nashi-cli was not found"));
        finish(Failed);
        return;
    }

    m_process = new QProcess(this);
    m_process->setProgram(program);
    m_process->setArguments(args);
    m_process->setProcessChannelMode(QProcess::SeparateChannels);
    connect(m_process, &QProcess::readyReadStandardOutput, this,
            [this] { feed(m_process->readAllStandardOutput()); });
    connect(m_process, &QProcess::readyReadStandardError, this, [this] {
        m_stderrTail.append(m_process->readAllStandardError());
        if (m_stderrTail.size() > kStderrTailBytes)
            m_stderrTail = m_stderrTail.right(kStderrTailBytes);
    });
    connect(m_process, &QProcess::finished, this, &JobRunner::onFinished);
    connect(m_process, &QProcess::errorOccurred, this, &JobRunner::onErrorOccurred);
    m_process->start(QIODevice::ReadOnly);
}

void JobRunner::cancel()
{
    if (m_state != Running || m_cancelRequested)
        return;
    m_cancelRequested = true;
    if (m_process && m_process->state() != QProcess::NotRunning) {
        m_process->terminate(); // SIGTERM: the CLI cleans up and emits done{cancelled}
        m_killTimer.start();
    } else {
        finish(Cancelled);
    }
}

void JobRunner::kill()
{
    if (m_process && m_process->state() != QProcess::NotRunning) {
        m_cancelRequested = true;
        m_process->kill();
    }
}

void JobRunner::feed(const QByteArray &bytes)
{
    const auto events = m_buffer.append(bytes);
    for (const QJsonObject &ev : events)
        handleEvent(ev);
}

double JobRunner::overallProgress() const
{
    if (m_state > Running)
        return 1.0;
    if (m_total <= 0)
        return m_fraction < 0 ? 0.0 : m_fraction;
    const double f = m_fraction < 0 ? 0.0 : m_fraction;
    if (m_wholeJob)
        return f; // index:null → the fraction is for the combined output
    return qBound(0.0, (m_index + f) / m_total, 1.0);
}

QString JobRunner::errorSummary() const
{
    if (m_errors.isEmpty())
        return {};
    const QVariantMap first = m_errors.constFirst().toMap();
    QString text = first.value(QStringLiteral("message")).toString();
    const QString input = first.value(QStringLiteral("input")).toString();
    if (!input.isEmpty())
        text = QFileInfo(input).fileName() + QStringLiteral(": ") + text;
    if (m_errors.size() > 1)
        text += tr(" (+%n more)", nullptr, int(m_errors.size() - 1));
    return text;
}

qint64 JobRunner::elapsedMs() const
{
    if (m_state == Running && m_clock.isValid())
        return m_clock.elapsed();
    return m_elapsed;
}

void JobRunner::handleEvent(const QJsonObject &ev)
{
    const QString type = ev.value(QStringLiteral("event")).toString();
    if (type == QLatin1String("start")) {
        m_jobId = ev.value(QStringLiteral("job")).toString();
        m_total = ev.value(QStringLiteral("total")).toInt();
        Q_EMIT progressChanged();
    } else if (type == QLatin1String("progress")) {
        const QJsonValue idx = ev.value(QStringLiteral("index"));
        m_wholeJob = !idx.isDouble();
        if (idx.isDouble())
            m_index = idx.toInt();
        const QJsonValue frac = ev.value(QStringLiteral("fraction"));
        m_fraction = frac.isDouble() ? qBound(0.0, frac.toDouble(), 1.0) : -1.0;
        m_message = ev.value(QStringLiteral("message")).toString();
        Q_EMIT progressChanged();
    } else if (type == QLatin1String("output")) {
        const QString path = ev.value(QStringLiteral("path")).toString();
        const QString msg = ev.value(QStringLiteral("message")).toString();
        if (!msg.isEmpty())
            m_outputMessages.append(msg);
        if (!path.isEmpty() && !m_outputs.contains(path)) {
            m_outputs.append(path);
            Q_EMIT outputsChanged();
            Q_EMIT outputProduced(path);
        } else if (!msg.isEmpty()) {
            Q_EMIT outputsChanged();
        }
    } else if (type == QLatin1String("error")) {
        addError(ev.value(QStringLiteral("input")).toString(), ev.value(QStringLiteral("code")).toString(),
                 ev.value(QStringLiteral("message")).toString());
    } else if (type == QLatin1String("done")) {
        m_doneSeen = true;
        m_doneCancelled = ev.value(QStringLiteral("cancelled")).toBool();
        m_failed = ev.value(QStringLiteral("failed")).toInt();
        const QJsonArray outs = ev.value(QStringLiteral("outputs")).toArray();
        bool changed = false;
        for (const QJsonValue &v : outs) {
            const QString p = v.toString();
            if (!p.isEmpty() && !m_outputs.contains(p)) {
                m_outputs.append(p);
                changed = true;
            }
        }
        if (changed)
            Q_EMIT outputsChanged();
    }
    // Unknown events are ignored for forward compatibility.
}

void JobRunner::onFinished(int exitCode, QProcess::ExitStatus status)
{
    m_killTimer.stop();
    feed(m_process->readAllStandardOutput());
    for (const QJsonObject &ev : m_buffer.flush())
        handleEvent(ev);

    if (m_doneCancelled || exitCode == 130 || m_cancelRequested) {
        finish(Cancelled);
        return;
    }
    if (status == QProcess::CrashExit) {
        addError(QString(), QStringLiteral("crashed"), tr("nashi-cli crashed"));
        finish(Failed);
        return;
    }
    if (!m_doneSeen && m_errors.isEmpty()) {
        const QString tail = QString::fromUtf8(m_stderrTail).trimmed();
        addError(QString(), QStringLiteral("protocol"),
                 tail.isEmpty() ? tr("nashi-cli exited with code %1").arg(exitCode) : tail.section(QLatin1Char('\n'), -1));
    }
    const bool ok = exitCode == 0 && m_failed == 0 && m_errors.isEmpty();
    if (!ok && m_failed == 0)
        m_failed = qMax(1, int(m_errors.size()));
    finish(ok ? Succeeded : Failed);
}

void JobRunner::onErrorOccurred(QProcess::ProcessError error)
{
    if (error != QProcess::FailedToStart || m_state != Running)
        return;
    addError(QString(), QStringLiteral("missing_cli"), tr("Could not start nashi-cli: %1").arg(m_process->errorString()));
    m_failed = 1;
    finish(m_cancelRequested ? Cancelled : Failed);
}

void JobRunner::finish(State state)
{
    if (m_state != Running)
        return;
    m_elapsed = m_clock.isValid() ? m_clock.elapsed() : 0;
    m_state = state;
    Q_EMIT progressChanged();
    Q_EMIT stateChanged();
    Q_EMIT jobFinished();
}

void JobRunner::addError(const QString &input, const QString &code, const QString &message)
{
    m_errors.append(QVariantMap{{QStringLiteral("input"), input},
                                {QStringLiteral("code"), code},
                                {QStringLiteral("message"), message}});
    Q_EMIT errorsChanged();
}
