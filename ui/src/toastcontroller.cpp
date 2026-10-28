#include "toastcontroller.h"
#include "jobmanager.h"
#include "surfaces.h"

#include <QLoggingCategory>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickWindow>

Q_LOGGING_CATEGORY(lcToast, "nashi.toast")

ToastController::ToastController(JobManager *jobs, QObject *parent)
    : QObject(parent)
    , m_jobs(jobs)
{
    m_linger.setSingleShot(true);
    m_linger.setInterval(kLingerMs);
    connect(&m_linger, &QTimer::timeout, this, [this] {
        if (!m_held && m_jobs->activeCount() == 0)
            hide();
    });
    connect(m_jobs, &JobManager::jobStarted, this, [this] { show(); });
    connect(m_jobs, &JobManager::activeCountChanged, this, [this] {
        if (m_jobs->activeCount() == 0 && m_shown && !m_held)
            m_linger.start();
    });
}

void ToastController::hold(bool on)
{
    m_held = on;
    if (on)
        m_linger.stop();
    else if (m_shown && m_jobs->activeCount() == 0)
        m_linger.start();
}

void ToastController::dismiss()
{
    hide();
}

void ToastController::contentResized()
{
    if (m_window && m_window->isVisible())
        Surfaces::placeToastFallback(m_window);
}

void ToastController::show()
{
    m_linger.stop();
    ensureWindow();
    if (m_window) {
        Surfaces::placeToastFallback(m_window);
        m_window->show();
    }
    setShown(true);
}

void ToastController::hide()
{
    m_linger.stop();
    if (m_window)
        m_window->hide();
    m_jobs->clearFinished();
    setShown(false);
}

void ToastController::ensureWindow()
{
    if (m_window || !m_engine)
        return;
    QQmlComponent component(m_engine);
    component.loadFromModule("NashiApp", "ToastWindow");
    QObject *obj = component.create();
    m_window = qobject_cast<QQuickWindow *>(obj);
    if (!m_window) {
        qCWarning(lcToast) << "ToastWindow failed to load:" << component.errorString();
        delete obj;
        return;
    }
    Surfaces::configureToast(m_window);
}

void ToastController::setShown(bool on)
{
    if (on == m_shown)
        return;
    m_shown = on;
    Q_EMIT shownChanged();
}
