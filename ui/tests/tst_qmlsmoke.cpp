#include "app.h"
#include "jobmanager.h"
#include "jobrunner.h"
#include "overlaycontroller.h"
#include "recentmodel.h"
#include "selection.h"
#include "settings.h"
#include "depsmodel.h"
#include "toastcontroller.h"
#include "wheelmodel.h"

#include <QDirIterator>
#include <QGuiApplication>
#include <QPointer>
#include <QMimeData>
#include <QSignalSpy>
#include <QTemporaryDir>
#include "wheelgeometry.h"
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickItem>
#include <QQuickWindow>
#include <QTest>
#include <QtQml/qqmlextensionplugin.h>

Q_IMPORT_QML_PLUGIN(NashiAppPlugin)

namespace {
QStringList g_messages;
QtMessageHandler g_previous = nullptr;

// Development aid: with NASHI_SCREENSHOT_DIR set, windows are saved as PNGs.
void shot(QQuickWindow *w, const QString &name)
{
    const QString dir = qEnvironmentVariable("NASHI_SCREENSHOT_DIR");
    if (dir.isEmpty())
        return;
    QTest::qWait(150);
    w->grabWindow().save(dir + QLatin1Char('/') + name + QStringLiteral(".png"));
}

// Repeater delegates have no QObject parent, so walk the visual tree.
QQuickItem *findItem(QQuickItem *root, const QString &name)
{
    if (!root)
        return nullptr;
    if (root->objectName() == name)
        return root;
    for (QQuickItem *child : root->childItems())
        if (QQuickItem *hit = findItem(child, name))
            return hit;
    return nullptr;
}

void captureHandler(QtMsgType type, const QMessageLogContext &ctx, const QString &msg)
{
    if (type != QtDebugMsg && type != QtInfoMsg)
        g_messages.append(msg);
    if (g_previous)
        g_previous(type, ctx, msg);
}
} // namespace

// Loads every QML file of the module offscreen and fails on any warning.
class TestQmlSmoke : public QObject
{
    Q_OBJECT

    App *app = nullptr;
    QQmlEngine *engine = nullptr;

private Q_SLOTS:
    void initTestCase()
    {
        // Outside ctest this would use the real settings, library and nashi-cli.
        if (qEnvironmentVariableIsEmpty("NASHI_CLI") || qEnvironmentVariableIsEmpty("XDG_CONFIG_HOME"))
            QFAIL("Run this test through ctest");
        g_previous = qInstallMessageHandler(captureHandler);
        app = new App(App::Options{false, false}, this);
        engine = new QQmlEngine(this);
        app->init(engine);
        connect(engine, &QQmlEngine::warnings, this, [](const QList<QQmlError> &errors) {
            for (const QQmlError &e : errors)
                g_messages.append(e.toString());
        });
    }

    void cleanupTestCase()
    {
        delete engine;
        delete app;
        qInstallMessageHandler(g_previous);
    }

    void loadAll_data()
    {
        QTest::addColumn<QString>("url");
        QDirIterator it(QStringLiteral(":/qt/qml/NashiApp/qml"), {QStringLiteral("*.qml")}, QDir::Files,
                        QDirIterator::Subdirectories);
        QStringList inModule;
        while (it.hasNext()) {
            const QString path = it.next();
            inModule << QFileInfo(path).fileName();
            QTest::newRow(qPrintable(inModule.constLast())) << (QStringLiteral("qrc") + path);
        }
        // Every QML file in the source tree must be registered in the module.
        QStringList inSource = QDir(QStringLiteral(NASHI_QML_SRC_DIR)).entryList({QStringLiteral("*.qml")}, QDir::Files);
        inModule.sort();
        inSource.sort();
        QCOMPARE(inModule, inSource);
    }

    void loadAll()
    {
        QFETCH(QString, url);
        g_messages.clear();
        QQmlComponent component(engine, QUrl(url));
        QVERIFY2(component.isReady(), qPrintable(component.errorString()));
        std::unique_ptr<QObject> obj(component.create());
        QVERIFY2(obj, qPrintable(component.errorString()));
        if (auto *w = qobject_cast<QQuickWindow *>(obj.get())) {
            w->show();
            QVERIFY(QTest::qWaitForWindowExposed(w));
            QTest::qWait(50);
            w->hide();
        } else {
            QTest::qWait(20);
        }
        obj.reset();
        QCoreApplication::processEvents();
        QVERIFY2(g_messages.isEmpty(), qPrintable(g_messages.join(QLatin1Char('\n'))));
    }

