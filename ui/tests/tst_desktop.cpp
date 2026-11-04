#include "app.h"
#include "jobmanager.h"
#include "jobrunner.h"
#include "recentmodel.h"
#include "selection.h"
#include "settings.h"
#include "shelfmodel.h"

#include <QDragEnterEvent>
#include <QDragMoveEvent>
#include <QDropEvent>
#include <QFile>
#include <QImage>
#include <QMimeData>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickWindow>
#include <QSettings>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QTest>
#include <QtQml/qqmlextensionplugin.h>

Q_IMPORT_QML_PLUGIN(NashiAppPlugin)

namespace {
QStringList g_warnings;
QtMessageHandler g_prevHandler = nullptr;

void warningCapture(QtMsgType type, const QMessageLogContext &ctx, const QString &msg)
{
    if (type != QtDebugMsg && type != QtInfoMsg)
        g_warnings.append(msg);
    if (g_prevHandler)
        g_prevHandler(type, ctx, msg);
}

void saveShot(QQuickWindow *w, const QString &name)
{
    const QString dir = qEnvironmentVariable("NASHI_SCREENSHOT_DIR");
    if (dir.isEmpty() || !w)
        return;
    QTest::qWait(150);
    w->grabWindow().save(dir + QLatin1Char('/') + name + QStringLiteral(".png"));
}
} // namespace

class TestDesktop : public QObject
{
    Q_OBJECT

    App *app = nullptr;
    QQmlEngine *engine = nullptr;

private Q_SLOTS:
    void initTestCase()
    {
        g_prevHandler = qInstallMessageHandler(warningCapture);
        app = new App(App::Options{false, true}, this);
        // Mark welcome wizard done so windows open directly in tests.
        app->finishWelcome();
        engine = new QQmlEngine(this);
        app->init(engine);
        connect(engine, &QQmlEngine::warnings, this, [](const QList<QQmlError> &errors) {
            for (const QQmlError &e : errors)
                g_warnings.append(e.toString());
        });
        if (auto *hw = app->findChild<QQuickWindow *>())
            hw->hide();
    }

    void cleanupTestCase()
    {
        delete engine;
        delete app;
        qInstallMessageHandler(g_prevHandler);
    }

    void shelfModelPersistenceAndDedupe()
    {
        QTemporaryDir dir;
        QVERIFY(dir.isValid());
        const QString f1 = dir.filePath(QStringLiteral("alpha.png"));
        const QString f2 = dir.filePath(QStringLiteral("beta.txt"));
        const QString missing = dir.filePath(QStringLiteral("ghost.pdf"));
        {
            QFile a(f1);
            QVERIFY(a.open(QIODevice::WriteOnly));
            a.write("png");
            QFile b(f2);
            QVERIFY(b.open(QIODevice::WriteOnly));
            b.write("hello");
        }

        const QString confPath = dir.filePath(QStringLiteral("shelf.ini"));
        {
            QSettings store(confPath, QSettings::IniFormat);
            ShelfModel model(&store);
            QCOMPARE(model.rowCount(), 0);

            model.add({f1, missing, f2});
            QCOMPARE(model.rowCount(), 2);
            // add({f1, f2}) puts the last valid item or preserves order at top;
            // re-adding f1 must deduplicate and move f1 to row 0.
            model.add({f1});
            QCOMPARE(model.rowCount(), 2);
            QCOMPARE(model.files().constFirst(), QFileInfo(f1).absoluteFilePath());

            const QString uris = model.uriListFor(model.files());
            QVERIFY(uris.contains(QLatin1String("file://")));
            QVERIFY(uris.endsWith(QLatin1String("\r\n")));
        }
        {
            QSettings store(confPath, QSettings::IniFormat);
            ShelfModel reloaded(&store);
            QCOMPARE(reloaded.rowCount(), 2);
            reloaded.remove(0);
            QCOMPARE(reloaded.rowCount(), 1);
            reloaded.clear();
            QCOMPARE(reloaded.rowCount(), 0);
        }
    }

    void downloadWindowAndJob()
    {
        g_warnings.clear();
        QTemporaryDir outDir;
        QVERIFY(outDir.isValid());

        QVERIFY(app->isValidWebUrl(QStringLiteral("https://example.org/video")));
        QVERIFY(!app->isValidWebUrl(QStringLiteral("file:///etc/passwd")));
        QVERIFY(!app->isValidWebUrl(QStringLiteral("-o/tmp/pwn")));

        app->showDownload(QStringLiteral("https://example.org/watch?v=42"), false);
        auto *w = qobject_cast<QQuickWindow *>(app->downloadWindow());
        QVERIFY(w);
        QVERIFY(QTest::qWaitForWindowExposed(w));
        QCOMPARE(w->property("urlValid").toBool(), true);
        saveShot(w, QStringLiteral("desktop-download-light"));

        QVERIFY(app->startDownload(QStringLiteral("https://example.org/watch?v=42"), QStringLiteral("audio"),
                                   outDir.path()));
        w->hide();

        QTRY_VERIFY_WITH_TIMEOUT(app->jobs()->activeCount() == 0, 10000);
        // Compare the entry, not the count: Recent is capped and this file survives between runs.
        RecentModel *recent = app->recent();
        QCOMPARE(recent->data(recent->index(0), RecentModel::PathRole).toString(),
                 outDir.filePath(QStringLiteral("downloaded.mp3")));
        QVERIFY2(g_warnings.isEmpty(), qPrintable(g_warnings.join(QLatin1Char('\n'))));
    }

