#include "jsonlinebuffer.h"

#include <QTest>

class TestJsonLines : public QObject
{
    Q_OBJECT
private Q_SLOTS:
    void wholeLines()
    {
        JsonLineBuffer b;
        const auto evs = b.append("{\"event\":\"start\",\"total\":2}\n{\"event\":\"done\"}\n");
        QCOMPARE(evs.size(), 2);
        QCOMPARE(evs.at(0).value(QStringLiteral("total")).toInt(), 2);
        QCOMPARE(evs.at(1).value(QStringLiteral("event")).toString(), QStringLiteral("done"));
        QVERIFY(!b.hasPending());
    }

    void partialLines()
    {
        JsonLineBuffer b;
        QVERIFY(b.append("{\"event\":\"pro").isEmpty());
        QVERIFY(b.hasPending());
        QVERIFY(b.append("gress\",\"fraction\":0.5").isEmpty());
        const auto evs = b.append("}\n{\"event\":");
        QCOMPARE(evs.size(), 1);
        QCOMPARE(evs.at(0).value(QStringLiteral("fraction")).toDouble(), 0.5);
        const auto rest = b.append("\"done\"}\n");
        QCOMPARE(rest.size(), 1);
    }

    void byteByByte()
    {
        JsonLineBuffer b;
        const QByteArray data = "{\"a\":1}\n{\"b\":\"ü\"}\n";
        int count = 0;
        for (char c : data)
            count += int(b.append(QByteArray(1, c)).size());
        QCOMPARE(count, 2);
    }

    void crlfBlankAndMalformed()
    {
        JsonLineBuffer b;
        const auto evs = b.append("{\"a\":1}\r\n\n   \nnot json\n[1,2]\n{\"b\":2}\n");
        QCOMPARE(evs.size(), 2);
        QCOMPARE(b.malformedCount(), 2);
    }

    void flushAtEof()
    {
        JsonLineBuffer b;
        QVERIFY(b.append("{\"event\":\"done\"}").isEmpty());
        const auto evs = b.flush();
        QCOMPARE(evs.size(), 1);
        QVERIFY(!b.hasPending());
        QVERIFY(b.flush().isEmpty());
    }
};

QTEST_GUILESS_MAIN(TestJsonLines)
#include "tst_jsonlines.moc"