    // Exercise the overlay through its states with the real QML.
    void overlayStages()
    {
        g_messages.clear();
        QQmlComponent component(engine);
        component.loadFromModule("NashiApp", "OverlayWindow");
        std::unique_ptr<QObject> obj(component.create());
        auto *w = qobject_cast<QQuickWindow *>(obj.get());
        QVERIFY2(w, qPrintable(component.errorString()));
        w->resize(1280, 800);
        w->show();
        QVERIFY(QTest::qWaitForWindowExposed(w));

        OverlayController *ctl = app->overlay();
        ctl->onChordStarted(QStringLiteral("convert"));
        ctl->dragEntered(w, {QUrl::fromLocalFile(QStringLiteral("/tmp/clip.mp4"))}, QPointF(20, 20));
        QTRY_COMPARE_WITH_TIMEOUT(ctl->stage(), OverlayController::Live, 10000);
        QTest::qWait(50);
        shot(w, QStringLiteral("wheel-convert"));
        ctl->onChordStarted(QStringLiteral("tools"));
        QTest::qWait(50);
        shot(w, QStringLiteral("wheel-tools"));
        ctl->dropped(w, {}, QPointF(20, 20), -3);
        QCOMPARE(ctl->stage(), OverlayController::Hidden); // -3 in Live = cancel
        ctl->showForFiles({QStringLiteral("/tmp/clip.mp4")}, QStringLiteral("actions"), w, QPointF(640, 400));
        QTRY_VERIFY_WITH_TIMEOUT(ctl->stage() == OverlayController::Pick, 10000);
        QTest::qWait(300);
        QTest::keyClick(w, Qt::Key_Escape);
        QTRY_COMPARE_WITH_TIMEOUT(ctl->stage(), OverlayController::Hidden, 2000);
        QVERIFY2(g_messages.isEmpty(), qPrintable(g_messages.join(QLatin1Char('\n'))));
    }

    // Synthetic Wayland-like DnD into the overlay: a Shift-drag proposes Move,
    // the overlay must answer Copy at enter, move and drop.
    void overlayDropIsAlwaysCopy()
    {
        g_messages.clear();
        QQmlComponent component(engine);
        component.loadFromModule("NashiApp", "OverlayWindow");
        std::unique_ptr<QObject> obj(component.create());
        auto *w = qobject_cast<QQuickWindow *>(obj.get());
        QVERIFY2(w, qPrintable(component.errorString()));
        w->resize(1280, 800);
        w->show();
        QVERIFY(QTest::qWaitForWindowExposed(w));

        OverlayController *ctl = app->overlay();
        QSignalSpy chosen(ctl, &OverlayController::itemChosen);
        ctl->onChordStarted(QStringLiteral("convert"));

        QMimeData mime;
        mime.setUrls({QUrl::fromLocalFile(QStringLiteral("/tmp/clip.mp4"))});
        const Qt::DropActions offered = Qt::CopyAction | Qt::MoveAction | Qt::LinkAction;
        const QPointF anchor(640, 400);

        // Like QGuiApplicationPrivate::processDrag: DragEnter, then DragMove whose
        // accepted action is what the platform reports back to the source.
        QDragEnterEvent enter(anchor.toPoint(), offered, &mime, Qt::LeftButton, Qt::ShiftModifier);
        enter.setDropAction(Qt::MoveAction);
        QCoreApplication::sendEvent(w, &enter);
        QDragMoveEvent firstMove(anchor.toPoint(), offered, &mime, Qt::LeftButton, Qt::ShiftModifier);
        firstMove.setDropAction(Qt::MoveAction);
        QCoreApplication::sendEvent(w, &firstMove);
        QVERIFY(firstMove.isAccepted());
        QCOMPARE(firstMove.dropAction(), Qt::CopyAction);
        QCOMPARE(ctl->hostWindow(), w);
        QTRY_COMPARE_WITH_TIMEOUT(ctl->stage(), OverlayController::Live, 10000);

        // Slice 0 (MP3) is straight above the centre.
        const QPointF onSlice0 = anchor + QPointF(0, -(WheelGeometry::kInnerRadius + 40));
        QDragMoveEvent move(onSlice0.toPoint(), offered, &mime, Qt::LeftButton, Qt::ShiftModifier);
        move.setDropAction(Qt::MoveAction);
        QCoreApplication::sendEvent(w, &move);
        QVERIFY(move.isAccepted());
        QCOMPARE(move.dropAction(), Qt::CopyAction);

        QDropEvent drop(onSlice0, offered, &mime, Qt::LeftButton, Qt::ShiftModifier);
        drop.setDropAction(Qt::MoveAction);
        QCoreApplication::sendEvent(w, &drop);
        QVERIFY(drop.isAccepted());
        QCOMPARE(drop.dropAction(), Qt::CopyAction);

        QCOMPARE(chosen.count(), 1);
        QCOMPARE(chosen.at(0).at(1).toString(), QStringLiteral("mp3"));
        QCOMPARE(ctl->stage(), OverlayController::Hidden);
        QTRY_VERIFY_WITH_TIMEOUT(app->jobs()->activeCount() == 0, 10000);
        QVERIFY2(g_messages.isEmpty(), qPrintable(g_messages.join(QLatin1Char('\n'))));
    }

