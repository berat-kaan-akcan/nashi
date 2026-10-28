#pragma once

#include <QAbstractListModel>
#include <QPointer>
#include <QtQmlIntegration/qqmlintegration.h>

#include <functional>

class JobRunner;
class Settings;

// The list of recent jobs (running first, newest first) shown by the toast.
class JobManager : public QAbstractListModel
{
    Q_OBJECT
    Q_MOC_INCLUDE("jobrunner.h")
    QML_ELEMENT
    QML_UNCREATABLE("Provided by the application")

    Q_PROPERTY(int count READ rowCount NOTIFY countChanged)
    Q_PROPERTY(int activeCount READ activeCount NOTIFY activeCountChanged)

public:
    enum Roles { JobRole = Qt::UserRole + 1 };
    static constexpr int kMaxFinishedKept = 6;

    JobManager(Settings *settings, QObject *parent = nullptr);

    void setProgram(const QString &program) { m_program = program; }
    QString program() const { return m_program; }

    // Builds the argv for `nashi-cli convert|tool` (pure, unit-tested).
    static QStringList buildArgs(const QString &kind, const QString &id, const QStringList &files, const QString &outDir);

    // kind: "convert" (id = target ext) or "tool" (id = tool id).
    Q_INVOKABLE JobRunner *start(const QString &kind, const QString &id, const QString &title, const QStringList &files);
    Q_INVOKABLE void cancelAll();
    Q_INVOKABLE void clearFinished();
    Q_INVOKABLE void remove(JobRunner *job);
    // Cancels every running job, waits (event loop) until all have finished
    // or `timeoutMs` passed, kills the rest, then calls `done`.
    void shutdown(int timeoutMs, std::function<void()> done);

    int rowCount(const QModelIndex &parent = QModelIndex()) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;
    int activeCount() const;
    QList<JobRunner *> jobs() const;

Q_SIGNALS:
    void countChanged();
    void activeCountChanged();
    void jobStarted(JobRunner *job);
    void jobFinished(JobRunner *job);

private:
    void prune();

    Settings *m_settings;
    QString m_program;
    QList<QPointer<JobRunner>> m_jobs;
};
