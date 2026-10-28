#include "selection.h"
#include "wheelmodel.h"
#include "wheelquery.h"

#include <QJsonArray>
#include <QJsonDocument>
#include <QSignalSpy>
#include <QTest>

namespace {
QJsonObject sampleWheel()
{
    return QJsonDocument::fromJson(R"({
      "files": [{"path": "/a.mp4", "ext": "mp4", "category": "video"}],
      "convert": [{"id": "mp3", "title": "MP3", "available": true},
                  {"id": "webm", "title": "WEBM", "available": false, "missing": ["ffmpeg"]},
                  {"title": "no id"}],
      "tools": [{"id": "compress", "title": {"en": "Compress", "tr": "Sıkıştır"}, "icon": "compress", "available": true, "needs_editor": false},
                {"id": "trim", "title": {"en": "Trim", "tr": "Kırp"}, "available": true, "needs_editor": true},
                {"id": "removeMetadata", "title": {"en": "Remove Metadata"}, "available": false, "missing": ["exiftool", "qpdf"]}],
      "actions": [{"id": "copyPath", "title": {"en": "Copy Path", "tr": "Yolu Kopyala"}, "icon": "edit-copy"}]
    })").object();
}
} // namespace

class TestWheelModel : public QObject
{
    Q_OBJECT
private Q_SLOTS:
    void convertList()
    {
        const auto items = WheelModel::buildItems(sampleWheel(), QStringLiteral("convert"), QStringLiteral("en"));
        QCOMPARE(items.size(), 2); // entry without id dropped
        QCOMPARE(items.at(0).id, QStringLiteral("mp3"));
        QCOMPARE(items.at(0).kind, QStringLiteral("convert"));
        QCOMPARE(items.at(0).title, QStringLiteral("MP3"));
        QVERIFY(items.at(0).available);
        QVERIFY(!items.at(1).available);
        QCOMPARE(items.at(1).missing, QStringList{QStringLiteral("ffmpeg")});
    }

    void toolsHideEditorsAndLocalise()
    {
        const auto tr = WheelModel::buildItems(sampleWheel(), QStringLiteral("tools"), QStringLiteral("tr"));
        QCOMPARE(tr.size(), 2);
        QCOMPARE(tr.at(0).id, QStringLiteral("compress"));
        QCOMPARE(tr.at(0).title, QStringLiteral("Sıkıştır"));
        QCOMPARE(tr.at(0).kind, QStringLiteral("tool"));
        QCOMPARE(tr.at(1).title, QStringLiteral("Remove Metadata")); // falls back to en
        QCOMPARE(tr.at(1).missing.size(), 2);
        for (const auto &i : tr)
            QVERIFY(i.id != QLatin1String("trim"));
    }

    void actionsAndModeNormalisation()
    {
        const auto a = WheelModel::buildItems(sampleWheel(), QStringLiteral("ACTIONS"), QStringLiteral("en"));
        QCOMPARE(a.size(), 1);
        QCOMPARE(a.at(0).kind, QStringLiteral("action"));
        QVERIFY(a.at(0).available); // actions have no "available" → true
        QCOMPARE(WheelModel::normalizeMode(QStringLiteral("bogus")), QStringLiteral("convert"));
        QCOMPARE(WheelModel::normalizeMode(QString()), QStringLiteral("convert"));
    }

    void modelRoles()
    {
        WheelModel m;
        QSignalSpy count(&m, &WheelModel::countChanged);
        m.setWheel(sampleWheel());
        QCOMPARE(m.rowCount(), 2);
        m.setMode(QStringLiteral("tools"));
        QCOMPARE(m.rowCount(), 2);
        QCOMPARE(m.data(m.index(1), WheelModel::MissingRole).toString(), QStringLiteral("exiftool, qpdf"));
        QCOMPARE(m.data(m.index(1), WheelModel::AvailableRole).toBool(), false);
        m.setLanguage(QStringLiteral("tr"));
        QCOMPARE(m.get(0).value(QStringLiteral("title")).toString(), QStringLiteral("Sıkıştır"));
        QCOMPARE(m.countFor(QStringLiteral("actions")), 1);
        QVERIFY(count.count() >= 3);
        m.clear();
        QCOMPARE(m.rowCount(), 0);
        QVERIFY(!m.hasData());
    }

    void queryAgainstFakeCli()
    {
        WheelQuery q;
        q.setProgram(QStringLiteral(FAKE_CLI));
        QSignalSpy ready(&q, &WheelQuery::ready);
        const QStringList files{QStringLiteral("/tmp/clip.mp4")};
        const QString key = q.request(files);
        QVERIFY(ready.wait(10000));
        QCOMPARE(ready.at(0).at(0).toString(), key);
        const QJsonObject w = ready.at(0).at(1).toJsonObject();
        QCOMPARE(w.value(QStringLiteral("files")).toArray().size(), 1);
        QVERIFY(q.cached(files));

        // Cached answers are still delivered asynchronously.
        q.request(files);
        QCOMPARE(ready.count(), 1);
        QVERIFY(ready.wait(1000));
        QCOMPARE(ready.count(), 2);
        QCOMPARE(WheelQuery::keyFor({QStringLiteral("b"), QStringLiteral("a")}),
                 WheelQuery::keyFor({QStringLiteral("a"), QStringLiteral("b")}));
    }

    void queryFailure()
    {
        WheelQuery q;
        q.setProgram(QStringLiteral(FAKE_CLI));
        QSignalSpy failed(&q, &WheelQuery::failed);
        q.request({QStringLiteral("/tmp/x.bad")});
        QVERIFY(failed.wait(10000));
        QVERIFY(failed.at(0).at(1).toString().contains(QStringLiteral("cannot inspect")));
    }

    void selectionFiltersLocalFiles()
    {
        const QStringList f = Selection::localFiles(
            {QUrl(QStringLiteral("file:///tmp/a.png")), QUrl(QStringLiteral("https://example.com/x.png")),
             QUrl(QStringLiteral("file:///tmp/a.png")), QUrl(QStringLiteral("file:///tmp/b%20c.jpg"))});
        QCOMPARE(f, QStringList({QStringLiteral("/tmp/a.png"), QStringLiteral("/tmp/b c.jpg")}));

        WheelQuery q;
        q.setProgram(QStringLiteral(FAKE_CLI));
        Selection sel(&q);
        sel.setFiles({QStringLiteral("/tmp/a.png")});
        QVERIFY(sel.loading());
        QTRY_VERIFY_WITH_TIMEOUT(sel.ready(), 10000);
        QCOMPARE(sel.model()->rowCount(), 3); // convert: mp3, gif, webm (unavailable)
        QCOMPARE(sel.summary(), QStringLiteral("a.png"));
    }
};

QTEST_GUILESS_MAIN(TestWheelModel)
#include "tst_wheelmodel.moc"
