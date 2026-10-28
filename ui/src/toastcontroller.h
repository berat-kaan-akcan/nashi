#pragma once

#include <QObject>
#include <QPointer>
#include <QTimer>
#include <QtQmlIntegration/qqmlintegration.h>

class JobManager;
class QQmlEngine;
class QQuickWindow;

// Shows ToastWindow.qml while jobs run and for a few seconds after they end.
class ToastController : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Provided by the application")

    Q_PROPERTY(bool shown READ shown NOTIFY shownChanged)

public:
    static constexpr int kLingerMs = 8000;

    ToastController(JobManager *jobs, QObject *parent = nullptr);
    void setEngine(QQmlEngine *engine) { m_engine = engine; }

    bool shown() const { return m_shown; }
    Q_INVOKABLE void hold(bool on);
    Q_INVOKABLE void dismiss();
    Q_INVOKABLE void contentResized();

Q_SIGNALS:
    void shownChanged();

private:
    void show();
    void hide();
    void ensureWindow();
    void setShown(bool on);

    JobManager *m_jobs;
    QQmlEngine *m_engine = nullptr;
    QPointer<QQuickWindow> m_window;
    QTimer m_linger;
    bool m_shown = false;
    bool m_held = false;
};
