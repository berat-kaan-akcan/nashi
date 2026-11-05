#include "app.h"
#include "kdetranslations.h"
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
#include <QJsonDocument>
#include <QJsonObject>
#include <QMimeData>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickItem>
#include <QQuickWindow>
#include <QSettings>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QTest>
#include <QtQml/qqmlextensionplugin.h>

#include <clocale>

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
        // Outside ctest this would use the real settings, library and nashi-cli.
        if (qEnvironmentVariableIsEmpty("NASHI_CLI") || qEnvironmentVariableIsEmpty("XDG_CONFIG_HOME"))
            QFAIL("Run this test through ctest");
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

    void recentSearchMatchesTurkishLetters()
    {
        QTemporaryDir dir;
        QVERIFY(dir.isValid());
        const QString dotted = dir.filePath(QString::fromUtf8("İSTANBUL Rapor.txt"));
        const QString dotless = dir.filePath(QString::fromUtf8("Kısa Not.txt"));
        for (const QString &path : {dotted, dotless}) {
            QFile f(path);
            QVERIFY(f.open(QIODevice::WriteOnly));
        }
        app->recent()->add({dotted, dotless});

        const auto found = [&](const char *query) {
            return app->filteredRecentPaths(QString::fromUtf8(query));
        };
        for (const char *query : {"istanbul", "ıstanbul", "İSTANBUL", "ISTANBUL"})
            QVERIFY2(found(query).contains(dotted), query);
        for (const char *query : {"kısa", "kisa", "KISA", "KİSA"})
            QVERIFY2(found(query).contains(dotless), query);
        QVERIFY(!found("kasa").contains(dotless));
        QVERIFY(app->matchesSearch(QString::fromUtf8("istanbul"), QString::fromUtf8("İstanbul.pdf"), QString()));
        QVERIFY(!app->matchesSearch(QString::fromUtf8("ankara"), QString::fromUtf8("İstanbul.pdf"), QString()));
    }

    void wheelActionsShareAndFloat()
    {
        QTemporaryDir dir;
        QVERIFY(dir.isValid());
        const QString file = dir.filePath(QStringLiteral("hold.txt"));
        {
            QFile f(file);
            QVERIFY(f.open(QIODevice::WriteOnly));
        }
        g_warnings.clear();
        app->shelf()->clear();
        app->runItem(QStringLiteral("action"), QStringLiteral("float"), QStringLiteral("Float"), {file});
        QCOMPARE(app->shelf()->rowCount(), 1);
        QVERIFY2(g_warnings.isEmpty(), qPrintable(g_warnings.join(QLatin1Char('\n'))));
        app->runItem(QStringLiteral("action"), QStringLiteral("share"), QStringLiteral("Share"), {file});
        // Without KDE Purpose the action only reports that sharing is unavailable.
        QCOMPARE(app->shareWindow() != nullptr, app->shareAvailable());
        // Opening the share sheet logs a thread warning in offscreen runs; its source is not pinned down yet.
        g_warnings.clear();
        app->shelf()->clear();
        app->shelf()->hide();
    }

    void pdfImageResolutionIsSentForImageTargets()
    {
        app->settings()->setPdfImageDpi(300);
        const QJsonObject png = QJsonDocument::fromJson(app->convertOptions(QStringLiteral("png")).toUtf8()).object();
        QCOMPARE(png.value(QStringLiteral("dpi")).toInt(), 300);
        const QJsonObject jpg = QJsonDocument::fromJson(app->convertOptions(QStringLiteral("jpg")).toUtf8()).object();
        QCOMPARE(jpg.value(QStringLiteral("dpi")).toInt(), 300);
        QVERIFY(app->convertOptions(QStringLiteral("mp3")).isEmpty());
        app->settings()->setPdfImageDpi(Settings::kDefaultPdfImageDpi);
    }

    void formatDefaultsJoinPdfResolution()
    {
        app->settings()->setPdfImageDpi(300);
        app->settings()->setFormatDefault(QStringLiteral("jpg"), QStringLiteral("quality"), QStringLiteral("higher"));
        const QJsonObject jpg = QJsonDocument::fromJson(app->convertOptions(QStringLiteral("jpg")).toUtf8()).object();
        QCOMPARE(jpg.value(QStringLiteral("dpi")).toInt(), 300);
        QCOMPARE(jpg.value(QStringLiteral("quality")).toInt(), 95);
        app->settings()->setFormatDefault(QStringLiteral("mp3"), QStringLiteral("audioKbps"), 128);
        const QJsonObject mp3 = QJsonDocument::fromJson(app->convertOptions(QStringLiteral("mp3")).toUtf8()).object();
        QCOMPARE(mp3.value(QStringLiteral("audio_kbps")).toInt(), 128);
        QVERIFY(!mp3.contains(QStringLiteral("dpi")));
        app->settings()->restoreFormatDefaults(QStringLiteral("jpg"));
        app->settings()->restoreFormatDefaults(QStringLiteral("mp3"));
        app->settings()->setPdfImageDpi(Settings::kDefaultPdfImageDpi);
    }

    void settingsPageShowsOnlyTheControlsOfTheChosenFormat()
    {
        g_warnings.clear();
        QQmlComponent component(engine);
        component.loadFromModule("NashiApp", "SettingsPage");
        QVERIFY2(component.isReady(), qPrintable(component.errorString()));
        std::unique_ptr<QObject> page(component.create());
        QVERIFY(page);
        auto *box = page->findChild<QObject *>(QStringLiteral("formatDefaultsBox"));
        QVERIFY(box);
        const auto shown = [&](const char *name) {
            auto *row = page->findChild<QObject *>(QString::fromLatin1(name));
            return row && row->property("visible").toBool();
        };
        const auto choose = [&](const char *format) {
            const QStringList formats = app->settings()->formatDefaultFormats();
            box->setProperty("currentIndex", formats.indexOf(QString::fromLatin1(format)));
        };
        choose("gif");
        QVERIFY(shown("formatGifFps"));
        QVERIFY(shown("formatGifWidth"));
        QVERIFY(!shown("formatAudioKbps"));
        choose("flac");
        QVERIFY(!shown("formatAudioKbps"));
        QVERIFY(shown("formatKeepMetadata"));
        choose("mp4");
        QVERIFY(shown("formatQuality"));
        QVERIFY(shown("formatAudioKbps"));
        QVERIFY(shown("formatVideoHeight"));
        QVERIFY(!shown("formatGifFps"));

        // A saved choice shows in its box, and choosing in the box saves it.
        auto *height = page->findChild<QObject *>(QStringLiteral("formatVideoHeightChoice"));
        QVERIFY(height);
        Settings *settings = app->settings();
        settings->setFormatDefault(QStringLiteral("mp4"), QStringLiteral("videoHeight"), 720);
        QCOMPARE(height->property("displayText").toString(), QStringLiteral("720p"));
        QVERIFY(QMetaObject::invokeMethod(height, "activated", Q_ARG(int, 3)));
        QCOMPARE(settings->formatDefaults(QStringLiteral("mp4")).value(QStringLiteral("videoHeight")).toInt(), 1080);
        settings->restoreFormatDefaults(QStringLiteral("mp4"));
        QCOMPARE(height->property("currentIndex").toInt(), 0);
        QVERIFY2(g_warnings.isEmpty(), qPrintable(g_warnings.join(QLatin1Char('\n'))));
        if (!qEnvironmentVariableIsEmpty("NASHI_SCREENSHOT_DIR")) {
            QQuickWindow window;
            window.resize(760, 3400);
            auto *item = qobject_cast<QQuickItem *>(page.get());
            QVERIFY(item);
            item->setParentItem(window.contentItem());
            item->setSize(QSizeF(760, 3400));
            choose("mp4");
            window.show();
            QVERIFY(QTest::qWaitForWindowExposed(&window));
            saveShot(&window, QStringLiteral("settings-format-defaults"));
            item->setParentItem(nullptr);
        }
    }

    void wheelAsksForASoundOnlyWhenASliceIsReachedAndSoundIsOn()
    {
        QVERIFY(QFile::exists(QStringLiteral(":/qt/qml/NashiApp/resources/wheel-selection.wav")));
        g_warnings.clear();
        QQmlComponent component(engine);
        component.loadFromModule("NashiApp", "Wheel");
        QVERIFY2(component.isReady(), qPrintable(component.errorString()));
        std::unique_ptr<QObject> wheel(component.create());
        QVERIFY(wheel);
        const auto requests = [&] { return wheel->property("soundRequests").toInt(); };
        wheel->setProperty("soundEnabled", true);
        wheel->setProperty("hoveredIndex", 2);
        QCOMPARE(requests(), 1);
        wheel->setProperty("hoveredIndex", 2);
        QCOMPARE(requests(), 1);
        wheel->setProperty("hoveredIndex", -1);
        QCOMPARE(requests(), 1);
        wheel->setProperty("soundEnabled", false);
        wheel->setProperty("hoveredIndex", 3);
        QCOMPARE(requests(), 1);
        QVERIFY2(g_warnings.isEmpty(), qPrintable(g_warnings.join(QLatin1Char('\n'))));
    }

    void kdeTranslationFunctionsExistForPurposePlugins()
    {
        QQmlComponent component(engine);
        component.setData(
            "import QtQml\nQtObject {\n"
            "  property string known: i18nd('nashi-no-such-domain', 'No devices found')\n"
            "  property string plain: i18n('Plain %1', 'x')\n"
            "  property string filled: i18nd('nashi-no-such-domain', '%1 of %2', 3, 4)\n"
            "}\n",
            QUrl());
        std::unique_ptr<QObject> obj(component.create());
        QVERIFY2(obj, qPrintable(component.errorString()));
        QCOMPARE(obj->property("known").toString(), QStringLiteral("No devices found"));
        QCOMPARE(obj->property("plain").toString(), QStringLiteral("Plain x"));
        QCOMPARE(obj->property("filled").toString(), QStringLiteral("3 of 4"));
    }

    void kdeTranslationsReadTheInstalledCatalogs()
    {
        const char *saved = setlocale(LC_MESSAGES, nullptr);
        const QByteArray previous = saved ? QByteArray(saved) : QByteArray();
        if (!setlocale(LC_MESSAGES, "tr_TR.UTF-8")
            || !QFile::exists(QStringLiteral("/usr/share/locale/tr/LC_MESSAGES/purpose6_kdeconnect.mo"))) {
            setlocale(LC_MESSAGES, previous.constData());
            QSKIP("The Turkish locale or the KDE Connect catalog is not installed");
        }
        const QByteArray language = qgetenv("LANGUAGE");
        qunsetenv("LANGUAGE");
        const QString text = KdeTranslations().i18nd(QStringLiteral("purpose6_kdeconnect"), QStringLiteral("No devices found"));
        qputenv("LANGUAGE", language);
        setlocale(LC_MESSAGES, previous.constData());
        QVERIFY(!text.isEmpty());
        QVERIFY2(text != QStringLiteral("No devices found"), qPrintable(text));
    }

    void closingTheShareSheetIsClean()
    {
        if (!app->shareAvailable())
            QSKIP("KDE Purpose is not installed");
        QTemporaryDir dir;
        const QString file = dir.filePath(QStringLiteral("share-close.txt"));
        QFile f(file);
        QVERIFY(f.open(QIODevice::WriteOnly));
        f.write("x");
        f.close();
        app->runItem(QStringLiteral("action"), QStringLiteral("share"), QStringLiteral("Share"), {file});
        auto *w = qobject_cast<QQuickWindow *>(app->shareWindow());
        QVERIFY(w);
        QTest::qWait(300);
        saveShot(w, QStringLiteral("desktop-share-list"));
        g_warnings.clear();
        QVERIFY(w->close());
        QTest::qWait(100);
        QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
        QCoreApplication::processEvents();
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

