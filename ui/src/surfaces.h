#pragma once

#include <QHash>
#include <QObject>
#include <QPointer>

class OverlayController;
class QQmlEngine;
class QQuickWindow;
class QScreen;
class QWindow;

// Window-system specifics: layer-shell configuration on Wayland (when built
// with LayerShellQt) and the frameless always-on-top fallback elsewhere.
namespace Surfaces {
bool layerShellCompiled();
bool layerShellActive(); // compiled in and running on a Wayland platform
bool isWayland();
void configureOverlay(QWindow *window, QScreen *screen);
void setOverlayKeyboard(QWindow *window, bool wantsKeyboard);
void configureToast(QWindow *window);
void placeToastFallback(QWindow *window);
} // namespace Surfaces

// One transparent full-screen OverlayWindow.qml per screen; all are shown
// while the overlay is active so the drag can enter on any monitor.
class OverlaySurfaces : public QObject
{
    Q_OBJECT

public:
    OverlaySurfaces(QQmlEngine *engine, OverlayController *controller, QObject *parent = nullptr);
    ~OverlaySurfaces() override;

    QQuickWindow *windowFor(QScreen *screen) const;
    // Window + local position for "show at the cursor" requests.
    QQuickWindow *windowAtCursor(QPointF *localPos) const;
    QList<QQuickWindow *> windows() const;

private:
    void createFor(QScreen *screen);
    void removeFor(QScreen *screen);
    void sync();
    void syncKeyboard();

    QQmlEngine *m_engine;
    OverlayController *m_controller;
    QHash<QScreen *, QPointer<QQuickWindow>> m_windows;
};
