#include "setupcheck.h"

#include <QSettings>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QTest>

class TestSetupCheck : public QObject
{
    Q_OBJECT
private Q_SLOTS:
    void evaluate_data()
    {
        QTest::addColumn<bool>("exists");
        QTest::addColumn<QList<unsigned>>("process");
        QTest::addColumn<QList<unsigned>>("account");
        QTest::addColumn<int>("expected");

        QTest::newRow("no group") << false << QList<unsigned>{994} << QList<unsigned>{994}
                                  << int(SetupCheck::NoGroup);
        QTest::newRow("session has it") << true << QList<unsigned>{1000, 994} << QList<unsigned>{1000, 994}
                                        << int(SetupCheck::InGroup);
        QTest::newRow("effective gid") << true << QList<unsigned>{994} << QList<unsigned>{} << int(SetupCheck::InGroup);
        QTest::newRow("added, not relogged") << true << QList<unsigned>{1000, 998}
                                             << QList<unsigned>{1000, 998, 994} << int(SetupCheck::NeedsRelogin);
        QTest::newRow("not a member") << true << QList<unsigned>{1000, 998} << QList<unsigned>{1000, 998}
                                      << int(SetupCheck::NotInGroup);
    }

    void evaluate()
    {
        QFETCH(bool, exists);
        QFETCH(QList<unsigned>, process);
        QFETCH(QList<unsigned>, account);
        QFETCH(int, expected);
        QCOMPARE(int(SetupCheck::evaluate(exists, 994, process, account)), expected);
    }

    void welcomeFlagPersists()
    {
        QTemporaryDir dir;
        QVERIFY(dir.isValid());
        const QString file = dir.filePath(QStringLiteral("nashi.conf"));
        {
            QSettings store(file, QSettings::IniFormat);
            SetupCheck check(&store);
            QVERIFY(!check.welcomeDone());
            QSignalSpy spy(&check, &SetupCheck::welcomeDoneChanged);
            check.setWelcomeDone(true);
            check.setWelcomeDone(true); // no change, no signal
            QCOMPARE(spy.count(), 1);
            QVERIFY(check.welcomeDone());
        }
        QSettings reopened(file, QSettings::IniFormat);
        QCOMPARE(reopened.value(SetupCheck::kWelcomeKey).toBool(), true);
        SetupCheck again(&reopened);
        QVERIFY(again.welcomeDone());
    }

    void commandsAreCopyOnly()
    {
        QSettings store(QTemporaryDir().filePath(QStringLiteral("x.conf")), QSettings::IniFormat);
        SetupCheck check(&store);
        QCOMPARE(check.groupCommand(), QStringLiteral("sudo usermod -aG input $USER"));
        QCOMPARE(check.serviceCommand(), QStringLiteral("systemctl --user enable --now nashi-inputd.service"));
    }

    // The live lookup must give a consistent answer on any machine.
    void refreshIsConsistent()
    {
        QSettings store(QTemporaryDir().filePath(QStringLiteral("y.conf")), QSettings::IniFormat);
        SetupCheck check(&store);
        check.refresh();
        QCOMPARE(check.inInputGroup(), check.groupState() == SetupCheck::InGroup);
    }
};

QTEST_MAIN(TestSetupCheck)
#include "tst_setupcheck.moc"
