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
#include <QMimeData>
#include <QSignalSpy>
#include "wheelgeometry.h"
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickWindow>
#include <QTest>
#include <QtQml/qqmlextensionplugin.h>

Q_IMPORT_QML_PLUGIN(NashiAppPlugin)

namespace {
QStringList g_messages;
QtMessageHandler g_previous = nullptr;

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
        int n = 0;
        while (it.hasNext()) {
            const QString path = it.next();
            QTest::newRow(qPrintable(QFileInfo(path).fileName())) << (QStringLiteral("qrc") + path);
            ++n;
        }
        QVERIFY2(n >= 11, "QML files missing from the module resources");
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
        ctl->onChordStarted(QStringLiteral("tools"));
        QTest::qWait(50);
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
        QDragEnterEvent enter(anchor, offered, &mime, Qt::LeftButton, Qt::ShiftModifier);
        enter.setDropAction(Qt::MoveAction);
        QCoreApplication::sendEvent(w, &enter);
        QDragMoveEvent firstMove(anchor, offered, &mime, Qt::LeftButton, Qt::ShiftModifier);
        firstMove.setDropAction(Qt::MoveAction);
        QCoreApplication::sendEvent(w, &firstMove);
        QVERIFY(firstMove.isAccepted());
        QCOMPARE(firstMove.dropAction(), Qt::CopyAction);
        QCOMPARE(ctl->hostWindow(), w);
        QTRY_COMPARE_WITH_TIMEOUT(ctl->stage(), OverlayController::Live, 10000);

        // Slice 0 (MP3) is straight above the centre.
        const QPointF onSlice0 = anchor + QPointF(0, -(WheelGeometry::kInnerRadius + 40));
        QDragMoveEvent move(onSlice0, offered, &mime, Qt::LeftButton, Qt::ShiftModifier);
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
        QDragEnterEvent enter(QPointF(300, 300), Qt::CopyAction | Qt::MoveAction, &empty, Qt::LeftButton, Qt::ShiftModifier);
        enter.setDropAction(Qt::MoveAction);
        QCoreApplication::sendEvent(w, &enter);
        QDragMoveEvent move(QPointF(300, 300), Qt::CopyAction | Qt::MoveAction, &empty, Qt::LeftButton, Qt::ShiftModifier);
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

    // Home window with files queued, cycling through every tab.
    void homeWithFiles()
    {
        g_messages.clear();
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
        for (int i : {1, 2, 3, 0}) {
            tabs->setProperty("currentIndex", i);
            QTest::qWait(50);
        }
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