    // Two-stage: URLs are only readable at drop time.
    void overlayTwoStageDrop()
    {
        g_messages.clear();
        QQmlComponent component(engine);
        component.loadFromModule("NashiApp", "OverlayWindow");
        std::unique_ptr<QObject> obj(component.create());
        auto *w = qobject_cast<QQuickWindow *>(obj.get());
        QVERIFY(w);
        w->resize(1280, 800);
        w->show();
        QVERIFY(QTest::qWaitForWindowExposed(w));
        OverlayController *ctl = app->overlay();
        ctl->onChordStarted(QStringLiteral("tools"));

        QMimeData empty; // nothing readable before the drop
        QDragEnterEvent enter(QPoint(300, 300), Qt::CopyAction | Qt::MoveAction, &empty, Qt::LeftButton, Qt::ShiftModifier);
        enter.setDropAction(Qt::MoveAction);
        QCoreApplication::sendEvent(w, &enter);
        QDragMoveEvent move(QPoint(300, 300), Qt::CopyAction | Qt::MoveAction, &empty, Qt::LeftButton, Qt::ShiftModifier);
        move.setDropAction(Qt::MoveAction);
        QCoreApplication::sendEvent(w, &move);
        QCOMPARE(move.dropAction(), Qt::CopyAction);
        QCOMPARE(ctl->stage(), OverlayController::Target);

        QMimeData full;
        full.setUrls({QUrl::fromLocalFile(QStringLiteral("/tmp/p.png"))});
        QDropEvent drop(QPointF(300, 300), Qt::CopyAction | Qt::MoveAction, &full, Qt::LeftButton, Qt::ShiftModifier);
        drop.setDropAction(Qt::MoveAction);
        QCoreApplication::sendEvent(w, &drop);
        QCOMPARE(drop.dropAction(), Qt::CopyAction);
        QCOMPARE(ctl->stage(), OverlayController::Pick);
        QTRY_VERIFY_WITH_TIMEOUT(ctl->selection()->ready(), 10000);
        QTest::qWait(50);
        // Click the centre → cancel.
        QTest::mouseClick(w, Qt::LeftButton, {}, QPoint(300, 300));
        QTRY_COMPARE_WITH_TIMEOUT(ctl->stage(), OverlayController::Hidden, 2000);
        QVERIFY2(g_messages.isEmpty(), qPrintable(g_messages.join(QLatin1Char('\n'))));
    }

