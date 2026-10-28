#pragma once

#include "jsonlinebuffer.h"

#include <QElapsedTimer>
#include <QObject>
#include <QProcess>
#include <QStringList>
#include <QTimer>
#include <QVariantList>
#include <QtQmlIntegration/qqmlintegration.h>

// Runs one `nashi-cli convert|tool … --json` process and turns its JSON-lines
// event stream (docs/CLI-PROTOCOL.md) into properties for QML.
class JobRunner : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Jobs are created by JobManager")

    Q_PROPERTY(State state READ state NOTIFY stateChanged)
    Q_PROPERTY(bool running READ running NOTIFY stateChanged)
    Q_PROPERTY(bool finished READ finished NOTIFY stateChanged)
    Q_PROPERTY(QString title READ title CONSTANT)
    Q_PROPERTY(QString jobId READ jobId NOTIFY progressChanged)
    Q_PROPERTY(int total READ total NOTIFY progressChanged)
    Q_PROPERTY(int currentIndex READ currentIndex NOTIFY progressChanged)
    Q_PROPERTY(double fraction READ fraction NOTIFY progressChanged)
    Q_PROPERTY(double overallProgress READ overallProgress NOTIFY progressChanged)
    Q_PROPERTY(bool indeterminate READ indeterminate NOTIFY progressChanged)
    Q_PROPERTY(QString message READ message NOTIFY progressChanged)
    Q_PROPERTY(QStringList outputs READ outputs NOTIFY outputsChanged)
    // Optional `message` of output events (e.g. text decoded by readQRCode).
    Q_PROPERTY(QStringList outputMessages READ outputMessages NOTIFY outputsChanged)
    Q_PROPERTY(QVariantList errors READ errors NOTIFY errorsChanged)
    Q_PROPERTY(int failedCount READ failedCount NOTIFY stateChanged)
    Q_PROPERTY(QString errorSummary READ errorSummary NOTIFY errorsChanged)
    Q_PROPERTY(qint64 elapsedMs READ elapsedMs NOTIFY stateChanged)

public:
    enum State { Idle, Running, Succeeded, Failed, Cancelled };
    Q_ENUM(State)

    explicit JobRunner(const QString &title, QObject *parent = nullptr);
    ~JobRunner() override;

    // Starts `program args…`. No shell is involved.
    void start(const QString &program, const QStringList &args);
    Q_INVOKABLE void cancel();
    // Last resort: SIGKILL a process that did not stop after cancel().
    void kill();

    // Feeds raw stdout bytes; public so the parser can be unit-tested.
    void feed(const QByteArray &bytes);

    State state() const { return m_state; }
    bool running() const { return m_state == Running; }
    bool finished() const { return m_state > Running; }
    QString title() const { return m_title; }
    QString jobId() const { return m_jobId; }
    int total() const { return m_total; }
    int currentIndex() const { return m_index; }
    double fraction() const { return m_fraction; }
    double overallProgress() const;
    bool indeterminate() const { return m_fraction < 0; }
    QString message() const { return m_message; }
    QStringList outputs() const { return m_outputs; }
    QStringList outputMessages() const { return m_outputMessages; }
    // True when the CLI reported `done` itself (clean shutdown, temp files removed).
    bool doneReceived() const { return m_doneSeen; }
    // True while progress covers the whole job (multi-input tools, index:null).
    bool wholeJobProgress() const { return m_wholeJob; }
    QVariantList errors() const { return m_errors; }
    int failedCount() const { return m_failed; }
    QString errorSummary() const;
    qint64 elapsedMs() const;
    QStringList arguments() const { return m_args; }

    void setKillTimeout(int ms) { m_killTimer.setInterval(ms); }

Q_SIGNALS:
    void stateChanged();
    void progressChanged();
    void outputsChanged();
    void errorsChanged();
    void outputProduced(const QString &path);
    void jobFinished();

private:
    void handleEvent(const QJsonObject &ev);
    void onFinished(int exitCode, QProcess::ExitStatus status);
    void onErrorOccurred(QProcess::ProcessError error);
    void finish(State state);
    void addError(const QString &input, const QString &code, const QString &message);

    QString m_title;
    QStringList m_args;
    QProcess *m_process = nullptr;
    JsonLineBuffer m_buffer;
    QTimer m_killTimer;
    QElapsedTimer m_clock;
    qint64 m_elapsed = 0;
    State m_state = Idle;
    QString m_jobId;
    int m_total = 0;
    int m_index = 0;
    double m_fraction = 0.0;
    QString m_message;
    QStringList m_outputs;
    QStringList m_outputMessages;
    bool m_wholeJob = false;
    QVariantList m_errors;
    int m_failed = 0;
    bool m_doneSeen = false;
    bool m_doneCancelled = false;
    bool m_cancelRequested = false;
    QByteArray m_stderrTail;
};
