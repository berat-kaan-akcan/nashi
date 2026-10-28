#include "overlaycontroller.h"
#include "selection.h"
#include "wheelmodel.h"
#include "wheelquery.h"

#include <QSignalSpy>
#include <QTest>

// The overlay state machine, driven like inputd + OverlayWindow.qml would.
class TestOverlay : public QObject
{
    Q_OBJECT

    WheelQuery *query = nullptr;
    OverlayController *ctl = nullptr;
    QObject windowA;
    QObject windowB;

    static QList<QUrl> urls(const QStringList &paths)
    {
        QList<QUrl> out;
        for (const QString &p : paths)
            out.append(QUrl::fromLocalFile(p));
        return out;
    }

private Q_SLOTS:
    void init()
    {
        query = new WheelQuery(this);
        query->setProgram(QStringLiteral(FAKE_CLI));
        ctl = new OverlayController(query, this);
        ctl->setGraceMs(100);
    }
    void cleanup()
    {
        delete ctl;
        delete query;
    }

    void chordShowsTarget()
    {
        QSignalSpy active(ctl, &OverlayController::activeChanged);
        ctl->onChordStarted(QStringLiteral("tools"));
        QVERIFY(ctl->active());
        QCOMPARE(ctl->stage(), OverlayController::Target);
        QCOMPARE(ctl->mode(), QStringLiteral("tools"));
        QCOMPARE(active.count(), 1);
        QVERIFY(!ctl->wantsKeyboard());
    }

    void singleStageLiveDrop()
    {
        QSignalSpy chosen(ctl, &OverlayController::itemChosen);
        ctl->onChordStarted(QStringLiteral("convert"));
        ctl->dragEntered(&windowA, urls({QStringLiteral("/tmp/a.mp4")}), QPointF(400, 300));
        QCOMPARE(ctl->hostWindow(), &windowA);
        QCOMPARE(ctl->anchor(), QPointF(400, 300));
        QTRY_COMPARE_WITH_TIMEOUT(ctl->stage(), OverlayController::Live, 10000);
        QCOMPARE(ctl->selection()->model()->rowCount(), 3);

        ctl->onDragEnded(); // button release usually precedes the drop
        ctl->dropped(&windowA, urls({QStringLiteral("/tmp/a.mp4")}), QPointF(400, 200), 1);
        QCOMPARE(chosen.count(), 1);
        QCOMPARE(chosen.at(0).at(0).toString(), QStringLiteral("convert"));
        QCOMPARE(chosen.at(0).at(1).toString(), QStringLiteral("gif"));
        QCOMPARE(chosen.at(0).at(2).toString(), QStringLiteral("GIF"));
        QCOMPARE(chosen.at(0).at(3).toStringList(), QStringList{QStringLiteral("/tmp/a.mp4")});
        QCOMPARE(ctl->stage(), OverlayController::Hidden);
        QCOMPARE(ctl->hostWindow(), nullptr);
    }

    void dropOnCentreCancels()
    {
        QSignalSpy chosen(ctl, &OverlayController::itemChosen);
        ctl->onChordStarted(QStringLiteral("convert"));
        ctl->dragEntered(&windowA, urls({QStringLiteral("/tmp/a.mp4")}), QPointF(10, 10));
        QTRY_COMPARE_WITH_TIMEOUT(ctl->stage(), OverlayController::Live, 10000);
        ctl->dropped(&windowA, {}, QPointF(10, 10), -1);
        QCOMPARE(chosen.count(), 0);
        QVERIFY(!ctl->active());
    }

    void dropOnUnavailableCancels()
    {
        QSignalSpy chosen(ctl, &OverlayController::itemChosen);
        ctl->onChordStarted(QStringLiteral("convert"));
        ctl->dragEntered(&windowA, urls({QStringLiteral("/tmp/a.mp4")}), QPointF(10, 10));
        QTRY_COMPARE_WITH_TIMEOUT(ctl->stage(), OverlayController::Live, 10000);
        ctl->dropped(&windowA, {}, QPointF(10, 10), 2); // webm: missing ffmpeg
        QCOMPARE(chosen.count(), 0);
        QVERIFY(!ctl->active());
    }

