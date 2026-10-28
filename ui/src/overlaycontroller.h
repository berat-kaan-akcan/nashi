#pragma once

#include <QObject>
#include <QPointF>
#include <QPointer>
#include <QTimer>
#include <QUrl>
#include <QtQmlIntegration/qqmlintegration.h>

class Selection;
class WheelQuery;

// State machine of the drag overlay (independent of the windows so it can be
// unit-tested). Per-screen overlay windows (OverlayWindow.qml) report drag
// events; the controller decides what the wheel shows.
//
//   Hidden ──ChordStarted──▶ Target ──urls readable + wheel ready──▶ Live
//      ▲                       │  drop (urls unknown before)          │ drop on slice → itemChosen
//      │                       ▼                                      │ drop on centre → cancel
//      └──── cancel / choose ─ Pick (click a slice, Esc cancels) ◀────┘ (if wheel not ready yet)
//
// DragEnded without a drop hides the overlay after a short grace period.
class OverlayController : public QObject
{
    Q_OBJECT
    Q_MOC_INCLUDE("selection.h")
    QML_ELEMENT
    QML_UNCREATABLE("Provided by the application")

    Q_PROPERTY(bool active READ active NOTIFY activeChanged)
    Q_PROPERTY(Stage stage READ stage NOTIFY stageChanged)
    Q_PROPERTY(QString mode READ mode NOTIFY modeChanged)
    Q_PROPERTY(QObject *hostWindow READ hostWindow NOTIFY hostChanged)
    Q_PROPERTY(QPointF anchor READ anchor NOTIFY hostChanged)
    Q_PROPERTY(Selection *selection READ selection CONSTANT)
    Q_PROPERTY(bool wantsKeyboard READ wantsKeyboard NOTIFY stageChanged)

public:
    enum Stage { Hidden, Target, Live, Pick };
    Q_ENUM(Stage)

    static constexpr int kGraceMs = 300;

    explicit OverlayController(WheelQuery *query, QObject *parent = nullptr);

    bool active() const { return m_stage != Hidden; }
    Stage stage() const { return m_stage; }
    QString mode() const { return m_mode; }
    QObject *hostWindow() const { return m_host; }
    QPointF anchor() const { return m_anchor; }
    Selection *selection() const { return m_selection; }
    bool wantsKeyboard() const { return m_stage == Pick; }
    bool dropReceived() const { return m_dropped; }
    void setGraceMs(int ms) { m_grace.setInterval(ms); }

    // Called by OverlayWindow.qml.
    Q_INVOKABLE void dragEntered(QObject *window, const QList<QUrl> &urls, const QPointF &pos);
    Q_INVOKABLE void dragLeft(QObject *window);
    Q_INVOKABLE void dropped(QObject *window, const QList<QUrl> &urls, const QPointF &pos, int slice);
    Q_INVOKABLE void choose(int index);
    Q_INVOKABLE void cancel();
    Q_INVOKABLE void setMode(const QString &mode);

    // Direct "show the wheel for these files" (D-Bus ShowWheel, clipboard).
    void showForFiles(const QStringList &files, const QString &mode, QObject *window, const QPointF &anchor);
    // Pre-places the wheel (e.g. at QCursor::pos() where that is known).
    void setInitialHost(QObject *window, const QPointF &anchor);

public Q_SLOTS:
    void onChordStarted(const QString &mode);
    void onChordReleased();
    void onDragEnded();
    void onDaemonLost();

Q_SIGNALS:
    void activeChanged();
    void stageChanged();
    void modeChanged();
    void hostChanged();
    void itemChosen(const QString &kind, const QString &id, const QString &title, const QStringList &files);
    void shown(); // windows should appear (before a host is known)

private:
    void setStage(Stage s);
    void setHost(QObject *window, const QPointF &anchor);
    void reset();
    void onWheelReady();

    Selection *m_selection;
    Stage m_stage = Hidden;
    QString m_mode = QStringLiteral("convert");
    QPointer<QObject> m_host;
    QPointF m_anchor;
    bool m_dropped = false;
    bool m_inside = false;
    QTimer m_grace;
};
