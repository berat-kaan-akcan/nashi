#pragma once

#include <QObject>
#include <QPointer>
#include <QStringList>
#include <QUrl>
#include <QtQmlIntegration/qqmlintegration.h>

class ClipboardImport;
class DepsModel;
class FileActions;
class InputdClient;
class JobManager;
class JobRunner;
class Language;
class Notifier;
class OverlayController;
class OverlaySurfaces;
class PreviewClient;
class QJSEngine;
class QQmlEngine;
class QQuickWindow;
class RecentModel;
class Selection;
class Settings;
class SetupCheck;
class ShelfModel;
class ToastController;
class Tray;
class WheelQuery;

// Root object: owns every service and is the `App` QML singleton.
class App : public QObject
{
    Q_OBJECT
    Q_MOC_INCLUDE("actions.h")
    Q_MOC_INCLUDE("clipboardimport.h")
    Q_MOC_INCLUDE("depsmodel.h")
    Q_MOC_INCLUDE("inputdclient.h")
    Q_MOC_INCLUDE("jobmanager.h")
    Q_MOC_INCLUDE("language.h")
    Q_MOC_INCLUDE("overlaycontroller.h")
    Q_MOC_INCLUDE("previewclient.h")
    Q_MOC_INCLUDE("recentmodel.h")
    Q_MOC_INCLUDE("selection.h")
    Q_MOC_INCLUDE("settings.h")
    Q_MOC_INCLUDE("setupcheck.h")
    Q_MOC_INCLUDE("shelfmodel.h")
    Q_MOC_INCLUDE("toastcontroller.h")
    QML_ELEMENT
    QML_SINGLETON

    Q_PROPERTY(Settings *settings READ settings CONSTANT)
    Q_PROPERTY(Language *language READ language CONSTANT)
    Q_PROPERTY(JobManager *jobs READ jobs CONSTANT)
    Q_PROPERTY(RecentModel *recent READ recent CONSTANT)
    Q_PROPERTY(DepsModel *deps READ deps CONSTANT)
    Q_PROPERTY(InputdClient *inputd READ inputd CONSTANT)
    Q_PROPERTY(OverlayController *overlay READ overlay CONSTANT)
    Q_PROPERTY(FileActions *actions READ actions CONSTANT)
    Q_PROPERTY(ToastController *toast READ toast CONSTANT)
    Q_PROPERTY(Selection *home READ home CONSTANT)
    Q_PROPERTY(SetupCheck *setup READ setup CONSTANT)
    Q_PROPERTY(ClipboardImport *clipboard READ clipboard CONSTANT)
    Q_PROPERTY(PreviewClient *preview READ preview CONSTANT)
    Q_PROPERTY(ShelfModel *shelf READ shelf CONSTANT)
    Q_PROPERTY(QString version READ version CONSTANT)
    Q_PROPERTY(QString cliPath READ cliPath CONSTANT)
    Q_PROPERTY(bool layerShell READ layerShell CONSTANT)
    Q_PROPERTY(bool shareAvailable READ shareAvailable NOTIFY engineChanged)
    Q_PROPERTY(bool multimediaAvailable READ multimediaAvailable NOTIFY engineChanged)
    Q_PROPERTY(QString lastMessage READ lastMessage NOTIFY message)

public:
    struct Options
    {
        bool tray = true;
        bool windows = true; // overlay + toast + home windows
    };

    explicit App(const Options &options, QObject *parent = nullptr);
    ~App() override;

    static App *create(QQmlEngine *, QJSEngine *);
    static App *instance() { return s_instance; }

    // Creates engine-dependent parts (windows, tray). Call once.
    void init(QQmlEngine *engine);
    bool registerDBusObject();

    Settings *settings() const { return m_settings; }
    Language *language() const { return m_language; }
    JobManager *jobs() const { return m_jobs; }
    RecentModel *recent() const { return m_recent; }
    DepsModel *deps() const { return m_deps; }
    InputdClient *inputd() const { return m_inputd; }
    OverlayController *overlay() const { return m_overlay; }
    FileActions *actions() const { return m_actions; }
    ToastController *toast() const { return m_toast; }
    Selection *home() const { return m_home; }
    SetupCheck *setup() const { return m_setup; }
    ClipboardImport *clipboard() const { return m_clipboard; }
    PreviewClient *preview() const { return m_preview; }
    ShelfModel *shelf() const { return m_shelf; }
    QString version() const;
    QString cliPath() const { return m_cliPath; }
    bool layerShell() const;
    bool shareAvailable() const;
    bool multimediaAvailable() const;
    QString lastMessage() const { return m_lastMessage; }

