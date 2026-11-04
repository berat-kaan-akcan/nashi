#include "clipboardimport.h"
#include "selection.h"

#include <QClipboard>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QImage>
#include <QImageReader>
#include <QMimeData>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QTest>
#include <QUrl>

class TestClipboardImport : public QObject
{
    Q_OBJECT

    QTemporaryDir m_dir;

    QString importsDir() const { return m_dir.filePath(QStringLiteral("imports")); }

    void setUp(ClipboardImport &ci) const
    {
        ci.setFolder(importsDir());
        ci.setClock([] { return QDateTime(QDate(2026, 10, 31), QTime(14, 5, 3)); });
    }

    QStringList visibleImports() const
    {
        return QDir(importsDir()).entryList(QDir::Files | QDir::Hidden, QDir::Name);
    }

private Q_SLOTS:
    void init()
    {
        QDir(importsDir()).removeRecursively();
    }

    void defaultFolderUnderDataHome()
    {
        QVERIFY(ClipboardImport::defaultFolder().endsWith(QStringLiteral("/nashi/imports")));
    }

    void files()
    {
        ClipboardImport ci;
        setUp(ci);
        const QString a = m_dir.filePath(QStringLiteral("a.txt"));
        const QString folder = m_dir.filePath(QStringLiteral("folder"));
        QFile f(a);
        QVERIFY(f.open(QIODevice::WriteOnly));
        f.write("x");
        f.close();
        QVERIFY(QDir().mkpath(folder));

        QMimeData mime;
        mime.setUrls({QUrl::fromLocalFile(a), QUrl::fromLocalFile(folder), QUrl::fromLocalFile(a),
                      QUrl(QStringLiteral("https://example.org/x.png")),
                      QUrl::fromLocalFile(m_dir.filePath(QStringLiteral("missing.bin")))});
        const auto r = ci.import(&mime);
        QCOMPARE(r.kind, ClipboardImport::Files);
        QCOMPARE(r.files, QStringList({a, folder}));
        // Input files are used in place; nothing is written to the imports folder.
        QVERIFY(!QFileInfo::exists(importsDir()));
    }

    void missingFiles()
    {
        ClipboardImport ci;
        setUp(ci);
        QMimeData mime;
        mime.setUrls({QUrl::fromLocalFile(m_dir.filePath(QStringLiteral("gone.png")))});
        const auto r = ci.import(&mime);
        QCOMPARE(r.kind, ClipboardImport::Failed);
        QVERIFY(r.files.isEmpty());
        QVERIFY(!r.message.isEmpty());
    }

    void image()
    {
        ClipboardImport ci;
        setUp(ci);
        QImage img(7, 5, QImage::Format_ARGB32);
        img.fill(Qt::red);
        QMimeData mime;
        mime.setImageData(img);

        const auto r = ci.import(&mime);
        QCOMPARE(r.kind, ClipboardImport::Image);
        QCOMPARE(r.files.size(), 1);
        QCOMPARE(QFileInfo(r.files.constFirst()).fileName(), QStringLiteral("Pasted image 2026-10-31 14.05.03.png"));
        QImageReader reader(r.files.constFirst());
        QCOMPARE(reader.format(), QByteArray("png"));
        QCOMPARE(reader.read().size(), QSize(7, 5));

        // Same timestamp again: a new unique name, the first file is kept.
        const auto r2 = ci.import(&mime);
        QCOMPARE(r2.kind, ClipboardImport::Image);
        QCOMPARE(QFileInfo(r2.files.constFirst()).fileName(),
                 QStringLiteral("Pasted image 2026-10-31 14.05.03 (2).png"));
        QVERIFY(QFileInfo::exists(r.files.constFirst()));
        // No temp files left behind.
        QCOMPARE(visibleImports().size(), 2);
    }

    void text()
    {
        ClipboardImport ci;
        setUp(ci);
        const QString content = QStringLiteral("Merhaba dünya\nsecond line https://example.org\n");
        QMimeData mime;
        mime.setText(content);

        const auto r = ci.import(&mime);
        QCOMPARE(r.kind, ClipboardImport::Text);
        QCOMPARE(r.files.size(), 1);
        QCOMPARE(QFileInfo(r.files.constFirst()).fileName(), QStringLiteral("Pasted text 2026-10-31 14.05.03.txt"));
        QFile f(r.files.constFirst());
        QVERIFY(f.open(QIODevice::ReadOnly));
        QCOMPARE(QString::fromUtf8(f.readAll()), content);
        QCOMPARE(visibleImports().size(), 1);
    }

    void link_data()
    {
        QTest::addColumn<QString>("text");
        QTest::newRow("https") << QStringLiteral("https://www.youtube.com/watch?v=abc");
        QTest::newRow("http padded") << QStringLiteral("  http://example.org/video.mp4\n");
    }

    void link()
    {
        QFETCH(QString, text);
        ClipboardImport ci;
        setUp(ci);
        QMimeData mime;
        mime.setText(text);
        const auto r = ci.import(&mime);
        QCOMPARE(r.kind, ClipboardImport::Link);
        QCOMPARE(r.link, text.trimmed());
        QVERIFY(r.files.isEmpty());
        QCOMPARE(r.message, QStringLiteral("Downloading links is coming soon"));
        QVERIFY(!QFileInfo::exists(importsDir()));
    }

    void webUrlOnly()
    {
        // A browser copy may put only a web URL into text/uri-list.
        ClipboardImport ci;
        setUp(ci);
        QMimeData mime;
        mime.setUrls({QUrl(QStringLiteral("https://example.org/page"))});
        const auto r = ci.import(&mime);
        QCOMPARE(r.kind, ClipboardImport::Link);
    }

    void notALink()
    {
        ClipboardImport ci;
        setUp(ci);
        QMimeData mime;
        mime.setText(QStringLiteral("ftp://example.org/file"));
        QCOMPARE(ci.import(&mime).kind, ClipboardImport::Text);
    }

    void pasteIntoEmitsLinkPasted()
    {
        ClipboardImport ci;
        setUp(ci);
        Selection sel(nullptr);
        QSignalSpy linkSpy(&ci, &ClipboardImport::linkPasted);
        QSignalSpy msgSpy(&ci, &ClipboardImport::message);
        QGuiApplication::clipboard()->setText(QStringLiteral("https://example.com/video"));
        QVERIFY(ci.pasteInto(&sel));
        QCOMPARE(linkSpy.count(), 1);
        QCOMPARE(linkSpy.at(0).at(0).toString(), QStringLiteral("https://example.com/video"));
        QCOMPARE(msgSpy.count(), 0);
        QCOMPARE(sel.fileCount(), 0);
    }

    void empty_data()
    {
        QTest::addColumn<QString>("text");
        QTest::newRow("no data") << QString();
        QTest::newRow("whitespace") << QStringLiteral("  \n\t ");
    }

    void empty()
    {
        QFETCH(QString, text);
        ClipboardImport ci;
        setUp(ci);
        QMimeData mime;
        if (!text.isNull())
            mime.setText(text);
        const auto r = ci.import(&mime);
        QCOMPARE(r.kind, ClipboardImport::Nothing);
        QVERIFY(r.files.isEmpty());
        QCOMPARE(r.message, QStringLiteral("Nothing to paste"));
        QCOMPARE(ci.import(nullptr).kind, ClipboardImport::Nothing);
        QVERIFY(!QFileInfo::exists(importsDir()));
    }
};

QTEST_MAIN(TestClipboardImport)
#include "tst_clipboardimport.moc"