    void twoStageWhenUrlsUnreadable()
    {
        QSignalSpy chosen(ctl, &OverlayController::itemChosen);
        ctl->onChordStarted(QStringLiteral("tools"));
        ctl->dragEntered(&windowA, {}, QPointF(50, 60)); // no URLs before the drop
        QCOMPARE(ctl->stage(), OverlayController::Target);
        ctl->dropped(&windowA, urls({QStringLiteral("/tmp/p.png")}), QPointF(55, 65), -3);
        QCOMPARE(ctl->stage(), OverlayController::Pick);
        QVERIFY(ctl->wantsKeyboard());
        QVERIFY(ctl->dropReceived());
        ctl->onDragEnded(); // must not hide a dropped overlay
        QTest::qWait(250);
        QCOMPARE(ctl->stage(), OverlayController::Pick);
        QTRY_VERIFY_WITH_TIMEOUT(ctl->selection()->ready(), 10000);
        QCOMPARE(ctl->selection()->model()->rowCount(), 2); // trim (needs_editor) hidden
        ctl->choose(1); // removeMetadata: unavailable → ignored in Pick
        QCOMPARE(ctl->stage(), OverlayController::Pick);
        ctl->choose(0);
        QCOMPARE(chosen.count(), 1);
        QCOMPARE(chosen.at(0).at(0).toString(), QStringLiteral("tool"));
        QCOMPARE(chosen.at(0).at(1).toString(), QStringLiteral("compress"));
        QVERIFY(!ctl->active());
    }

    void pickCancelledByEscapeOrClickOutside()
    {
        ctl->onChordStarted(QStringLiteral("convert"));
        ctl->dropped(&windowA, urls({QStringLiteral("/tmp/p.png")}), QPointF(1, 1), -3);
        QCOMPARE(ctl->stage(), OverlayController::Pick);
        ctl->cancel();
        QVERIFY(!ctl->active());
        QVERIFY(ctl->selection()->files().isEmpty());
    }

    void dragEndedWithoutDropHidesAfterGrace()
    {
        ctl->onChordStarted(QStringLiteral("convert"));
        ctl->onChordReleased(); // keeps the overlay
        QVERIFY(ctl->active());
        ctl->onDragEnded();
        QVERIFY(ctl->active()); // still within grace
        QTRY_VERIFY_WITH_TIMEOUT(!ctl->active(), 2000);
    }

    void dropWithinGraceKeepsOverlay()
    {
        ctl->onChordStarted(QStringLiteral("convert"));
        ctl->onDragEnded();
        QTest::qWait(30);
        ctl->dropped(&windowA, urls({QStringLiteral("/tmp/p.png")}), QPointF(1, 1), -3);
        QTest::qWait(250);
        QCOMPARE(ctl->stage(), OverlayController::Pick);
    }

    void modeSwitchDuringDrag()
    {
        ctl->onChordStarted(QStringLiteral("convert"));
        ctl->dragEntered(&windowA, urls({QStringLiteral("/tmp/a.mp4")}), QPointF(10, 10));
        QTRY_COMPARE_WITH_TIMEOUT(ctl->stage(), OverlayController::Live, 10000);
        QCOMPARE(ctl->selection()->model()->rowCount(), 3);
        QSignalSpy mode(ctl, &OverlayController::modeChanged);
        ctl->onChordStarted(QStringLiteral("actions"));
        QCOMPARE(mode.count(), 1);
        QCOMPARE(ctl->stage(), OverlayController::Live);
        QCOMPARE(ctl->selection()->model()->rowCount(), 5);
    }

    void hostFollowsScreens()
    {
        ctl->onChordStarted(QStringLiteral("convert"));
        ctl->dragEntered(&windowA, {}, QPointF(10, 10));
        ctl->dragLeft(&windowA);
        ctl->dragEntered(&windowB, {}, QPointF(20, 30));
        QCOMPARE(ctl->hostWindow(), &windowB);
        QCOMPARE(ctl->anchor(), QPointF(20, 30));
    }

    void daemonLossHidesUndropped()
    {
        ctl->onChordStarted(QStringLiteral("convert"));
        ctl->onDaemonLost();
        QVERIFY(!ctl->active());
    }

    void showForFilesGoesStraightToPick()
    {
        QSignalSpy shown(ctl, &OverlayController::shown);
        ctl->showForFiles({QStringLiteral("/tmp/a.mp4")}, QStringLiteral("actions"), &windowA, QPointF(5, 5));
        QCOMPARE(shown.count(), 1);
        QCOMPARE(ctl->stage(), OverlayController::Pick);
        QCOMPARE(ctl->mode(), QStringLiteral("actions"));
        QTRY_VERIFY_WITH_TIMEOUT(ctl->selection()->ready(), 10000);
        QCOMPARE(ctl->selection()->model()->rowCount(), 5);
        // A new chord replaces a pending pick.
        ctl->onChordStarted(QStringLiteral("convert"));
        QCOMPARE(ctl->stage(), OverlayController::Target);
    }
};

QTEST_GUILESS_MAIN(TestOverlay)
#include "tst_overlay.moc"
