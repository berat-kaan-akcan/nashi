#include "jobmanager.h"
#include "jobrunner.h"

#include <QElapsedTimer>
#include <QPointer>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QTest>

class TestJobRunner : public QObject
{
    Q_OBJECT

    static bool waitFinished(JobRunner &job, int ms = 15000)
    {
        if (job.finished())
            return true;
        QSignalSpy spy(&job, &JobRunner::jobFinished);
        return spy.wait(ms);
    }

private Q_SLOTS:
    void initTestCase() { QVERIFY(QFileInfo(QStringLiteral(FAKE_CLI)).isExecutable()); }

    void success()
    {
        JobRunner job(QStringLiteral("MP3"));
        QSignalSpy progress(&job, &JobRunner::progressChanged);
        QSignalSpy produced(&job, &JobRunner::outputProduced);
        job.start(QStringLiteral(FAKE_CLI),
                  JobManager::buildArgs(QStringLiteral("convert"), QStringLiteral("mp3"),
                                        {QStringLiteral("/tmp/a.mp4"), QStringLiteral("/tmp/b.mov")}, QString()));
        QVERIFY(job.running());
        QVERIFY(waitFinished(job));
        QCOMPARE(job.state(), JobRunner::Succeeded);
        QCOMPARE(job.jobId(), QStringLiteral("7f3c"));
        QCOMPARE(job.total(), 2);
        QCOMPARE(job.outputs(), QStringList({QStringLiteral("/tmp/a.mp3"), QStringLiteral("/tmp/b.mp3")}));
        QCOMPARE(produced.count(), 2);
        QVERIFY(progress.count() >= 3);
        QCOMPARE(job.message(), QStringLiteral("Encoding"));
        QCOMPARE(job.overallProgress(), 1.0);
        QCOMPARE(job.failedCount(), 0);
        QVERIFY(job.errors().isEmpty());
    }

    void toolFailure()
    {
        JobRunner job(QStringLiteral("x"));
        job.start(QStringLiteral(FAKE_CLI), JobManager::buildArgs(QStringLiteral("tool"), QStringLiteral("fail"),
                                                                  {QStringLiteral("/tmp/b.mov")}, QString()));
        QVERIFY(waitFinished(job));
        QCOMPARE(job.state(), JobRunner::Failed);
        QCOMPARE(job.failedCount(), 1);
        QCOMPARE(job.errors().size(), 1);
        const QVariantMap e = job.errors().constFirst().toMap();
        QCOMPARE(e.value(QStringLiteral("code")).toString(), QStringLiteral("tool_failed"));
        QVERIFY(job.errorSummary().contains(QStringLiteral("b.mov")));
    }

    void garbageLinesIgnored()
    {
        JobRunner job(QStringLiteral("x"));
        job.start(QStringLiteral(FAKE_CLI), JobManager::buildArgs(QStringLiteral("convert"), QStringLiteral("garbage"),
                                                                  {QStringLiteral("/tmp/a.png")}, QString()));
        QVERIFY(waitFinished(job));
        QCOMPARE(job.state(), JobRunner::Succeeded);
        QCOMPARE(job.outputs().size(), 1);
    }

    void exitWithoutDone()
    {
        JobRunner job(QStringLiteral("x"));
        job.start(QStringLiteral(FAKE_CLI), JobManager::buildArgs(QStringLiteral("tool"), QStringLiteral("nodone"),
                                                                  {QStringLiteral("/tmp/a.png")}, QString()));
        QVERIFY(waitFinished(job));
        QCOMPARE(job.state(), JobRunner::Failed);
        QVERIFY(job.errorSummary().contains(QStringLiteral("something went wrong")));
    }

    void cancel()
    {
        JobRunner job(QStringLiteral("slow"));
        QSignalSpy progress(&job, &JobRunner::progressChanged);
        job.start(QStringLiteral(FAKE_CLI), JobManager::buildArgs(QStringLiteral("tool"), QStringLiteral("slow"),
                                                                  {QStringLiteral("/tmp/a.mp4")}, QString()));
        QTRY_VERIFY_WITH_TIMEOUT(job.total() == 1 && job.indeterminate(), 10000);
        job.cancel();
        QVERIFY(waitFinished(job));
        QCOMPARE(job.state(), JobRunner::Cancelled);
    }

    void cancelKillsStubbornProcess()
    {
        JobRunner job(QStringLiteral("stubborn"));
        job.setKillTimeout(300);
        job.start(QStringLiteral(FAKE_CLI), JobManager::buildArgs(QStringLiteral("tool"), QStringLiteral("stubborn"),
                                                                  {QStringLiteral("/tmp/a.mp4")}, QString()));
        QTRY_VERIFY_WITH_TIMEOUT(job.total() == 1, 10000);
        job.cancel();
        QVERIFY(waitFinished(job, 5000));
        QCOMPARE(job.state(), JobRunner::Cancelled);
    }

