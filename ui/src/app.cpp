#include "app.h"
#include "actions.h"
#include "appdbus.h"
#include "cliargs.h"
#include "clilocator.h"
#include "depsmodel.h"
#include "iconprovider.h"
#include "inputdclient.h"
#include "jobmanager.h"
#include "jobrunner.h"
#include "language.h"
#include "notifier.h"
#include "overlaycontroller.h"
#include "recentmodel.h"
#include "selection.h"
#include "settings.h"
#include "surfaces.h"
#include "toastcontroller.h"
#include "tray.h"
#include "wheelmodel.h"
#include "wheelquery.h"

#include <QClipboard>
#include <QDBusConnection>
#include <QFileInfo>
#include <QGuiApplication>
#include <QLoggingCategory>
#include <QMimeData>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickWindow>

Q_LOGGING_CATEGORY(lcApp, "nashi.app")

App *App::s_instance = nullptr;

namespace {
// A desktop notification is also sent when the job ran this long, since the
// user has probably looked away from the toast by then.
constexpr qint64 kLongJobMs = 10000;
constexpr int kQuitGraceMs = 5000;
} // namespace

App::App(const Options &options, QObject *parent)
    : QObject(parent)
    , m_options(options)
    , m_cliPath(CliLocator::locate())
    , m_settings(new Settings(this))
    , m_language(new Language(this))
    , m_wheelQuery(new WheelQuery(this))
    , m_jobs(new JobManager(m_settings, this))
    , m_recent(new RecentModel(&m_settings->store(), this))
    , m_deps(new DepsModel(this))
    , m_inputd(new InputdClient(this))
    , m_overlay(new OverlayController(m_wheelQuery, this))
    , m_actions(new FileActions(this))
    , m_toast(new ToastController(m_jobs, this))
    , m_home(new Selection(m_wheelQuery, this))
    , m_notifier(new Notifier(this))
{
    Q_ASSERT(!s_instance);
    s_instance = this;
    if (m_cliPath.isEmpty())
        qCWarning(lcApp) << "nashi-cli not found ($NASHI_CLI, next to nashi, PATH)";
    m_wheelQuery->setProgram(m_cliPath);
    m_jobs->setProgram(m_cliPath);
    m_deps->setProgram(m_cliPath);

    auto applyLanguage = [this] {
        m_language->apply(m_settings->language());
        m_overlay->selection()->model()->setLanguage(m_language->code());
        m_home->model()->setLanguage(m_language->code());
    };
    connect(m_settings, &Settings::languageChanged, this, applyLanguage);
    applyLanguage();

    connect(m_inputd, &InputdClient::chordStarted, m_overlay, &OverlayController::onChordStarted);
    connect(m_inputd, &InputdClient::chordReleased, m_overlay, &OverlayController::onChordReleased);
    connect(m_inputd, &InputdClient::dragEnded, m_overlay, &OverlayController::onDragEnded);
    connect(m_inputd, &InputdClient::daemonLost, m_overlay, &OverlayController::onDaemonLost);
    // Chords switched off mid-drag: drop a pending (undropped) overlay too.
    connect(m_inputd, &InputdClient::enabledChanged, m_overlay, [this] {
        if (!m_inputd->enabled())
            m_overlay->onDaemonLost();
    });
    connect(m_overlay, &OverlayController::itemChosen, this, &App::runItem);

    connect(m_jobs, &JobManager::jobFinished, this, &App::onJobFinished);
    connect(m_actions, &FileActions::feedback, this, &App::postMessage);
    connect(m_notifier, &Notifier::openRequested, m_actions, &FileActions::open);
    connect(m_notifier, &Notifier::revealRequested, m_actions, &FileActions::reveal);
}

App::~App()
{
    delete m_surfaces;
    delete m_tray;
    delete m_homeWindow.data();
    s_instance = nullptr;
}

App *App::create(QQmlEngine *, QJSEngine *engine)
{
    Q_ASSERT(s_instance);
    QJSEngine::setObjectOwnership(s_instance, QJSEngine::CppOwnership);
    Q_UNUSED(engine);
    return s_instance;
}

void App::init(QQmlEngine *engine)
{
    m_engine = engine;
    m_language->addEngine(engine);
    engine->addImageProvider(QStringLiteral("icon"), new IconProvider);
    m_toast->setEngine(engine);
    if (m_options.windows)
        m_surfaces = new OverlaySurfaces(engine, m_overlay);
    if (m_options.tray)
        m_tray = new Tray(this);
}

bool App::registerDBusObject()
{
    auto bus = QDBusConnection::sessionBus();
    if (!bus.isConnected())
        return false;
    new NashiAdaptor(this);
    new FdoApplicationAdaptor(this);
    return bus.registerObject(CliArgs::kPath, this, QDBusConnection::ExportAdaptors);
}

