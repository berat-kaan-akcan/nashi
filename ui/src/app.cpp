#include "app.h"
#include "actions.h"
#include "appdbus.h"
#include "cliargs.h"
#include "clilocator.h"
#include "clipboardimport.h"
#include "depsmodel.h"
#include "iconprovider.h"
#include "inputdclient.h"
#include "jobmanager.h"
#include "jobrunner.h"
#include "language.h"
#include "notifier.h"
#include "overlaycontroller.h"
#include "previewclient.h"
#include "recentmodel.h"
#include "selection.h"
#include "settings.h"
#include "setupcheck.h"
#include "shelfmodel.h"
#include "surfaces.h"
#include "toastcontroller.h"
#include "tray.h"
#include "wheelmodel.h"
#include "wheelquery.h"

#include <QClipboard>
#include <QDBusConnection>
#include <QDir>
#include <QFileInfo>
#include <QGuiApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLibraryInfo>
#include <QLocale>
#include <QLoggingCategory>
#include <QMimeData>
#include <QMimeDatabase>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickWindow>
#include <QStandardPaths>

Q_LOGGING_CATEGORY(lcApp, "nashi.app")

App *App::s_instance = nullptr;

namespace {
// A desktop notification is also sent when the job ran this long, since the
// user has probably looked away from the toast by then.
constexpr qint64 kLongJobMs = 10000;
constexpr int kQuitGraceMs = 5000;

bool hasQmlModuleFile(QQmlEngine *engine, const QString &relativePath)
{
    QStringList roots;
    if (engine)
        roots.append(engine->importPathList());
    const QString sysQml = QLibraryInfo::path(QLibraryInfo::QmlImportsPath);
    if (!sysQml.isEmpty())
        roots.append(sysQml);
    roots.append(QStringLiteral("/usr/lib/qt6/qml"));
    for (const QString &r : std::as_const(roots)) {
        if (r.isEmpty() || r.startsWith(QLatin1String("qrc:")))
            continue;
        const QString base = r.startsWith(QLatin1String("file:")) ? QUrl(r).toLocalFile() : r;
        if (!base.isEmpty() && QFileInfo::exists(QDir(base).filePath(relativePath)))
            return true;
    }
    return false;
}

void presentWindow(QQuickWindow *window)
{
    if (!window)
        return;
    window->show();
    if (QGuiApplication::platformName() != QLatin1String("offscreen"))
        window->raise();
    window->requestActivate();
}
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
    , m_setup(new SetupCheck(&m_settings->store(), this))
    , m_notifier(new Notifier(this))
    , m_clipboard(new ClipboardImport(this))
    , m_preview(new PreviewClient(this))
    , m_shelf(new ShelfModel(&m_settings->store(), this))
{
    Q_ASSERT(!s_instance);
    s_instance = this;
    if (m_cliPath.isEmpty())
        qCWarning(lcApp) << "nashi-cli not found ($NASHI_CLI, next to nashi, PATH)";
    m_wheelQuery->setProgram(m_cliPath);
    m_jobs->setProgram(m_cliPath);
    m_deps->setProgram(m_cliPath);
    m_preview->setProgram(m_cliPath);

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
    connect(m_clipboard, &ClipboardImport::message, this, &App::postMessage);
    connect(m_clipboard, &ClipboardImport::linkPasted, this, [this](const QString &url) { showDownload(url, false); });
    connect(m_shelf, &ShelfModel::visibleChanged, this, &App::syncShelfWindow);
    connect(m_notifier, &Notifier::openRequested, m_actions, &FileActions::open);
    connect(m_notifier, &Notifier::revealRequested, m_actions, &FileActions::reveal);
}

App::~App()
{
    delete m_surfaces;
    delete m_tray;
    delete m_homeWindow.data();
    delete m_optionsWindow.data();
    delete m_welcomeWindow.data();
    delete m_downloadWindow.data();
    delete m_previewWindow.data();
    delete m_shelfWindow.data();
    delete m_shareWindow.data();
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
    Q_EMIT engineChanged();
    if (m_options.windows)
        m_surfaces = new OverlaySurfaces(engine, m_overlay);
    if (m_options.tray)
        m_tray = new Tray(this);
    if (m_options.windows && m_settings->showShelfOnStartup())
        m_shelf->show();
    // First run: the wizard comes up before anything else (never headless).
    if (m_options.windows && !m_setup->welcomeDone())
        showWelcome();
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

bool App::shareAvailable() const
{
    if (qEnvironmentVariableIntValue("NASHI_DISABLE_SHARE") != 0)
        return false;
    return hasQmlModuleFile(m_engine, QStringLiteral("org/kde/purpose/qmldir"))
           && hasQmlModuleFile(m_engine, QStringLiteral("org/kde/purpose/AlternativesView.qml"));
}

bool App::multimediaAvailable() const
{
    if (qEnvironmentVariableIntValue("NASHI_DISABLE_MULTIMEDIA") != 0)
        return false;
    if (QGuiApplication::platformName() == QLatin1String("offscreen")
        && qEnvironmentVariableIntValue("NASHI_FORCE_MULTIMEDIA") == 0) {
        return false;
    }
    return hasQmlModuleFile(m_engine, QStringLiteral("QtMultimedia/qmldir"));
}

void App::showHome()
{
    if (!m_engine || !m_options.windows)
        return;
    // While the first-run wizard is open, it finishes by showing Home itself.
    if (m_welcomeWindow && m_welcomeWindow->isVisible() && !m_setup->welcomeDone()) {
        presentWindow(m_welcomeWindow);
        return;
    }
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
    presentWindow(m_homeWindow);
}

void App::showWelcome()
{
    if (!m_engine || !m_options.windows)
        return;
    if (!m_welcomeWindow) {
        QQmlComponent component(m_engine);
        component.loadFromModule("NashiApp", "WelcomeWindow");
        QObject *obj = component.create();
        m_welcomeWindow = qobject_cast<QQuickWindow *>(obj);
        if (!m_welcomeWindow) {
            qCWarning(lcApp) << "WelcomeWindow failed to load:" << component.errorString();
            delete obj;
            return;
        }
    }
    m_setup->refresh();
    m_inputd->refresh();
    m_deps->refresh();
    QMetaObject::invokeMethod(m_welcomeWindow, "reset");
    presentWindow(m_welcomeWindow);
}

void App::finishWelcome()
{
    m_setup->setWelcomeDone(true);
    if (m_welcomeWindow)
        m_welcomeWindow->hide();
    showHome();
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
        if (id == QLatin1String("share")) {
            showShare(files);
            return;
        }
        if (id == QLatin1String("float")) {
            addToShelf(files);
            return;
        }
        if (!m_actions->run(id, files))
            postMessage(tr("Unknown action: %1").arg(id));
        return;
    }
    if (kind == QLatin1String("tool") && WheelModel::hasOptionsForm(id)) {
        showOptions(id, title, files);
        return;
    }
    startJob(kind, id, title, files, kind == QLatin1String("convert") ? convertOptions(id) : QString());
}

QString App::convertOptions(const QString &target) const
{
    if (target != QLatin1String("jpg") && target != QLatin1String("png"))
        return {};
    QJsonObject options;
    options.insert(QStringLiteral("dpi"), m_settings->pdfImageDpi());
    return QString::fromUtf8(QJsonDocument(options).toJson(QJsonDocument::Compact));
}

void App::runWithOptions(const QString &id, const QString &title, const QStringList &files, const QString &options)
{
    if (!files.isEmpty())
        startJob(QStringLiteral("tool"), id, title, files, options);
}

void App::startJob(const QString &kind, const QString &id, const QString &title, const QStringList &files,
                   const QString &options)
{
    QString overlayToClean;
    if (!options.isEmpty()) {
        const QJsonObject obj = QJsonDocument::fromJson(options.toUtf8()).object();
        overlayToClean = obj.value(QStringLiteral("overlay")).toString();
    }
    const QString subject = files.size() == 1 ? QFileInfo(files.constFirst()).fileName()
                                              : tr("%n file(s)", nullptr, int(files.size()));
    const QString jobTitle = (title.isEmpty() ? id : title) + QStringLiteral(" · ") + subject;
    JobRunner *job = m_jobs->start(kind, id, jobTitle, files, options);
    if (!job) {
        if (!overlayToClean.isEmpty())
            m_preview->removeAnnotationFile(overlayToClean);
        postMessage(tr("Cannot run %1").arg(id));
        return;
    }
    if (!overlayToClean.isEmpty()) {
        connect(job, &JobRunner::jobFinished, m_preview,
                [this, overlayToClean] { m_preview->removeAnnotationFile(overlayToClean); });
    }
}

void App::showOptions(const QString &id, const QString &title, const QStringList &files)
{
    if (!m_engine || files.isEmpty())
        return;
    if (!m_optionsWindow) {
        QQmlComponent component(m_engine);
        component.loadFromModule("NashiApp", "OptionsWindow");
        QObject *obj = component.create();
        m_optionsWindow = qobject_cast<QQuickWindow *>(obj);
        if (!m_optionsWindow) {
            qCWarning(lcApp) << "OptionsWindow failed to load:" << component.errorString();
            delete obj;
            return;
        }
    }
    QQuickWindow *w = m_optionsWindow;
    w->setProperty("toolId", id);
    w->setProperty("toolTitle", title.isEmpty() ? id : title);
    w->setProperty("files", files);
    QMetaObject::invokeMethod(w, "reset");
    QMetaObject::invokeMethod(w, "applyInfo", Q_ARG(QVariant, QVariant(QVariantMap())));
    if (m_options.windows)
        presentWindow(w);
    // Facts for defaults (duration, size, pages) of the first file.
    QPointer<QQuickWindow> guard(w);
    CliCall::run(this, m_cliPath, {QStringLiteral("inspect"), QFileInfo(files.constFirst()).absoluteFilePath(),
                                   QStringLiteral("--json")},
                 [guard, id](bool ok, const QJsonObject &result, const QString &) {
                     if (!ok || !guard || guard->property("toolId").toString() != id)
                         return;
                     const QJsonArray list = result.value(QStringLiteral("files")).toArray();
                     if (!list.isEmpty())
                         QMetaObject::invokeMethod(guard, "applyInfo",
                                                   Q_ARG(QVariant, list.first().toObject().toVariantMap()));
                 });
}

QStringList App::pathsFromUrls(const QList<QUrl> &urls) const
{
    return Selection::localFiles(urls);
}

QString App::fileName(const QString &path) const
{
    return QFileInfo(path).fileName();
}

QString App::mimeIcon(const QString &path) const
{
    static const QMimeDatabase db;
    const QMimeType type = db.mimeTypeForFile(path, QMimeDatabase::MatchExtension);
    return type.isValid() ? type.iconName() : QStringLiteral("text-x-generic");
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

QObject *App::optionsWindow() const
{
    return m_optionsWindow.data();
}

QObject *App::downloadWindow() const
{
    return m_downloadWindow.data();
}

QObject *App::previewWindow() const
{
    return m_previewWindow.data();
}

QObject *App::shelfWindow() const
{
    return m_shelfWindow.data();
}

QObject *App::shareWindow() const
{
    return m_shareWindow.data();
}

QString App::defaultDownloadDir() const
{
    const QString dl = QStandardPaths::writableLocation(QStandardPaths::DownloadLocation);
    if (!dl.isEmpty())
        return dl;
    const QString fallback = QDir::homePath() + QStringLiteral("/Downloads");
    return QFileInfo::exists(fallback) ? fallback : QDir::homePath();
}

bool App::isValidWebUrl(const QString &url) const
{
    return ClipboardImport::isSingleWebLink(url);
}

QString App::clipboardWebUrl() const
{
    const QClipboard *cb = QGuiApplication::clipboard();
    return cb ? ClipboardImport::webLinkFromMime(cb->mimeData()) : QString();
}

QString App::clipboardText() const
{
    const QClipboard *cb = QGuiApplication::clipboard();
    if (!cb || !cb->mimeData())
        return {};
    const QString link = ClipboardImport::webLinkFromMime(cb->mimeData());
    if (!link.isEmpty())
        return link;
    return cb->mimeData()->hasText() ? cb->mimeData()->text().trimmed() : QString();
}

bool App::isToolMissing(const QString &toolId) const
{
    const int n = m_deps->rowCount();
    for (int i = 0; i < n; ++i) {
        const QModelIndex idx = m_deps->index(i, 0);
        if (m_deps->data(idx, DepsModel::IdRole).toString() == toolId)
            return !m_deps->data(idx, DepsModel::AvailableRole).toBool();
    }
    return false;
}

void App::showDownload(const QString &url, bool fromClipboard)
{
    if (!m_engine)
        return;
    if (!m_downloadWindow) {
        QQmlComponent component(m_engine);
        component.loadFromModule("NashiApp", "DownloadWindow");
        QObject *obj = component.create();
        m_downloadWindow = qobject_cast<QQuickWindow *>(obj);
        if (!m_downloadWindow) {
            qCWarning(lcApp) << "DownloadWindow failed to load:" << component.errorString();
            delete obj;
            return;
        }
    }
    if (m_deps->rowCount() == 0 && !m_deps->loading() && !m_cliPath.isEmpty())
        m_deps->refresh();
    QString initial = url.trimmed();
    if (!initial.isEmpty() && !isValidWebUrl(initial))
        initial.clear();
    QMetaObject::invokeMethod(m_downloadWindow, "openFor", Q_ARG(QVariant, QVariant(initial)),
                              Q_ARG(QVariant, QVariant(fromClipboard)));
    if (m_options.windows)
        presentWindow(m_downloadWindow);
}

void App::showDownloadFromClipboard()
{
    showDownload(QString(), true);
}

bool App::startDownload(const QString &url, const QString &kind, const QString &outDir)
{
    const QString trimmed = url.trimmed();
    if (!isValidWebUrl(trimmed)) {
        postMessage(tr("Enter a valid http:// or https:// URL"));
        return false;
    }
    const QUrl parsed(trimmed, QUrl::StrictMode);
    QString subject = QFileInfo(parsed.path()).fileName();
    if (subject.isEmpty())
        subject = parsed.host();
    const QString title = subject.isEmpty() ? tr("Download") : tr("Download · %1").arg(subject);
    const QString targetDir = outDir.trimmed().isEmpty() ? defaultDownloadDir() : outDir.trimmed();
    JobRunner *job = m_jobs->startDownload(trimmed, kind, targetDir, title);
    if (!job) {
        postMessage(tr("Cannot start download"));
        return false;
    }
    return true;
}

void App::reuseFiles(const QStringList &files)
{
    QStringList existing;
    for (const QString &f : files) {
        const QFileInfo fi(f);
        if (fi.exists()) {
            const QString abs = fi.absoluteFilePath();
            if (!existing.contains(abs))
                existing.append(abs);
        }
    }
    if (existing.isEmpty()) {
        postMessage(tr("Those files are no longer available"));
        return;
    }
    m_home->setFiles(existing);
    showHome();
    if (m_homeWindow) {
        if (QObject *tabs = m_homeWindow->findChild<QObject *>(QStringLiteral("mainTabs")))
            tabs->setProperty("currentIndex", 0);
    }
    Q_EMIT homeTabRequested();
}

void App::showPreview(const QString &path, const QStringList &siblings)
{
    if (!m_engine || path.trimmed().isEmpty())
        return;
    if (!m_previewWindow) {
        QQmlComponent component(m_engine);
        component.loadFromModule("NashiApp", "PreviewWindow");
        QObject *obj = component.create();
        m_previewWindow = qobject_cast<QQuickWindow *>(obj);
        if (!m_previewWindow) {
            qCWarning(lcApp) << "PreviewWindow failed to load:" << component.errorString();
            delete obj;
            return;
        }
    }
    const QString abs = QFileInfo(path).absoluteFilePath();
    QStringList list;
    for (const QString &s : siblings) {
        if (s.trimmed().isEmpty())
            continue;
        const QString item = QFileInfo(s).absoluteFilePath();
        if (!list.contains(item))
            list.append(item);
    }
    int idx = int(list.indexOf(abs));
    if (idx < 0) {
        list.prepend(abs);
        idx = 0;
    }
    QMetaObject::invokeMethod(m_previewWindow, "openPreview", Q_ARG(QVariant, QVariant(list)),
                              Q_ARG(QVariant, QVariant(idx)));
    if (m_options.windows)
        presentWindow(m_previewWindow);
}

namespace {
// Lowercasing İ gives "i" plus a combining dot, and ı stays apart from i, so Turkish
// names would not match their ASCII spelling or their other case.
QString foldForSearch(const QString &text)
{
    QString folded = text.toLower();
    folded.remove(QChar(0x0307));
    folded.replace(QChar(0x0131), QLatin1Char('i'));
    return folded;
}
} // namespace

bool App::matchesSearch(const QString &query, const QString &name, const QString &folder) const
{
    const QString q = foldForSearch(query.trimmed());
    return q.isEmpty() || foldForSearch(name).contains(q) || foldForSearch(folder).contains(q);
}

QStringList App::filteredRecentPaths(const QString &query) const
{
    QStringList out;
    const int n = m_recent->rowCount();
    for (int i = 0; i < n; ++i) {
        const QModelIndex idx = m_recent->index(i, 0);
        if (!m_recent->data(idx, RecentModel::ExistsRole).toBool())
            continue;
        const QString name = m_recent->data(idx, RecentModel::NameRole).toString();
        const QString folder = m_recent->data(idx, RecentModel::FolderRole).toString();
        if (!matchesSearch(query, name, folder))
            continue;
        out.append(m_recent->data(idx, RecentModel::PathRole).toString());
    }
    return out;
}

bool App::fileExists(const QString &path) const
{
    return !path.isEmpty() && QFileInfo::exists(path);
}

qint64 App::fileSize(const QString &path) const
{
    return path.isEmpty() ? 0 : QFileInfo(path).size();
}

QString App::formatSize(qint64 bytes) const
{
    if (bytes < 0)
        return {};
    return QLocale().formattedDataSize(bytes, 1, QLocale::DataSizeTraditionalFormat);
}

QUrl App::localFileUrl(const QString &path) const
{
    return path.isEmpty() ? QUrl() : QUrl::fromLocalFile(QFileInfo(path).absoluteFilePath());
}

void App::toggleShelf()
{
    m_shelf->toggle();
}

void App::addToShelf(const QStringList &files)
{
    const int before = m_shelf->rowCount();
    m_shelf->add(files);
    if (m_shelf->rowCount() > 0 || before > 0)
        m_shelf->show();
}

void App::shelfResized()
{
    if (m_shelfWindow && m_shelfWindow->isVisible())
        Surfaces::placeShelfFallback(m_shelfWindow);
}

void App::syncShelfWindow()
{
    if (!m_engine)
        return;
    if (m_shelf->visible()) {
        if (!m_shelfWindow) {
            QQmlComponent component(m_engine);
            component.loadFromModule("NashiApp", "ShelfWindow");
            QObject *obj = component.create();
            m_shelfWindow = qobject_cast<QQuickWindow *>(obj);
            if (!m_shelfWindow) {
                qCWarning(lcApp) << "ShelfWindow failed to load:" << component.errorString();
                delete obj;
                return;
            }
            Surfaces::configureShelf(m_shelfWindow);
        }
        if (m_options.windows) {
            Surfaces::placeShelfFallback(m_shelfWindow);
            m_shelfWindow->show();
        }
    } else if (m_shelfWindow) {
        m_shelfWindow->hide();
    }
}

void App::showShare(const QStringList &paths)
{
    if (!m_engine)
        return;
    QStringList existing;
    for (const QString &p : paths) {
        const QFileInfo fi(p);
        if (fi.exists()) {
            const QString abs = fi.absoluteFilePath();
            if (!existing.contains(abs))
                existing.append(abs);
        }
    }
    if (existing.isEmpty()) {
        postMessage(tr("No existing files to share"));
        return;
    }
    if (!shareAvailable()) {
        postMessage(tr("Sharing is unavailable (KDE Purpose is not installed)"));
        return;
    }
    if (!m_shareWindow) {
        QQmlComponent component(m_engine);
        component.loadFromModule("NashiApp", "ShareSheet");
        QObject *obj = component.create();
        m_shareWindow = qobject_cast<QQuickWindow *>(obj);
        if (!m_shareWindow) {
            qCWarning(lcApp) << "ShareSheet failed to load:" << component.errorString();
            delete obj;
            return;
        }
    }
    QMetaObject::invokeMethod(m_shareWindow, "share", Q_ARG(QVariant, QVariant(existing)));
    if (m_options.windows)
        presentWindow(m_shareWindow);
}

QStringList App::fileUrlsFor(const QStringList &paths) const
{
    QStringList urls;
    for (const QString &p : paths) {
        if (p.isEmpty())
            continue;
        urls.append(QUrl::fromLocalFile(QFileInfo(p).absoluteFilePath()).toString(QUrl::FullyEncoded));
    }
    return urls;
}

QString App::commonMimeType(const QStringList &paths) const
{
    if (paths.isEmpty())
        return QStringLiteral("*/*");
    static const QMimeDatabase db;
    QString exact;
    QString primary;
    for (const QString &p : paths) {
        const QMimeType t = db.mimeTypeForFile(p);
        const QString name = t.isValid() ? t.name() : QStringLiteral("application/octet-stream");
        const QString major = name.section(QLatin1Char('/'), 0, 0);
        if (exact.isEmpty()) {
            exact = name;
            primary = major;
        } else {
            if (exact != name)
                exact = QStringLiteral("*");
            if (primary != major)
                primary = QStringLiteral("*");
        }
    }
    if (exact != QLatin1String("*"))
        return exact;
    if (primary != QLatin1String("*") && !primary.isEmpty())
        return primary + QStringLiteral("/*");
    return QStringLiteral("*/*");
}

void App::reportShareResult(int error, const QString &errorMessage, const QString &outputUrl, int fileCount)
{
    if (error != 0) {
        postMessage(errorMessage.isEmpty() ? tr("Sharing failed") : tr("Sharing failed: %1").arg(errorMessage));
        return;
    }
    if (!outputUrl.trimmed().isEmpty()) {
        m_actions->copyText(outputUrl.trimmed());
        postMessage(tr("Shared — link copied: %1").arg(outputUrl.trimmed()));
    } else {
        postMessage(tr("Shared %n file(s)", nullptr, qMax(1, fileCount)));
    }
}