    // Pick stage: arrows walk the available slices, Return runs the highlighted one.
    void overlayPickStageWorksFromTheKeyboard()
    {
        g_messages.clear();
        QQmlComponent component(engine);
        component.loadFromModule("NashiApp", "OverlayWindow");
        std::unique_ptr<QObject> obj(component.create());
        auto *w = qobject_cast<QQuickWindow *>(obj.get());
        QVERIFY(w);
        w->resize(1280, 800);
        w->show();
        QVERIFY(QTest::qWaitForWindowExposed(w));
        OverlayController *ctl = app->overlay();
        QSignalSpy chosen(ctl, &OverlayController::itemChosen);
        ctl->onChordStarted(QStringLiteral("convert"));

        QMimeData empty;
        QDragEnterEvent enter(QPoint(300, 300), Qt::CopyAction, &empty, Qt::LeftButton, Qt::NoModifier);
        QCoreApplication::sendEvent(w, &enter);
        QDragMoveEvent move(QPoint(300, 300), Qt::CopyAction, &empty, Qt::LeftButton, Qt::NoModifier);
        QCoreApplication::sendEvent(w, &move);
        QMimeData full;
        full.setUrls({QUrl::fromLocalFile(QStringLiteral("/tmp/p.png"))});
        QDropEvent drop(QPointF(300, 300), Qt::CopyAction, &full, Qt::LeftButton, Qt::NoModifier);
        QCoreApplication::sendEvent(w, &drop);
        QCOMPARE(ctl->stage(), OverlayController::Pick);
        QTRY_VERIFY_WITH_TIMEOUT(ctl->selection()->ready(), 10000);

        // The available slices, in wheel order, are what the keys must walk.
        QStringList available;
        auto *model = ctl->selection()->model();
        for (int i = 0; i < model->rowCount(); ++i) {
            const QVariantMap item = model->get(i);
            if (item.value(QStringLiteral("available")).toBool())
                available.append(item.value(QStringLiteral("itemId")).toString());
        }
        QVERIFY(available.size() >= 2);

        QTest::keyClick(w, Qt::Key_Return); // nothing highlighted yet
        QCOMPARE(chosen.count(), 0);
        QCOMPARE(ctl->stage(), OverlayController::Pick);

        QTest::keyClick(w, Qt::Key_Right); // first
        QTest::keyClick(w, Qt::Key_Down); // second
        QTest::keyClick(w, Qt::Key_Left); // first again
        QTest::keyClick(w, Qt::Key_Up); // wraps to the last
        QTest::keyClick(w, Qt::Key_Return);
        QCOMPARE(chosen.count(), 1);
        QCOMPARE(chosen.at(0).at(1).toString(), available.constLast());
        QCOMPARE(ctl->stage(), OverlayController::Hidden);
        QTRY_VERIFY_WITH_TIMEOUT(app->jobs()->activeCount() == 0, 10000);
        QVERIFY2(g_messages.isEmpty(), qPrintable(g_messages.join(QLatin1Char('\n'))));
    }

    // Toast + JobCard with real jobs from the fake CLI.
    void toastWithJobs()
    {
        g_messages.clear();
        JobRunner *ok = app->jobs()->start(QStringLiteral("convert"), QStringLiteral("mp3"), QStringLiteral("MP3"),
                                           {QStringLiteral("/tmp/a.mp4")});
        JobRunner *fail = app->jobs()->start(QStringLiteral("tool"), QStringLiteral("fail"), QStringLiteral("Fail"),
                                             {QStringLiteral("/tmp/b.mp4")});
        JobRunner *slow = app->jobs()->start(QStringLiteral("tool"), QStringLiteral("slow"), QStringLiteral("Slow"),
                                             {QStringLiteral("/tmp/c.mp4")});
        JobRunner *qr = app->jobs()->start(QStringLiteral("tool"), QStringLiteral("qr"), QStringLiteral("QR"),
                                           {QStringLiteral("/tmp/code.png")});
        QVERIFY(ok && fail && slow && qr);
        QVERIFY(app->toast()->shown());
        if (auto *tw = app->toast()->window())
            shot(tw, QStringLiteral("toast-running"));
        QTRY_VERIFY_WITH_TIMEOUT(ok->finished() && fail->finished() && qr->finished(), 10000);
        QCOMPARE(qr->outputMessages().size(), 1);
        QTest::qWait(100);
        slow->cancel();
        QTRY_VERIFY_WITH_TIMEOUT(slow->finished(), 10000);
        QCOMPARE(slow->state(), JobRunner::Cancelled);
        QTest::qWait(100);
        app->toast()->dismiss();
        QVERIFY(!app->toast()->shown());
        QCOMPARE(app->recent()->rowCount() >= 1, true);
        QVERIFY2(g_messages.isEmpty(), qPrintable(g_messages.join(QLatin1Char('\n'))));
    }