    Q_INVOKABLE void showHome();
    // First-run wizard (WelcomeWindow.qml); shown by init() until finished.
    Q_INVOKABLE void showWelcome();
    // Marks the wizard done, hides it and shows Home.
    Q_INVOKABLE void finishWelcome();
    Q_INVOKABLE void quit();
    Q_INVOKABLE void openFiles(const QStringList &files);
    Q_INVOKABLE void showWheel(const QStringList &files, const QString &mode);
    Q_INVOKABLE void showWheelFromClipboard(const QString &mode);
    // kind: convert | tool | action.
    // Runs a wheel item; editor tools open the options dialog first.
    Q_INVOKABLE void runItem(const QString &kind, const QString &id, const QString &title, const QStringList &files);
    // Runs a tool with `--options` JSON (from OptionsWindow.qml).
    Q_INVOKABLE void runWithOptions(const QString &id, const QString &title, const QStringList &files,
                                    const QString &options);
    // Opens OptionsWindow.qml for an editor tool and fills in `nashi-cli inspect` facts.
    Q_INVOKABLE void showOptions(const QString &id, const QString &title, const QStringList &files);
    QObject *optionsWindow() const;
    QObject *downloadWindow() const;
    QObject *previewWindow() const;
    QObject *shelfWindow() const;
    QObject *shareWindow() const;

    // Web download (DownloadWindow.qml).
    Q_INVOKABLE QString defaultDownloadDir() const;
    Q_INVOKABLE bool isValidWebUrl(const QString &url) const;
    Q_INVOKABLE QString clipboardWebUrl() const;
    Q_INVOKABLE QString clipboardText() const;
    Q_INVOKABLE bool isToolMissing(const QString &toolId) const;
    Q_INVOKABLE void showDownload(const QString &url = QString(), bool fromClipboard = false);
    Q_INVOKABLE void showDownloadFromClipboard();
    Q_INVOKABLE bool startDownload(const QString &url, const QString &kind, const QString &outDir);

    // Library preview & reuse (PreviewWindow.qml, RecentPage.qml).
    Q_INVOKABLE void reuseFiles(const QStringList &files);
    Q_INVOKABLE void showPreview(const QString &path, const QStringList &siblings = QStringList());
    Q_INVOKABLE QStringList filteredRecentPaths(const QString &query) const;
    Q_INVOKABLE bool fileExists(const QString &path) const;
    Q_INVOKABLE qint64 fileSize(const QString &path) const;
    Q_INVOKABLE QString formatSize(qint64 bytes) const;
    Q_INVOKABLE QUrl localFileUrl(const QString &path) const;

    // Float shelf (ShelfWindow.qml).
    // Whether a Library entry matches a search; İ, I, ı and i all count as one letter.
    Q_INVOKABLE bool matchesSearch(const QString &query, const QString &name, const QString &folder) const;
    // The --options JSON a plain conversion to `target` sends; empty when it needs none.
    QString convertOptions(const QString &target) const;

    Q_INVOKABLE void toggleShelf();
    Q_INVOKABLE void addToShelf(const QStringList &files);
    Q_INVOKABLE void shelfResized();

    // Share (ShareSheet.qml via KDE Purpose).
    Q_INVOKABLE void showShare(const QStringList &paths);
    Q_INVOKABLE QStringList fileUrlsFor(const QStringList &paths) const;
    Q_INVOKABLE QString commonMimeType(const QStringList &paths) const;
    Q_INVOKABLE void reportShareResult(int error, const QString &errorMessage, const QString &outputUrl, int fileCount);

    Q_INVOKABLE QStringList pathsFromUrls(const QList<QUrl> &urls) const;
    Q_INVOKABLE QString fileName(const QString &path) const;
    // Freedesktop icon name for the file's type (by extension).
    Q_INVOKABLE QString mimeIcon(const QString &path) const;

Q_SIGNALS:
    void message(const QString &text);
    void homeTabRequested();
    void engineChanged();

private:
    void onJobFinished(JobRunner *job);
    void postMessage(const QString &text);
    void syncShelfWindow();

    static App *s_instance;
    Options m_options;
    QString m_cliPath;
    Settings *m_settings;
    Language *m_language;
    WheelQuery *m_wheelQuery;
    JobManager *m_jobs;
    RecentModel *m_recent;
    DepsModel *m_deps;
    InputdClient *m_inputd;
    OverlayController *m_overlay;
    FileActions *m_actions;
    ToastController *m_toast;
    Selection *m_home;
    SetupCheck *m_setup;
    Notifier *m_notifier;
    ClipboardImport *m_clipboard;
    PreviewClient *m_preview;
    ShelfModel *m_shelf;
    QQmlEngine *m_engine = nullptr;
    OverlaySurfaces *m_surfaces = nullptr;
    Tray *m_tray = nullptr;
    QPointer<QQuickWindow> m_homeWindow;
    QPointer<QQuickWindow> m_optionsWindow;
    QPointer<QQuickWindow> m_welcomeWindow;
    QPointer<QQuickWindow> m_downloadWindow;
    QPointer<QQuickWindow> m_previewWindow;
    QPointer<QQuickWindow> m_shelfWindow;
    QPointer<QQuickWindow> m_shareWindow;
    void startJob(const QString &kind, const QString &id, const QString &title, const QStringList &files,
                  const QString &options);
    QString m_lastMessage;
    bool m_quitting = false;
};
