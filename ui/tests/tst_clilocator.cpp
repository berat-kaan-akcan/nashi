#include "clilocator.h"

#include <QFile>
#include <QTemporaryDir>
#include <QTest>

class TestCliLocator : public QObject
{
    Q_OBJECT

    static QString makeExe(const QString &dir, const QString &name)
    {
        const QString path = dir + QLatin1Char('/') + name;
        QFile f(path);
        if (!f.open(QIODevice::WriteOnly))
            return {};
        f.write("#!/bin/sh\nexit 0\n");
        f.close();
        f.setPermissions(QFileDevice::ReadOwner | QFileDevice::WriteOwner | QFileDevice::ExeOwner);
        return path;
    }

private Q_SLOTS:
    void envWins()
    {
        QTemporaryDir app, path;
        makeExe(app.path(), QStringLiteral("nashi-cli"));
        makeExe(path.path(), QStringLiteral("nashi-cli"));
        QCOMPARE(CliLocator::locate(QStringLiteral(FAKE_CLI), app.path(), path.path()), QStringLiteral(FAKE_CLI));
    }

    void envFromProcessEnvironment()
    {
        // ctest sets NASHI_CLI to the fake CLI.
        QCOMPARE(CliLocator::locate(), QStringLiteral(FAKE_CLI));
    }

    void envBareNameUsesPath()
    {
        QTemporaryDir path;
        const QString exe = makeExe(path.path(), QStringLiteral("my-cli"));
        QCOMPARE(CliLocator::locate(QStringLiteral("my-cli"), QString(), path.path()), exe);
    }

    void siblingBeforePath()
    {
        QTemporaryDir app, path;
        const QString sibling = makeExe(app.path(), QStringLiteral("nashi-cli"));
        makeExe(path.path(), QStringLiteral("nashi-cli"));
        QCOMPARE(CliLocator::locate(QString(), app.path(), path.path()), sibling);
        // A bogus $NASHI_CLI falls through to the next candidates.
        QCOMPARE(CliLocator::locate(QStringLiteral("/nonexistent/nashi-cli"), app.path(), path.path()), sibling);
    }

    void pathFallback()
    {
        QTemporaryDir app, path;
        const QString inPath = makeExe(path.path(), QStringLiteral("nashi-cli"));
        QCOMPARE(CliLocator::locate(QString(), app.path(), path.path()), inPath);
    }

    void notExecutableIgnored()
    {
        QTemporaryDir app, path;
        QFile f(app.path() + QStringLiteral("/nashi-cli"));
        QVERIFY(f.open(QIODevice::WriteOnly));
        f.close();
        f.setPermissions(QFileDevice::ReadOwner | QFileDevice::WriteOwner);
        QCOMPARE(CliLocator::locate(QString(), app.path(), path.path()), QString());
    }
};

QTEST_GUILESS_MAIN(TestCliLocator)
#include "tst_clilocator.moc"