    // A protected archive: the card offers a password box, which reruns the
    // job with the password and forgets the typed text.
    void archivePassword()
    {
        g_messages.clear();
        JobRunner *locked = app->jobs()->start(QStringLiteral("tool"), QStringLiteral("locked"),
                                               QStringLiteral("Extract"), {QStringLiteral("/tmp/s.zip")});
        QVERIFY(locked);
        QTRY_VERIFY_WITH_TIMEOUT(locked->finished(), 10000);
        QVERIFY(locked->needsPassword());
        QQuickWindow *toast = app->toast()->window();
        QVERIFY(toast);
        QQuickItem *button = nullptr;
        QTRY_VERIFY((button = findItem(toast->contentItem(), QStringLiteral("jobPasswordButton"))));
        QTRY_VERIFY(button->isVisible());

        app->askPassword(locked);
        QQuickWindow *box = nullptr;
        for (QWindow *w : QGuiApplication::topLevelWindows())
            if (w->objectName() == QLatin1String("passwordWindow"))
                box = qobject_cast<QQuickWindow *>(w);
        QVERIFY(box);
        box->show();
        QVERIFY(QTest::qWaitForWindowExposed(box));
        shot(box, QStringLiteral("file-password"));
        QQuickItem *field = findItem(box->contentItem(), QStringLiteral("filePasswordField"));
        QVERIFY(field);
        QCOMPARE(field->property("echoMode").toInt(), 2); // TextInput.Password
        field->setProperty("text", QStringLiteral("open sesame"));
        QPointer<JobRunner> old = locked;
        QVERIFY(QMetaObject::invokeMethod(box, "submit"));
        QCOMPARE(field->property("text").toString(), QString());
        QVERIFY(!box->isVisible());
        QTRY_VERIFY(old.isNull());
        JobRunner *retry = app->jobs()->jobs().constFirst();
        QTRY_VERIFY_WITH_TIMEOUT(retry->finished(), 10000);
        QCOMPARE(retry->state(), JobRunner::Succeeded);
        app->toast()->dismiss();
        QVERIFY2(g_messages.isEmpty(), qPrintable(g_messages.join(QLatin1Char('\n'))));
    }

    // Options dialog: every form loads, validates and starts a job.
    void optionsWindow()
    {
        g_messages.clear();
        const QStringList tools = {QStringLiteral("changeVideoSpeed"), QStringLiteral("trimVideo"),
                                   QStringLiteral("trimAudio"),        QStringLiteral("cropVideo"),
                                   QStringLiteral("cropImage"),        QStringLiteral("editImage"),
                                   QStringLiteral("frameImage"),       QStringLiteral("redactImage"),
                                   QStringLiteral("redactVideo"),      QStringLiteral("redactAudio"),
                                   QStringLiteral("organizePDF")};
        app->showOptions(tools.constFirst(), tools.constFirst(), {QStringLiteral("/tmp/clip.mp4")});
        auto *w = qobject_cast<QQuickWindow *>(app->optionsWindow());
        QVERIFY(w);
        w->resize(520, 680);
        w->show();
        QVERIFY(QTest::qWaitForWindowExposed(w));
        for (const QString &id : tools) {
            app->showOptions(id, id, {QStringLiteral("/tmp/clip.mp4")});
            // Simulated `inspect` facts.
            const QVariantMap facts{{QStringLiteral("duration"), 12.5},
                                    {QStringLiteral("width"), 640},
                                    {QStringLiteral("height"), 360},
                                    {QStringLiteral("pages"), 4}};
            QMetaObject::invokeMethod(w, "applyInfo", Q_ARG(QVariant, QVariant(facts)));
            QTest::qWait(20);
            const QVariant result = w->property("result");
            const bool needsChoice = id == QLatin1String("editImage");
            QVERIFY2(needsChoice == (result.typeId() == QMetaType::QString),
                     qPrintable(id + QStringLiteral(": ") + result.toString()));
            shot(w, QStringLiteral("options-") + id);
        }
        const int before = app->jobs()->rowCount();
        app->showOptions(QStringLiteral("changeVideoSpeed"), QStringLiteral("Speed"), {QStringLiteral("/tmp/clip.mp4")});
        QMetaObject::invokeMethod(w, "run");
        QCOMPARE(app->jobs()->rowCount(), before + 1);
        QVERIFY(!w->isVisible());
        QVERIFY2(g_messages.isEmpty(), qPrintable(g_messages.join(QLatin1Char('\n'))));
    }

