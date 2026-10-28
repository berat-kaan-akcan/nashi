#include "overlaycontroller.h"
#include "selection.h"
#include "wheelmodel.h"

#include <QLoggingCategory>

Q_LOGGING_CATEGORY(lcOverlay, "nashi.overlay")

OverlayController::OverlayController(WheelQuery *query, QObject *parent)
    : QObject(parent)
    , m_selection(new Selection(query, this))
{
    m_grace.setSingleShot(true);
    m_grace.setInterval(kGraceMs);
    connect(&m_grace, &QTimer::timeout, this, [this] {
        if (!m_dropped && m_stage != Pick) {
            qCDebug(lcOverlay) << "drag ended without a drop; hiding";
            reset();
        }
    });
    connect(m_selection, &Selection::wheelReady, this, &OverlayController::onWheelReady);
}

void OverlayController::onChordStarted(const QString &mode)
{
    const QString m = WheelModel::normalizeMode(mode);
    m_grace.stop();
    if (m_stage == Hidden || m_stage == Pick) {
        if (m_stage == Pick)
            reset();
        m_dropped = false;
        setMode(m);
        setStage(Target);
        Q_EMIT shown();
        return;
    }
    setMode(m); // modifier changed mid-drag
}

void OverlayController::onChordReleased()
{
    // Keep the overlay: the user may still drop on the wheel.
}

void OverlayController::onDragEnded()
{
    if (m_stage == Hidden || m_stage == Pick || m_dropped)
        return;
    m_grace.start(); // the drop event may arrive slightly after the button release
}

void OverlayController::onDaemonLost()
{
    if (m_stage == Target || m_stage == Live) {
        if (!m_dropped)
            reset();
    }
}

void OverlayController::setMode(const QString &mode)
{
    const QString m = WheelModel::normalizeMode(mode);
    m_selection->model()->setMode(m);
    if (m == m_mode)
        return;
    m_mode = m;
    Q_EMIT modeChanged();
}

void OverlayController::dragEntered(QObject *window, const QList<QUrl> &urls, const QPointF &pos)
{
    if (m_stage != Target && m_stage != Live)
        return;
    m_inside = true;
    if (window != m_host)
        setHost(window, pos);
    const QStringList files = Selection::localFiles(urls);
    if (!files.isEmpty())
        m_selection->setFiles(files); // no-op when unchanged; answers from cache
    if (m_selection->ready() && !m_selection->files().isEmpty())
        setStage(Live);
}

void OverlayController::dragLeft(QObject *window)
{
    if (window == m_host)
        m_inside = false;
}

void OverlayController::dropped(QObject *window, const QList<QUrl> &urls, const QPointF &pos, int slice)
{
    if (m_stage != Target && m_stage != Live)
        return;
    m_grace.stop();
    m_dropped = true;
    m_inside = false;
    if (!m_host)
        setHost(window, pos);

    const QStringList files = Selection::localFiles(urls);
    if (files.isEmpty() && m_selection->files().isEmpty()) {
        qCDebug(lcOverlay) << "drop without local files; cancelling";
        reset();
        return;
    }
    const bool sameFiles = files.isEmpty() || files == m_selection->files();
    if (m_stage == Live && sameFiles && m_selection->ready()) {
        if (slice >= 0)
            choose(slice);
        else
            cancel(); // centre or far away
        return;
    }
    if (!files.isEmpty())
        m_selection->setFiles(files);
    setStage(Pick);
}

void OverlayController::choose(int index)
{
    if (m_stage != Live && m_stage != Pick)
        return;
    const QVariantMap item = m_selection->model()->get(index);
    if (item.isEmpty() || !item.value(QStringLiteral("available")).toBool()) {
        if (m_stage == Live)
            cancel();
        return; // in Pick mode unavailable slices are simply not clickable
    }
    const QStringList files = m_selection->files();
    const QString kind = item.value(QStringLiteral("kind")).toString();
    const QString id = item.value(QStringLiteral("itemId")).toString();
    const QString title = item.value(QStringLiteral("title")).toString();
    reset();
    Q_EMIT itemChosen(kind, id, title, files);
}

void OverlayController::cancel()
{
    reset();
}

void OverlayController::showForFiles(const QStringList &files, const QString &mode, QObject *window, const QPointF &anchor)
{
    reset();
    setMode(mode);
    m_dropped = true;
    m_selection->setFiles(files);
    setHost(window, anchor);
    setStage(Pick);
    Q_EMIT shown();
}

void OverlayController::setInitialHost(QObject *window, const QPointF &anchor)
{
    if (m_stage != Hidden && !m_host)
        setHost(window, anchor);
}

void OverlayController::onWheelReady()
{
    if (m_stage == Target && m_inside && !m_dropped)
        setStage(Live);
}

void OverlayController::setStage(Stage s)
{
    if (s == m_stage)
        return;
    const bool wasActive = active();
    m_stage = s;
    Q_EMIT stageChanged();
    if (wasActive != active())
        Q_EMIT activeChanged();
}

void OverlayController::setHost(QObject *window, const QPointF &anchor)
{
    m_host = window;
    m_anchor = anchor;
    Q_EMIT hostChanged();
}

void OverlayController::reset()
{
    m_grace.stop();
    m_dropped = false;
    m_inside = false;
    setStage(Hidden);
    setHost(nullptr, {});
    m_selection->clear();
}
