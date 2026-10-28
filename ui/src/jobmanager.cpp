#include "jobmanager.h"
#include "jobrunner.h"
#include "settings.h"

#include <QFileInfo>
#include <QTimer>

#include <memory>

JobManager::JobManager(Settings *settings, QObject *parent)
    : QAbstractListModel(parent)
    , m_settings(settings)
{
}

QStringList JobManager::buildArgs(const QString &kind, const QString &id, const QStringList &files, const QString &outDir)
{
    QStringList args;
    args << (kind == QLatin1String("convert") ? QStringLiteral("convert") : QStringLiteral("tool")) << id;
    for (const QString &f : files)
        args << QFileInfo(f).absoluteFilePath(); // absolute paths never start with '-'
    if (!outDir.isEmpty())
        args << QStringLiteral("--out-dir") << QFileInfo(outDir).absoluteFilePath();
    args << QStringLiteral("--json");
    return args;
}

JobRunner *JobManager::start(const QString &kind, const QString &id, const QString &title, const QStringList &files)
{
    if (files.isEmpty() || id.isEmpty() || (kind != QLatin1String("convert") && kind != QLatin1String("tool")))
        return nullptr;
    auto *job = new JobRunner(title, this);
    beginInsertRows({}, 0, 0);
    m_jobs.prepend(job);
    endInsertRows();
    connect(job, &JobRunner::stateChanged, this, [this, job] {
        const int row = int(m_jobs.indexOf(job));
        if (row >= 0)
            Q_EMIT dataChanged(index(row), index(row));
        Q_EMIT activeCountChanged();
    });
    connect(job, &JobRunner::jobFinished, this, [this, job] {
        Q_EMIT jobFinished(job);
        prune();
    });
    Q_EMIT countChanged();
    Q_EMIT jobStarted(job);
    job->start(m_program, buildArgs(kind, id, files, m_settings ? m_settings->effectiveOutputDir() : QString()));
    Q_EMIT activeCountChanged();
    return job;
}

void JobManager::cancelAll()
{
    for (const auto &j : std::as_const(m_jobs))
        if (j)
            j->cancel();
}

void JobManager::clearFinished()
{
    for (int i = int(m_jobs.size()) - 1; i >= 0; --i) {
        if (!m_jobs.at(i) || m_jobs.at(i)->finished()) {
            beginRemoveRows({}, i, i);
            JobRunner *j = m_jobs.takeAt(i);
            endRemoveRows();
            if (j)
                j->deleteLater();
        }
    }
    Q_EMIT countChanged();
}

void JobManager::remove(JobRunner *job)
{
    const int row = int(m_jobs.indexOf(job));
    if (row < 0)
        return;
    if (job->running()) {
        // Never delete a running process: let the CLI clean up first.
        job->cancel();
        connect(job, &JobRunner::jobFinished, this, [this, job] { remove(job); }, Qt::SingleShotConnection);
        return;
    }
    beginRemoveRows({}, row, row);
    m_jobs.removeAt(row);
    endRemoveRows();
    job->deleteLater();
    Q_EMIT countChanged();
    Q_EMIT activeCountChanged();
}

int JobManager::rowCount(const QModelIndex &parent) const
{
    return parent.isValid() ? 0 : int(m_jobs.size());
}

QVariant JobManager::data(const QModelIndex &index, int role) const
{
    if (!index.isValid() || index.row() >= m_jobs.size() || role != JobRole)
        return {};
    return QVariant::fromValue<QObject *>(m_jobs.at(index.row()).data());
}

QHash<int, QByteArray> JobManager::roleNames() const
{
    return {{JobRole, "job"}};
}

int JobManager::activeCount() const
{
    return int(std::count_if(m_jobs.cbegin(), m_jobs.cend(), [](const QPointer<JobRunner> &j) { return j && j->running(); }));
}

QList<JobRunner *> JobManager::jobs() const
{
    QList<JobRunner *> out;
    for (const auto &j : m_jobs)
        if (j)
            out.append(j);
    return out;
}

void JobManager::prune()
{
    int finishedSeen = 0;
    for (int i = 0; i < m_jobs.size(); ++i) {
        JobRunner *j = m_jobs.at(i);
        if (!j || !j->finished())
            continue;
        if (++finishedSeen > kMaxFinishedKept) {
            beginRemoveRows({}, i, i);
            m_jobs.removeAt(i);
            endRemoveRows();
            j->deleteLater();
            --i;
        }
    }
    Q_EMIT countChanged();
}

void JobManager::shutdown(int timeoutMs, std::function<void()> done)
{
    cancelAll();
    if (activeCount() == 0) {
        done();
        return;
    }
    auto finished = std::make_shared<bool>(false);
    auto finish = [this, finished, done] {
        if (*finished)
            return;
        *finished = true;
        done();
    };
    auto *timer = new QTimer(this);
    timer->setSingleShot(true);
    connect(timer, &QTimer::timeout, this, [this, timer, finish] {
        for (JobRunner *j : jobs())
            if (j->running())
                j->kill();
        timer->deleteLater();
        // Give QProcess a moment to reap the killed processes.
        QTimer::singleShot(200, this, finish);
    });
    connect(this, &JobManager::activeCountChanged, timer, [this, timer, finish] {
        if (activeCount() == 0) {
            timer->stop();
            timer->deleteLater();
            finish();
        }
    });
    timer->start(timeoutMs);
}