QString App::version() const
{
    return QStringLiteral(NASHI_VERSION);
}

bool App::layerShell() const
{
    return Surfaces::layerShellActive();
}

void App::showHome()
{
    if (!m_engine || !m_options.windows)
        return;
    if (!m_homeWindow) {
        QQmlComponent component(m_engine);
        component.loadFromModule("NashiApp", "HomeWindow");
        QObject *obj = component.create();
        m_homeWindow = qobject_cast<QQuickWindow *>(obj);
        if (!m_homeWindow) {
            qCWarning(lcApp) << "HomeWindow failed to load:" << component.errorString();
            delete obj;
            return;
        }
        m_deps->refresh();
    }
    m_homeWindow->show();
    m_homeWindow->raise();
    m_homeWindow->requestActivate();
}

void App::quit()
{
    if (m_quitting)
        return;
    m_quitting = true;
    // Let running CLIs stop their children and remove temp files first.
    m_jobs->shutdown(kQuitGraceMs, [] { QCoreApplication::quit(); });
}

void App::openFiles(const QStringList &files)
{
    QStringList existing;
    for (const QString &f : files) {
        const QFileInfo fi(f);
        if (fi.exists())
            existing.append(fi.absoluteFilePath());
    }
    if (!existing.isEmpty())
        m_home->setFiles(existing);
    showHome();
}

void App::showWheel(const QStringList &files, const QString &mode)
{
    QStringList existing;
    for (const QString &f : files) {
        const QFileInfo fi(f);
        if (fi.exists())
            existing.append(fi.absoluteFilePath());
    }
    if (existing.isEmpty()) {
        postMessage(tr("No existing files to show the wheel for"));
        return;
    }
    QPointF anchor;
    QQuickWindow *window = m_surfaces ? m_surfaces->windowAtCursor(&anchor) : nullptr;
    m_overlay->showForFiles(existing, mode.isEmpty() ? QStringLiteral("convert") : mode, window, anchor);
}

void App::showWheelFromClipboard(const QString &mode)
{
    QStringList files;
    if (const QMimeData *mime = QGuiApplication::clipboard()->mimeData()) {
        if (mime->hasUrls()) {
            files = Selection::localFiles(mime->urls());
        } else if (mime->hasText()) {
            const QStringList lines = mime->text().split(QLatin1Char('\n'), Qt::SkipEmptyParts);
            for (const QString &l : lines) {
                const QString t = l.trimmed();
                const QString p = t.startsWith(QLatin1String("file:")) ? QUrl(t).toLocalFile() : t;
                if (QFileInfo(p).isAbsolute() && QFileInfo::exists(p))
                    files.append(p);
            }
        }
    }
    if (files.isEmpty()) {
        // On Wayland the clipboard is only readable with focus; fall back to Home.
        postMessage(tr("The clipboard holds no files"));
        showHome();
        return;
    }
    showWheel(files, mode);
}

void App::runItem(const QString &kind, const QString &id, const QString &title, const QStringList &files)
{
    if (files.isEmpty())
        return;
    if (kind == QLatin1String("action")) {
        if (!m_actions->run(id, files))
            postMessage(tr("Unknown action: %1").arg(id));
        return;
    }
    const QString subject = files.size() == 1 ? QFileInfo(files.constFirst()).fileName()
                                              : tr("%n file(s)", nullptr, int(files.size()));
    const QString jobTitle = (title.isEmpty() ? id : title) + QStringLiteral(" · ") + subject;
    if (!m_jobs->start(kind, id, jobTitle, files))
        postMessage(tr("Cannot run %1").arg(id));
}

QStringList App::pathsFromUrls(const QList<QUrl> &urls) const
{
    return Selection::localFiles(urls);
}

QString App::fileName(const QString &path) const
{
    return QFileInfo(path).fileName();
}

void App::onJobFinished(JobRunner *job)
{
    m_recent->add(job->outputs());
    if (!m_settings->notifications() || job->state() == JobRunner::Cancelled)
        return;
    // Policy: the toast is the primary feedback. A desktop notification is
    // added when the toast is not on screen, or the job took long enough that
    // the user has likely looked away.
    if (m_toast->shown() && job->elapsedMs() < kLongJobMs)
        return;
    const bool failed = job->state() == JobRunner::Failed;
    QString body;
    if (failed)
        body = job->errorSummary();
    else if (job->outputs().size() == 1)
        body = QFileInfo(job->outputs().constFirst()).fileName();
    else
        body = tr("%n file(s) created", nullptr, int(job->outputs().size()));
    m_notifier->notifyFinished(failed ? tr("Failed: %1").arg(job->title()) : tr("Done: %1").arg(job->title()), body,
                               job->outputs(), failed);
}

void App::postMessage(const QString &text)
{
    m_lastMessage = text;
    qCInfo(lcApp).noquote() << text;
    Q_EMIT message(text);
}
