#include "surfaces.h"
#include "overlaycontroller.h"

#include <QCursor>
#include <QGuiApplication>
#include <QLoggingCategory>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickWindow>
#include <QScreen>

#ifdef NASHI_HAVE_LAYERSHELL
#include <LayerShellQt/Window>
#endif

Q_LOGGING_CATEGORY(lcSurfaces, "nashi.surfaces")

namespace Surfaces {

bool layerShellCompiled()
{
#ifdef NASHI_HAVE_LAYERSHELL
    return true;
#else
    return false;
#endif
}

bool isWayland()
{
    return QGuiApplication::platformName().startsWith(QLatin1String("wayland"));
}

bool layerShellActive()
{
    return layerShellCompiled() && isWayland();
}

void configureOverlay(QWindow *window, QScreen *screen)
{
    window->setScreen(screen);
#ifdef NASHI_HAVE_LAYERSHELL
    if (layerShellActive()) {
        window->setFlag(Qt::FramelessWindowHint);
        auto *ls = LayerShellQt::Window::get(window);
        ls->setLayer(LayerShellQt::Window::LayerOverlay);
        ls->setAnchors(LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorTop | LayerShellQt::Window::AnchorBottom |
                                                     LayerShellQt::Window::AnchorLeft | LayerShellQt::Window::AnchorRight));
        ls->setExclusiveZone(-1);
        ls->setKeyboardInteractivity(LayerShellQt::Window::KeyboardInteractivityNone);
        ls->setActivateOnShow(false);
        ls->setScope(QStringLiteral("nashi-overlay"));
        ls->setScreen(screen);
        window->resize(screen->size());
        return;
    }
#endif
    window->setFlags(Qt::FramelessWindowHint | Qt::WindowStaysOnTopHint | Qt::Tool | Qt::WindowDoesNotAcceptFocus);
    window->setGeometry(screen->geometry());
}

void setOverlayKeyboard(QWindow *window, bool wantsKeyboard)
{
#ifdef NASHI_HAVE_LAYERSHELL
    if (layerShellActive()) {
        LayerShellQt::Window::get(window)->setKeyboardInteractivity(
            wantsKeyboard ? LayerShellQt::Window::KeyboardInteractivityOnDemand : LayerShellQt::Window::KeyboardInteractivityNone);
        if (wantsKeyboard && window->isVisible())
            window->requestActivate();
        return;
    }
#endif
    window->setFlag(Qt::WindowDoesNotAcceptFocus, !wantsKeyboard);
    if (wantsKeyboard && window->isVisible())
        window->requestActivate();
}

void configureToast(QWindow *window)
{
#ifdef NASHI_HAVE_LAYERSHELL
    if (layerShellActive()) {
        window->setFlag(Qt::FramelessWindowHint);
        auto *ls = LayerShellQt::Window::get(window);
        ls->setLayer(LayerShellQt::Window::LayerTop);
        ls->setAnchors(LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorTop | LayerShellQt::Window::AnchorRight));
        ls->setMargins(QMargins(0, 12, 12, 0));
        ls->setExclusiveZone(0);
        ls->setKeyboardInteractivity(LayerShellQt::Window::KeyboardInteractivityNone);
        ls->setActivateOnShow(false);
        ls->setScope(QStringLiteral("nashi-toast"));
        ls->setWantsToBeOnActiveScreen(true);
        return;
    }
#endif
    window->setFlags(Qt::FramelessWindowHint | Qt::WindowStaysOnTopHint | Qt::Tool | Qt::WindowDoesNotAcceptFocus);
}

void placeToastFallback(QWindow *window)
{
    if (layerShellActive())
        return;
    QScreen *screen = QGuiApplication::screenAt(QCursor::pos());
    if (!screen)
        screen = QGuiApplication::primaryScreen();
    if (!screen)
        return;
    const QRect avail = screen->availableGeometry();
    window->setPosition(avail.right() - window->width() - 12, avail.top() + 12);
}

} // namespace Surfaces