    void multiInputProgressCoversWholeJob()
    {
        JobRunner fed(QStringLiteral("x"));
        fed.feed("{\"event\":\"start\",\"job\":\"j\",\"total\":4}\n"
                 "{\"event\":\"progress\",\"input\":null,\"index\":null,\"fraction\":0.8}\n");
        QVERIFY(fed.wholeJobProgress());
        QCOMPARE(fed.overallProgress(), 0.8); // not 0.8 / 4
        fed.feed("{\"event\":\"progress\",\"index\":1,\"fraction\":0.5}\n");
        QVERIFY(!fed.wholeJobProgress());
        QCOMPARE(fed.overallProgress(), 1.5 / 4);

        JobRunner job(QStringLiteral("merge"));
        double maxSeen = 0;
        connect(&job, &JobRunner::progressChanged, this, [&] {
            if (job.running())
                maxSeen = qMax(maxSeen, job.overallProgress());
        });
        job.start(QStringLiteral(FAKE_CLI),
                  JobManager::buildArgs(QStringLiteral("tool"), QStringLiteral("merge"),
                                        {QStringLiteral("/tmp/a.pdf"), QStringLiteral("/tmp/b.pdf"), QStringLiteral("/tmp/c.pdf")}, {}));
        QVERIFY(waitFinished(job));
        QCOMPARE(job.state(), JobRunner::Succeeded);
        QCOMPARE(job.total(), 3);
        QCOMPARE(maxSeen, 0.75);
        QCOMPARE(job.outputs(), QStringList{QStringLiteral("/tmp/merged.pdf")});
    }

    void outputMessageKept()
    {
        JobRunner job(QStringLiteral("qr"));
        job.start(QStringLiteral(FAKE_CLI), JobManager::buildArgs(QStringLiteral("tool"), QStringLiteral("qr"),
                                                                  {QStringLiteral("/tmp/code.png")}, {}));
        QVERIFY(waitFinished(job));
        QCOMPARE(job.outputMessages(), QStringList{QStringLiteral("https://example.org/42")});
    }

    // Quitting must let the CLI finish its SIGTERM cleanup (~1 s) instead of
    // killing it right away.
    void shutdownWaitsForCleanup()
    {
        QTemporaryDir dir;
        const QString mark = dir.filePath(QStringLiteral("cleaned"));
        qputenv("FAKE_CLI_CLEANUP_MARK", mark.toUtf8());
        JobManager mgr(nullptr);
        mgr.setProgram(QStringLiteral(FAKE_CLI));
        JobRunner *job = mgr.start(QStringLiteral("tool"), QStringLiteral("slowclean"), QStringLiteral("x"),
                                   {QStringLiteral("/tmp/a.mp4")});
        QVERIFY(job);
        QTRY_VERIFY_WITH_TIMEOUT(job->total() == 1, 10000);
        bool done = false;
        QElapsedTimer t;
        t.start();
        mgr.shutdown(5000, [&] { done = true; });
        QTRY_VERIFY_WITH_TIMEOUT(done, 8000);
        QVERIFY(t.elapsed() >= 900);
        QCOMPARE(job->state(), JobRunner::Cancelled);
        QVERIFY(job->doneReceived()); // reported done(cancelled) itself, not killed
        QVERIFY(QFileInfo::exists(mark));
        qunsetenv("FAKE_CLI_CLEANUP_MARK");
    }

    void shutdownKillsAfterTimeout()
    {
        JobManager mgr(nullptr);
        mgr.setProgram(QStringLiteral(FAKE_CLI));
        JobRunner *job = mgr.start(QStringLiteral("tool"), QStringLiteral("stubborn"), QStringLiteral("x"),
                                   {QStringLiteral("/tmp/a.mp4")});
        QTRY_VERIFY_WITH_TIMEOUT(job->total() == 1, 10000);
        bool done = false;
        mgr.shutdown(400, [&] { done = true; });
        QTRY_VERIFY_WITH_TIMEOUT(done, 5000);
        QTRY_VERIFY_WITH_TIMEOUT(job->finished(), 3000);
        QCOMPARE(job->state(), JobRunner::Cancelled);
    }

    void removeRunningWaitsForFinish()
    {
        QTemporaryDir dir;
        const QString mark = dir.filePath(QStringLiteral("cleaned"));
        qputenv("FAKE_CLI_CLEANUP_MARK", mark.toUtf8());
        JobManager mgr(nullptr);
        mgr.setProgram(QStringLiteral(FAKE_CLI));
        QPointer<JobRunner> job = mgr.start(QStringLiteral("tool"), QStringLiteral("slowclean"), QStringLiteral("x"),
                                            {QStringLiteral("/tmp/a.mp4")});
        QTRY_VERIFY_WITH_TIMEOUT(job->total() == 1, 10000);
        mgr.remove(job);
        QCOMPARE(mgr.rowCount(), 1); // still listed while cleaning up
        QVERIFY(job);
        QTRY_COMPARE_WITH_TIMEOUT(mgr.rowCount(), 0, 8000);
        QTRY_VERIFY_WITH_TIMEOUT(job.isNull(), 2000);
        QVERIFY(QFileInfo::exists(mark));
        qunsetenv("FAKE_CLI_CLEANUP_MARK");
    }