    void previewWindowModesAndReuse()
    {
        g_warnings.clear();
        QTemporaryDir dir;
        QVERIFY(dir.isValid());

        const QString txtPath = dir.filePath(QStringLiteral("notes.txt"));
        {
            QFile f(txtPath);
            QVERIFY(f.open(QIODevice::WriteOnly));
            f.write("Hello Nashi preview!\nLine two.\n");
        }
        const QString imgPath = dir.filePath(QStringLiteral("sample.png"));
        {
            QImage img(160, 120, QImage::Format_RGBA8888);
            img.fill(QColor("#30543b"));
            QVERIFY(img.save(imgPath, "PNG"));
        }
        const QString pdfPath = dir.filePath(QStringLiteral("doc.pdf"));
        {
            QFile f(pdfPath);
            QVERIFY(f.open(QIODevice::WriteOnly));
            f.write("%PDF-1.4\n");
        }

        const QStringList list{txtPath, imgPath, pdfPath};
        app->showPreview(txtPath, list);
        auto *w = qobject_cast<QQuickWindow *>(app->previewWindow());
        QVERIFY(w);
        w->resize(760, 560);
        QVERIFY(QTest::qWaitForWindowExposed(w));
        QTRY_COMPARE_WITH_TIMEOUT(w->property("viewMode").toString(), QStringLiteral("text"), 5000);
        QVERIFY(w->property("textContent").toString().contains(QLatin1String("Hello Nashi")));
        saveShot(w, QStringLiteral("desktop-preview-text"));

        // Step right to image, then right to PDF.
        QTest::keyClick(w, Qt::Key_Right);
        QTRY_COMPARE_WITH_TIMEOUT(w->property("viewMode").toString(), QStringLiteral("image"), 5000);
        saveShot(w, QStringLiteral("desktop-preview-image"));

        QTest::keyClick(w, Qt::Key_Right);
        QTRY_COMPARE_WITH_TIMEOUT(w->property("viewMode").toString(), QStringLiteral("pdf"), 5000);
        QTRY_VERIFY_WITH_TIMEOUT(w->property("pageImages").toList().size() >= 1, 5000);
        saveShot(w, QStringLiteral("desktop-preview-pdf"));

        // Reuse puts the file back into Home's selection.
        app->reuseFiles({imgPath});
        QCOMPARE(app->home()->files(), QStringList{QFileInfo(imgPath).absoluteFilePath()});

        w->hide();
        QVERIFY2(g_warnings.isEmpty(), qPrintable(g_warnings.join(QLatin1Char('\n'))));
    }

    void shelfWindowDropAcceptsCopy()
    {
        g_warnings.clear();
        QTemporaryDir dir;
        QVERIFY(dir.isValid());
        const QString file = dir.filePath(QStringLiteral("pinned.png"));
        {
            QImage img(64, 64, QImage::Format_RGBA8888);
            img.fill(QColor("#e3b859"));
            QVERIFY(img.save(file, "PNG"));
        }

        app->shelf()->clear();
        app->shelf()->show();
        auto *w = qobject_cast<QQuickWindow *>(app->shelfWindow());
        QVERIFY(w);
        QVERIFY(QTest::qWaitForWindowExposed(w));

        QMimeData mime;
        mime.setUrls({QUrl::fromLocalFile(file)});
        const Qt::DropActions offered = Qt::CopyAction | Qt::MoveAction;
        const QPointF pt(w->width() / 2.0, w->height() / 2.0);

        QDragEnterEvent enter(pt.toPoint(), offered, &mime, Qt::LeftButton, Qt::ShiftModifier);
        enter.setDropAction(Qt::MoveAction);
        QCoreApplication::sendEvent(w, &enter);

        QDragMoveEvent move(pt.toPoint(), offered, &mime, Qt::LeftButton, Qt::ShiftModifier);
        move.setDropAction(Qt::MoveAction);
        QCoreApplication::sendEvent(w, &move);
        QVERIFY(move.isAccepted());
        QCOMPARE(move.dropAction(), Qt::CopyAction);

        QDropEvent drop(pt, offered, &mime, Qt::LeftButton, Qt::ShiftModifier);
        drop.setDropAction(Qt::MoveAction);
        QCoreApplication::sendEvent(w, &drop);
        QVERIFY(drop.isAccepted());
        QCOMPARE(drop.dropAction(), Qt::CopyAction);

        QCOMPARE(app->shelf()->rowCount(), 1);
        saveShot(w, QStringLiteral("desktop-shelf-light"));

        app->shelf()->hide();
        app->shelf()->clear();
        QVERIFY2(g_warnings.isEmpty(), qPrintable(g_warnings.join(QLatin1Char('\n'))));
    }

    void shareSheetLoadsCleanly()
    {
        g_warnings.clear();
        QQmlComponent component(engine);
        component.loadFromModule("NashiApp", "ShareSheet");
        std::unique_ptr<QObject> obj(component.create());
        QVERIFY2(obj, qPrintable(component.errorString()));
        auto *w = qobject_cast<QQuickWindow *>(obj.get());
        QVERIFY(w);
        w->show();
        QVERIFY(QTest::qWaitForWindowExposed(w));
        saveShot(w, QStringLiteral("desktop-share"));
        w->hide();
        obj.reset();
        QCoreApplication::processEvents();
        QVERIFY2(g_warnings.isEmpty(), qPrintable(g_warnings.join(QLatin1Char('\n'))));
    }
};

QTEST_MAIN(TestDesktop)
#include "tst_desktop.moc"