OverlaySurfaces::OverlaySurfaces(QQmlEngine *engine, OverlayController *controller, QObject *parent)
    : QObject(parent)
    , m_engine(engine)
    , m_controller(controller)
{
    const auto screens = QGuiApplication::screens();
    for (QScreen *s : screens)
        createFor(s);
    connect(qGuiApp, &QGuiApplication::screenAdded, this, &OverlaySurfaces::createFor);
    connect(qGuiApp, &QGuiApplication::screenRemoved, this, &OverlaySurfaces::removeFor);
    connect(m_controller, &OverlayController::activeChanged, this, &OverlaySurfaces::sync);
    connect(m_controller, &OverlayController::stageChanged, this, &OverlaySurfaces::syncKeyboard);
    connect(m_controller, &OverlayController::shown, this, [this] {
        sync();
        if (!Surfaces::isWayland()) {
            // Outside Wayland the cursor position is known: pre-place the wheel.
            QPointF local;
            if (QQuickWindow *w = windowAtCursor(&local))
                m_controller->setInitialHost(w, local);
        }
    });
}

OverlaySurfaces::~OverlaySurfaces()
{
    for (const auto &w : std::as_const(m_windows))
        delete w.data();
}

QQuickWindow *OverlaySurfaces::windowFor(QScreen *screen) const
{
    return m_windows.value(screen).data();
}

QQuickWindow *OverlaySurfaces::windowAtCursor(QPointF *localPos) const
{
    const QPoint global = QCursor::pos();
    QScreen *screen = QGuiApplication::screenAt(global);
    if (!screen || Surfaces::isWayland())
        screen = QGuiApplication::primaryScreen();
    QQuickWindow *w = windowFor(screen);
    if (!w && !m_windows.isEmpty())
        w = m_windows.begin()->data();
    if (w && localPos) {
        if (Surfaces::isWayland())
            *localPos = QPointF(w->width() / 2.0, w->height() / 2.0);
        else
            *localPos = QPointF(global - screen->geometry().topLeft());
    }
    return w;
}

QList<QQuickWindow *> OverlaySurfaces::windows() const
{
    QList<QQuickWindow *> out;
    for (const auto &w : m_windows)
        if (w)
            out.append(w);
    return out;
}

void OverlaySurfaces::createFor(QScreen *screen)
{
    if (m_windows.contains(screen))
        return;
    QQmlComponent component(m_engine);
    component.loadFromModule("NashiApp", "OverlayWindow");
    QObject *obj = component.create();
    auto *window = qobject_cast<QQuickWindow *>(obj);
    if (!window) {
        qCWarning(lcSurfaces) << "OverlayWindow failed to load:" << component.errorString();
        delete obj;
        return;
    }
    Surfaces::configureOverlay(window, screen);
    connect(screen, &QScreen::geometryChanged, window, [window, screen] {
        if (!Surfaces::layerShellActive())
            window->setGeometry(screen->geometry());
    });
    m_windows.insert(screen, window);
    if (m_controller->active())
        window->show();
}

void OverlaySurfaces::removeFor(QScreen *screen)
{
    if (QQuickWindow *w = m_windows.take(screen).data()) {
        if (m_controller->hostWindow() == w)
            m_controller->cancel();
        w->deleteLater();
    }
}

void OverlaySurfaces::sync()
{
    const bool on = m_controller->active();
    for (const auto &w : std::as_const(m_windows)) {
        if (!w)
            continue;
        if (on) {
            w->show();
        } else {
            w->hide();
            Surfaces::setOverlayKeyboard(w, false);
        }
    }
    syncKeyboard();
}

void OverlaySurfaces::syncKeyboard()
{
    auto *host = qobject_cast<QQuickWindow *>(m_controller->hostWindow());
    for (const auto &w : std::as_const(m_windows)) {
        if (w && w->isVisible())
            Surfaces::setOverlayKeyboard(w, m_controller->wantsKeyboard() && w == host);
    }
}
