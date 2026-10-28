#pragma once

#include <QObject>
#include <QPointer>
#include <QStringList>
#include <QUrl>
#include <QtQmlIntegration/qqmlintegration.h>

class DepsModel;
class FileActions;
class InputdClient;
class JobManager;
class JobRunner;
class Language;
class Notifier;
class OverlayController;
class OverlaySurfaces;
class QJSEngine;
class QQmlEngine;
class QQuickWindow;
class RecentModel;
class Selection;
class Settings;
class ToastController;
class Tray;
class WheelQuery;

// Root object: owns every service and is the `App` QML singleton.
class App : public QObject
{
    Q_OBJECT
    Q_MOC_INCLUDE("actions.h")
    Q_MOC_INCLUDE("depsmodel.h")
    Q_MOC_INCLUDE("inputdclient.h")
    Q_MOC_INCLUDE("jobmanager.h")
    Q_MOC_INCLUDE("language.h")
    Q_MOC_INCLUDE("overlaycontroller.h")
    Q_MOC_INCLUDE("recentmodel.h")
    Q_MOC_INCLUDE("selection.h")
    Q_MOC_INCLUDE("settings.h")
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
    Q_PROPERTY(QString version READ version CONSTANT)
    Q_PROPERTY(QString cliPath READ cliPath CONSTANT)
    Q_PROPERTY(bool layerShell READ layerShell CONSTANT)
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
    QString version() const;
    QString cliPath() const { return m_cliPath; }
    bool layerShell() const;
    QString lastMessage() const { return m_lastMessage; }

    Q_INVOKABLE void showHome();
    Q_INVOKABLE void quit();
    Q_INVOKABLE void openFiles(const QStringList &files);
    Q_INVOKABLE void showWheel(const QStringList &files, const QString &mode);
    Q_INVOKABLE void showWheelFromClipboard(const QString &mode);
    // kind: convert | tool | action.
    Q_INVOKABLE void runItem(const QString &kind, const QString &id, const QString &title, const QStringList &files);
    Q_INVOKABLE QStringList pathsFromUrls(const QList<QUrl> &urls) const;
    Q_INVOKABLE QString fileName(const QString &path) const;

Q_SIGNALS:
    void message(const QString &text);

private:
    void onJobFinished(JobRunner *job);
    void postMessage(const QString &text);

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
    Notifier *m_notifier;
    QQmlEngine *m_engine = nullptr;
    OverlaySurfaces *m_surfaces = nullptr;
    Tray *m_tray = nullptr;
    QPointer<QQuickWindow> m_homeWindow;
    QString m_lastMessage;
    bool m_quitting = false;
};