    void destructorTerminatesGracefully()
    {
        QTemporaryDir dir;
        const QString mark = dir.filePath(QStringLiteral("cleaned"));
        qputenv("FAKE_CLI_CLEANUP_MARK", mark.toUtf8());
        auto *job = new JobRunner(QStringLiteral("x"));
        job->start(QStringLiteral(FAKE_CLI), JobManager::buildArgs(QStringLiteral("tool"), QStringLiteral("slowclean"),
                                                                   {QStringLiteral("/tmp/a.mp4")}, {}));
        QTRY_VERIFY_WITH_TIMEOUT(job->total() == 1, 10000);
        delete job; // SIGTERM + wait, not an immediate SIGKILL
        QVERIFY(QFileInfo::exists(mark));
        qunsetenv("FAKE_CLI_CLEANUP_MARK");
    }

    void missingProgram()
    {
        JobRunner job(QStringLiteral("x"));
        job.start(QStringLiteral("/nonexistent/nashi-cli"), {QStringLiteral("deps")});
        QVERIFY(waitFinished(job));
        QCOMPARE(job.state(), JobRunner::Failed);
        QVERIFY(!job.errors().isEmpty());

        JobRunner empty(QStringLiteral("y"));
        empty.start(QString(), {});
        QCOMPARE(empty.state(), JobRunner::Failed);
    }

    void feedParsesEventsIncrementally()
    {
        JobRunner job(QStringLiteral("x"));
        job.feed("{\"event\":\"start\",\"job\":\"j\",\"total\":4}\n{\"event\":\"progress\",\"index\":1,");
        QCOMPARE(job.total(), 4);
        job.feed("\"fraction\":0.5,\"message\":\"m\"}\n");
        QCOMPARE(job.currentIndex(), 1);
        QCOMPARE(job.fraction(), 0.5);
        QCOMPARE(job.overallProgress(), 1.5 / 4);
        job.feed("{\"event\":\"progress\",\"index\":2,\"fraction\":null}\n");
        QVERIFY(job.indeterminate());
        job.feed("{\"event\":\"output\",\"index\":null,\"input\":null,\"path\":\"/m.pdf\"}\n");
        QCOMPARE(job.outputs(), QStringList{QStringLiteral("/m.pdf")});
    }

    void buildArgs()
    {
        const QStringList a = JobManager::buildArgs(QStringLiteral("convert"), QStringLiteral("png"),
                                                    {QStringLiteral("/x/-weird.jpg")}, QStringLiteral("/out"));
        QCOMPARE(a, QStringList({QStringLiteral("convert"), QStringLiteral("png"), QStringLiteral("/x/-weird.jpg"),
                                 QStringLiteral("--out-dir"), QStringLiteral("/out"), QStringLiteral("--json")}));
        const QStringList t = JobManager::buildArgs(QStringLiteral("tool"), QStringLiteral("compress"),
                                                    {QStringLiteral("/a b/c.mp4")}, QString());
        QCOMPARE(t, QStringList({QStringLiteral("tool"), QStringLiteral("compress"), QStringLiteral("/a b/c.mp4"),
                                 QStringLiteral("--json")}));
        // Relative names are made absolute so they can never look like options.
        const QStringList r = JobManager::buildArgs(QStringLiteral("tool"), QStringLiteral("x"), {QStringLiteral("-rf")}, {});
        QVERIFY(r.at(2).startsWith(QLatin1Char('/')));
    }

    void managerRunsAndLists()
    {
        JobManager mgr(nullptr);
        mgr.setProgram(QStringLiteral(FAKE_CLI));
        QSignalSpy finished(&mgr, &JobManager::jobFinished);
        QVERIFY(!mgr.start(QStringLiteral("action"), QStringLiteral("copy"), QString(), {QStringLiteral("/a")}));
        JobRunner *job = mgr.start(QStringLiteral("convert"), QStringLiteral("gif"), QStringLiteral("GIF"),
                                   {QStringLiteral("/tmp/v.mp4")});
        QVERIFY(job);
        QCOMPARE(mgr.rowCount(), 1);
        QCOMPARE(mgr.activeCount(), 1);
        QVERIFY(finished.wait(15000));
        QCOMPARE(mgr.activeCount(), 0);
        QCOMPARE(job->outputs(), QStringList{QStringLiteral("/tmp/v.gif")});
        mgr.clearFinished();
        QCOMPARE(mgr.rowCount(), 0);
    }
};

QTEST_GUILESS_MAIN(TestJobRunner)
#include "tst_jobrunner.moc"