    // Home window with files queued, cycling through every tab.
    void homeWithFiles()
    {
        g_messages.clear();
        // Two Library files that exist, so Home's "Freshly made" lists rows.
        QTemporaryDir made;
        QVERIFY(made.isValid());
        const QStringList madePaths{made.filePath(QStringLiteral("report.pdf")),
                                    made.filePath(QStringLiteral("holiday.png"))};
        for (const QString &path : madePaths) {
            QFile f(path);
            QVERIFY(f.open(QIODevice::WriteOnly));
            f.write("x");
        }
        app->recent()->add(madePaths);
        app->home()->setFiles({QStringLiteral("/tmp/clip.mp4")});
        QQmlComponent component(engine);
        component.loadFromModule("NashiApp", "HomeWindow");
        std::unique_ptr<QObject> obj(component.create());
        auto *w = qobject_cast<QQuickWindow *>(obj.get());
        QVERIFY2(w, qPrintable(component.errorString()));
        w->show();
        QVERIFY(QTest::qWaitForWindowExposed(w));
        QTRY_VERIFY_WITH_TIMEOUT(app->home()->ready(), 10000);
        app->deps()->refresh();
        QTRY_VERIFY_WITH_TIMEOUT(!app->deps()->loading(), 10000);
        QObject *tabs = w->findChild<QObject *>(QStringLiteral("mainTabs"));
        QVERIFY(tabs);
        QTRY_VERIFY(findItem(w->contentItem(), QStringLiteral("homeRecentPreview_0")));
        QTRY_VERIFY(findItem(w->contentItem(), QStringLiteral("homeRecentPreview_1")));
        QObject *seeAll = w->findChild<QObject *>(QStringLiteral("homeSeeAllButton"));
        QVERIFY(seeAll);
        QMetaObject::invokeMethod(seeAll, "clicked");
        QCOMPARE(tabs->property("currentIndex").toInt(), 1);
        for (int i : {1, 2, 3, 0}) {
            tabs->setProperty("currentIndex", i);
            QTest::qWait(50);
        }
        if (!qEnvironmentVariableIsEmpty("NASHI_SCREENSHOT_DIR")) {
            const QString before = app->settings()->appearance();
            w->resize(960, 820);
            for (const QString &look : {QStringLiteral("light"), QStringLiteral("dark")}) {
                app->settings()->setAppearance(look);
                for (int i : {0, 1, 2, 3}) {
                    tabs->setProperty("currentIndex", i);
                    shot(w, QStringLiteral("home-%1-%2").arg(look).arg(i));
                }
            }
            app->settings()->setAppearance(before);
            tabs->setProperty("currentIndex", 0);
        }
        // Home extras after upstream Windows: Tools & shortcuts and the sheet.
        QObject *expander = w->findChild<QObject *>(QStringLiteral("toolsExpander"));
        QVERIFY(expander);
        expander->setProperty("expanded", true);
        QObject *sheet = w->findChild<QObject *>(QStringLiteral("shortcutsDialog"));
        QVERIFY(sheet);
        QMetaObject::invokeMethod(sheet, "open");
        QTRY_VERIFY(sheet->property("visible").toBool());
        shot(w, QStringLiteral("home-shortcuts"));
        QMetaObject::invokeMethod(sheet, "close");
        // Empty Home, the first thing a new user sees.
        app->home()->clear();
        if (!qEnvironmentVariableIsEmpty("NASHI_SCREENSHOT_DIR")) {
            const QString before = app->settings()->appearance();
            for (const QString &look : {QStringLiteral("light"), QStringLiteral("dark")}) {
                app->settings()->setAppearance(look);
                shot(w, QStringLiteral("home-%1-empty").arg(look));
            }
            app->settings()->setAppearance(before);
            // Longest header labels (Turkish) at the narrowest window.
            app->settings()->setLanguage(QStringLiteral("tr"));
            w->resize(700, 820);
            shot(w, QStringLiteral("home-tr-narrow"));
            app->settings()->setLanguage(QStringLiteral("en"));
            w->resize(960, 820);
        }
        expander->setProperty("expanded", false);
        app->home()->setFiles({QStringLiteral("/tmp/clip.mp4")});
        QTRY_VERIFY_WITH_TIMEOUT(app->home()->ready(), 10000);
        app->home()->model()->setMode(QStringLiteral("tools"));
        QTest::qWait(30);
        app->home()->model()->setMode(QStringLiteral("actions"));
        QTest::qWait(30);
        app->settings()->setLanguage(QStringLiteral("tr"));
        QTest::qWait(50);
        app->settings()->setLanguage(QStringLiteral("en"));
        QTest::qWait(30);
        w->hide();
        QVERIFY2(g_messages.isEmpty(), qPrintable(g_messages.join(QLatin1Char('\n'))));
    }
};

QTEST_MAIN(TestQmlSmoke)
#include "tst_qmlsmoke.moc"
