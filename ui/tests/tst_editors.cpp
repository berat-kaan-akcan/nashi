#include "app.h"
#include "previewclient.h"
#include "jobmanager.h"
#include "settings.h"
#include "wheelmodel.h"

#include <QColor>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QFont>
#include <QImage>
#include <QLinearGradient>
#include <QPainter>
#include <QQmlEngine>
#include <QQuickItem>
#include <QQuickWindow>
#include <QTemporaryDir>
#include <QJSValue>
#include <QTest>
#include <QVariantList>
#include <QVariantMap>
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

void saveEditorScreenshots(App *app, QQuickWindow *w, const QString &editorName)
{
    const QString dir = qEnvironmentVariable("NASHI_SCREENSHOT_DIR");
    if (dir.isEmpty() || !w)
        return;
    QDir().mkpath(dir);
    const QString prev = app->settings()->appearance();
    for (const QString &look : {QStringLiteral("light"), QStringLiteral("dark")}) {
        app->settings()->setAppearance(look);
        QTest::qWait(180);
        const QImage grab = w->grabWindow();
        grab.save(dir + QStringLiteral("/editor-") + editorName + QLatin1Char('-') + look + QStringLiteral(".png"));
    }
    app->settings()->setAppearance(prev);
    QTest::qWait(60);
}

bool createSamplePhoto(const QString &path, int w = 640, int h = 360)
{
    QImage img(w, h, QImage::Format_ARGB32);
    QPainter p(&img);
    p.setRenderHint(QPainter::Antialiasing, true);

    // Warm sky gradient
    QLinearGradient sky(0, 0, 0, h * 0.72);
    sky.setColorAt(0.0, QColor(214, 229, 216));
    sky.setColorAt(0.55, QColor(246, 236, 206));
    sky.setColorAt(1.0, QColor(238, 214, 168));
    p.fillRect(0, 0, w, h, sky);

    // Sun disc
    p.setPen(Qt::NoPen);
    p.setBrush(QColor(227, 184, 89));
    p.drawEllipse(QPointF(w * 0.76, h * 0.27), 42, 42);

    // Rolling forest hills
    p.setBrush(QColor(102, 140, 108));
    p.drawEllipse(QRectF(-80, h * 0.50, w * 0.85, h * 0.75));
    p.setBrush(QColor(48, 84, 59));
    p.drawEllipse(QRectF(w * 0.25, h * 0.56, w * 0.95, h * 0.75));

    // Sample subject card inside the scene (good visual anchor for crop/redact/annotate)
    const QRectF card(76, 70, 230, 130);
    p.setBrush(QColor(255, 253, 247, 235));
    p.setPen(QPen(QColor(48, 84, 59, 90), 2));
    p.drawRoundedRect(card, 14, 14);

    p.setPen(Qt::NoPen);
    p.setBrush(QColor(48, 84, 59));
    p.drawRoundedRect(QRectF(96, 92, 110, 14), 5, 5);
    p.setBrush(QColor(130, 142, 126));
    p.drawRoundedRect(QRectF(96, 118, 180, 10), 4, 4);
    p.drawRoundedRect(QRectF(96, 138, 155, 10), 4, 4);
    p.setBrush(QColor(227, 184, 89));
    p.drawRoundedRect(QRectF(96, 162, 92, 20), 6, 6);

    p.end();
    return img.save(path, "PNG");
}

bool createDummyFile(const QString &path, const QByteArray &bytes = QByteArray("nashi-test-fixture"))
{
    QFile f(path);
    if (!f.open(QIODevice::WriteOnly))
        return false;
    f.write(bytes);
    return true;
}
} // namespace

class TestEditors : public QObject
{
    Q_OBJECT

    App *app = nullptr;
    QQmlEngine *engine = nullptr;
    QTemporaryDir tempDir;
    QString photoPath;
    QString videoPath;
    QString audioPath;
    QString pdfPath;

    QQuickWindow *openEditor(const QString &toolId, const QString &title, const QString &file)
    {
        app->showOptions(toolId, title, {file});
        auto *w = qobject_cast<QQuickWindow *>(app->optionsWindow());
        if (!w)
            return nullptr;
        w->show();
        if (!QTest::qWaitForWindowExposed(w))
            return nullptr;
        return w;
    }

private Q_SLOTS:
    void initTestCase()
    {
        QVERIFY(tempDir.isValid());
        photoPath = tempDir.filePath(QStringLiteral("photo.png"));
        videoPath = tempDir.filePath(QStringLiteral("clip.mp4"));
        audioPath = tempDir.filePath(QStringLiteral("voice.mp3"));
        pdfPath = tempDir.filePath(QStringLiteral("report.pdf"));

        QVERIFY(createSamplePhoto(photoPath, 640, 360));
        QVERIFY(createDummyFile(videoPath));
        QVERIFY(createDummyFile(audioPath));
        QVERIFY(createDummyFile(pdfPath));

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

    void hasOptionsFormIncludesAnnotate()
    {
        g_messages.clear();
        QVERIFY(WheelModel::hasOptionsForm(QStringLiteral("annotateImage")));
        QVERIFY(WheelModel::hasOptionsForm(QStringLiteral("cropImage")));
        QVERIFY(WheelModel::hasOptionsForm(QStringLiteral("redactImage")));
        QVERIFY(WheelModel::hasOptionsForm(QStringLiteral("trimVideo")));
        QVERIFY(WheelModel::hasOptionsForm(QStringLiteral("organizePDF")));
        QVERIFY2(g_messages.isEmpty(), qPrintable(g_messages.join(QLatin1Char('\n'))));
    }

    void cropImageEditor()
    {
        g_messages.clear();
        QQuickWindow *w = openEditor(QStringLiteral("cropImage"), QStringLiteral("Crop Image"), photoPath);
        QVERIFY(w);
        QCOMPARE(w->width(), 880);

        auto *editor = w->findChild<QObject *>(QStringLiteral("rectEditor"));
        QVERIFY(editor);

        // Wait for async inspect (640x360) and frame preview resolution.
        QTRY_COMPARE_WITH_TIMEOUT(w->property("mediaWidth").toInt(), 640, 10000);
        QCOMPARE(w->property("mediaHeight").toInt(), 360);
        QTRY_VERIFY_WITH_TIMEOUT(!editor->property("resolvedImageUrl").toString().isEmpty(), 10000);

        // Default full-image crop
        QVariantMap res = w->property("result").toMap();
        QVariantMap rect = res.value(QStringLiteral("rect")).toMap();
        QCOMPARE(rect.value(QStringLiteral("x")).toInt(), 0);
        QCOMPARE(rect.value(QStringLiteral("y")).toInt(), 0);
        QCOMPARE(rect.value(QStringLiteral("w")).toInt(), 640);
        QCOMPARE(rect.value(QStringLiteral("h")).toInt(), 360);

        // Aspect ratio preset 1:1 -> centered 360x360 at (140, 0)
        QMetaObject::invokeMethod(editor, "selectAspect", Q_ARG(QVariant, QStringLiteral("1:1")),
                                  Q_ARG(QVariant, 1), Q_ARG(QVariant, 1));
        rect = w->property("result").toMap().value(QStringLiteral("rect")).toMap();
        QCOMPARE(rect.value(QStringLiteral("x")).toInt(), 140);
        QCOMPARE(rect.value(QStringLiteral("y")).toInt(), 0);
        QCOMPARE(rect.value(QStringLiteral("w")).toInt(), 360);
        QCOMPARE(rect.value(QStringLiteral("h")).toInt(), 360);

        // Programmatic crop box update -> syncs precision fields & JSON output
        QMetaObject::invokeMethod(editor, "setCropRect", Q_ARG(QVariant, 60), Q_ARG(QVariant, 45),
                                  Q_ARG(QVariant, 440), Q_ARG(QVariant, 250));
        auto *rwField = w->findChild<QObject *>(QStringLiteral("rectWidthField"));
        QVERIFY(rwField);
        QCOMPARE(rwField->property("text").toString(), QStringLiteral("440"));

        rect = w->property("result").toMap().value(QStringLiteral("rect")).toMap();
        QCOMPARE(rect.value(QStringLiteral("x")).toInt(), 60);
        QCOMPARE(rect.value(QStringLiteral("y")).toInt(), 45);
        QCOMPARE(rect.value(QStringLiteral("w")).toInt(), 440);
        QCOMPARE(rect.value(QStringLiteral("h")).toInt(), 250);

        saveEditorScreenshots(app, w, QStringLiteral("cropImage"));

        // Precision field edit syncs back to RectEditor
        rwField->setProperty("text", QStringLiteral("320"));
        QCOMPARE(editor->property("rectW").toInt(), 320);
        QCOMPARE(w->property("result").toMap().value(QStringLiteral("rect")).toMap().value(QStringLiteral("w")).toInt(),
                 320);

        w->hide();
        QVERIFY2(g_messages.isEmpty(), qPrintable(g_messages.join(QLatin1Char('\n'))));
    }

    void redactImageMultiRectAndStyles()
    {
        g_messages.clear();
        QQuickWindow *w = openEditor(QStringLiteral("redactImage"), QStringLiteral("Redact Image"), photoPath);
        QVERIFY(w);

        auto *editor = w->findChild<QObject *>(QStringLiteral("rectEditor"));
        QVERIFY(editor);

        QTRY_COMPARE_WITH_TIMEOUT(w->property("mediaWidth").toInt(), 640, 10000);
        QTRY_VERIFY_WITH_TIMEOUT(!editor->property("resolvedImageUrl").toString().isEmpty(), 10000);

        // Initial default redaction rect exists and is valid
        QVariantList rects = w->property("result").toMap().value(QStringLiteral("rects")).toList();
        QCOMPARE(rects.size(), 1);
        QCOMPARE(rects.at(0).toMap().value(QStringLiteral("style")).toString(), QStringLiteral("black"));

        // Move/resize first rect (black) and add two more rects (blur + pixelate)
        QMetaObject::invokeMethod(editor, "updateSelectedRect", Q_ARG(QVariant, 84), Q_ARG(QVariant, 84),
                                  Q_ARG(QVariant, 140), Q_ARG(QVariant, 44));
        QMetaObject::invokeMethod(editor, "addRedactRect", Q_ARG(QVariant, 84), Q_ARG(QVariant, 140),
                                  Q_ARG(QVariant, 195), Q_ARG(QVariant, 48),
                                  Q_ARG(QVariant, QStringLiteral("blur")));
        QMetaObject::invokeMethod(editor, "addRedactRect", Q_ARG(QVariant, 370), Q_ARG(QVariant, 65),
                                  Q_ARG(QVariant, 180), Q_ARG(QVariant, 115),
                                  Q_ARG(QVariant, QStringLiteral("pixelate")));

        rects = w->property("result").toMap().value(QStringLiteral("rects")).toList();
        QCOMPARE(rects.size(), 3);
        QCOMPARE(rects.at(0).toMap().value(QStringLiteral("style")).toString(), QStringLiteral("black"));
        QCOMPARE(rects.at(1).toMap().value(QStringLiteral("style")).toString(), QStringLiteral("blur"));
        QCOMPARE(rects.at(1).toMap().value(QStringLiteral("x")).toInt(), 84);
        QCOMPARE(rects.at(2).toMap().value(QStringLiteral("style")).toString(), QStringLiteral("pixelate"));
        QCOMPARE(rects.at(2).toMap().value(QStringLiteral("w")).toInt(), 180);

        saveEditorScreenshots(app, w, QStringLiteral("redactImage"));

        // Select first rect, change its style to blur, then remove second rect
        QMetaObject::invokeMethod(editor, "selectRect", Q_ARG(QVariant, 0));
        QMetaObject::invokeMethod(editor, "setSelectedStyle", Q_ARG(QVariant, QStringLiteral("blur")));
        QMetaObject::invokeMethod(editor, "selectRect", Q_ARG(QVariant, 1));
        QMetaObject::invokeMethod(editor, "removeSelectedRect");

        rects = w->property("result").toMap().value(QStringLiteral("rects")).toList();
        QCOMPARE(rects.size(), 2);
        QCOMPARE(rects.at(0).toMap().value(QStringLiteral("style")).toString(), QStringLiteral("blur"));
        QCOMPARE(rects.at(1).toMap().value(QStringLiteral("style")).toString(), QStringLiteral("pixelate"));

        w->hide();
        QVERIFY2(g_messages.isEmpty(), qPrintable(g_messages.join(QLatin1Char('\n'))));
    }

    void trimVideoAndAudioTimeline()
    {
        g_messages.clear();
        QQuickWindow *w = openEditor(QStringLiteral("trimVideo"), QStringLiteral("Trim Video"), videoPath);
        QVERIFY(w);

        auto *timeline = w->findChild<QObject *>(QStringLiteral("timelineEditor"));
        QVERIFY(timeline);

        // Wait for inspect (duration 12.5s) and waveform + frame PNGs from fake-nashi-cli
        QTRY_VERIFY_WITH_TIMEOUT(w->property("duration").toDouble() > 12.0, 10000);
        QTRY_VERIFY_WITH_TIMEOUT(!timeline->property("waveformUrl").toString().isEmpty(), 10000);
        QTRY_VERIFY_WITH_TIMEOUT(!timeline->property("frameUrl").toString().isEmpty(), 10000);

        // Scrub playhead and set In / Out markers
        QMetaObject::invokeMethod(timeline, "seekTo", Q_ARG(QVariant, 2.25));
        QMetaObject::invokeMethod(timeline, "setInAtPlayhead");
        QMetaObject::invokeMethod(timeline, "seekTo", Q_ARG(QVariant, 9.5));
        QMetaObject::invokeMethod(timeline, "setOutAtPlayhead");
        QMetaObject::invokeMethod(timeline, "seekTo", Q_ARG(QVariant, 5.2));

        QVariantMap res = w->property("result").toMap();
        QCOMPARE(res.value(QStringLiteral("start")).toDouble(), 2.25);
        QCOMPARE(res.value(QStringLiteral("end")).toDouble(), 9.5);

        auto *startField = w->findChild<QObject *>(QStringLiteral("trimStartField"));
        auto *endField = w->findChild<QObject *>(QStringLiteral("trimEndField"));
        QVERIFY(startField && endField);
        QCOMPARE(startField->property("text").toString(), QStringLiteral("0:02.25"));
        QCOMPARE(endField->property("text").toString(), QStringLiteral("0:09.50"));

        saveEditorScreenshots(app, w, QStringLiteral("trimVideo"));

        // Also verify redactAudio multi-range mute addition at playhead
        app->showOptions(QStringLiteral("redactAudio"), QStringLiteral("Redact Audio"), {audioPath});
        QTRY_VERIFY_WITH_TIMEOUT(w->property("duration").toDouble() > 12.0, 10000);
        QTRY_VERIFY_WITH_TIMEOUT(!timeline->property("waveformUrl").toString().isEmpty(), 10000);
        QMetaObject::invokeMethod(timeline, "seekTo", Q_ARG(QVariant, 4.5));
        QMetaObject::invokeMethod(timeline, "addRangeAtPlayhead");

        const QVariantList ranges = w->property("result").toMap().value(QStringLiteral("ranges")).toList();
        QCOMPARE(ranges.size(), 2);
        QCOMPARE(ranges.at(1).toList().at(0).toDouble(), 4.5);
        QCOMPARE(ranges.at(1).toList().at(1).toDouble(), 5.5);

        // Muting is the default; bleeping is an explicit choice.
        QVERIFY(!w->property("result").toMap().contains(QStringLiteral("audio_mode")));
        w->setProperty("audioMode", QStringLiteral("bleep"));
        QCOMPARE(w->property("result").toMap().value(QStringLiteral("audio_mode")).toString(), QStringLiteral("bleep"));

        // Silence removal is offered for trimming audio only.
        auto *silence = w->findChild<QObject *>(QStringLiteral("trimSilenceSwitch"));
        QVERIFY(silence);
        app->showOptions(QStringLiteral("trimVideo"), QStringLiteral("Trim Video"), {videoPath});
        QVERIFY(!silence->property("visible").toBool());
        app->showOptions(QStringLiteral("trimAudio"), QStringLiteral("Trim Audio"), {audioPath});
        QTRY_VERIFY_WITH_TIMEOUT(w->property("duration").toDouble() > 12.0, 10000);
        QVERIFY(silence->property("visible").toBool());
        QVERIFY(!w->property("result").toMap().contains(QStringLiteral("trim_silence")));
        silence->setProperty("checked", true);
        QCOMPARE(w->property("result").toMap().value(QStringLiteral("trim_silence")).toBool(), true);

        // A choice made for one file must not carry over to the next.
        app->showOptions(QStringLiteral("trimAudio"), QStringLiteral("Trim Audio"), {audioPath});
        QVERIFY(!silence->property("checked").toBool());
        QVERIFY(!w->property("result").toMap().contains(QStringLiteral("trim_silence")));
        QCOMPARE(w->property("audioMode").toString(), QStringLiteral("mute"));

        w->hide();
        QVERIFY2(g_messages.isEmpty(), qPrintable(g_messages.join(QLatin1Char('\n'))));
    }

    void compressFormBuildsOptionsForEachKindOfFile()
    {
        g_messages.clear();
        QVERIFY(WheelModel::hasOptionsForm(QStringLiteral("compress")));
        QQuickWindow *w = openEditor(QStringLiteral("compress"), QStringLiteral("Compress"), photoPath);
        QVERIFY(w);
        const auto result = [w] { return w->property("result").toMap(); };
        const auto imageSection = w->findChild<QObject *>(QStringLiteral("compressImageSize"));
        const auto videoSection = w->findChild<QObject *>(QStringLiteral("compressVideoSize"));
        QVERIFY(imageSection && videoSection);
        QTRY_VERIFY_WITH_TIMEOUT(w->property("mediaWidth").toInt() > 0, 10000);
        QVERIFY(imageSection->property("visible").toBool());
        QVERIFY(!videoSection->property("visible").toBool());

        // Balanced by default, with no size limit.
        QCOMPARE(result().value(QStringLiteral("quality")).toInt(), 80);
        QCOMPARE(result().value(QStringLiteral("crf")).toInt(), 26);
        QCOMPARE(result().value(QStringLiteral("audio_kbps")).toInt(), 128);
        QCOMPARE(result().value(QStringLiteral("pdf_preset")).toString(), QStringLiteral("ebook"));
        QVERIFY(!result().contains(QStringLiteral("target_bytes")));
        QVERIFY(!result().contains(QStringLiteral("max_edge")));

        w->setProperty("compressPreset", QStringLiteral("strong"));
        QCOMPARE(result().value(QStringLiteral("quality")).toInt(), 45);
        QCOMPARE(result().value(QStringLiteral("crf")).toInt(), 32);
        QCOMPARE(result().value(QStringLiteral("audio_kbps")).toInt(), 64);
        QCOMPARE(result().value(QStringLiteral("pdf_preset")).toString(), QStringLiteral("screen"));

        w->setProperty("compressImageSizeChoice", QStringLiteral("1920"));
        QCOMPARE(result().value(QStringLiteral("max_edge")).toInt(), 1920);
        w->setProperty("compressImageSizeChoice", QStringLiteral("50%"));
        QVERIFY(!result().contains(QStringLiteral("max_edge")));
        QCOMPARE(result().value(QStringLiteral("image_scale")).toDouble(), 0.5);
        w->setProperty("compressImageSizeChoice", QStringLiteral("custom"));
        w->setProperty("compressCustomWidth", QStringLiteral("abc"));
        QCOMPARE(w->property("result").typeId(), QMetaType::QString);
        w->setProperty("compressCustomWidth", QStringLiteral("640"));
        QCOMPARE(result().value(QStringLiteral("resize")).toMap().value(QStringLiteral("w")).toInt(), 640);
        w->setProperty("compressImageSizeChoice", QStringLiteral("original"));

        w->setProperty("compressLimit", QStringLiteral("size"));
        QCOMPARE(w->property("result").typeId(), QMetaType::QString); // nothing entered yet
        w->setProperty("compressLimitValue", QStringLiteral("2"));
        w->setProperty("compressLimitUnit", QStringLiteral("MB"));
        QCOMPARE(result().value(QStringLiteral("target_bytes")).toLongLong(), 2097152);
        w->setProperty("compressLimitUnit", QStringLiteral("KB"));
        QCOMPARE(result().value(QStringLiteral("target_bytes")).toLongLong(), 2048);
        w->setProperty("compressLimit", QStringLiteral("percent"));
        w->setProperty("compressLimitValue", QStringLiteral("40"));
        QVERIFY(!result().contains(QStringLiteral("target_bytes")));
        QCOMPARE(result().value(QStringLiteral("percentage")).toDouble(), 40.0);
        w->setProperty("compressLimitValue", QStringLiteral("100"));
        QCOMPARE(w->property("result").typeId(), QMetaType::QString);

        // Video offers a height instead of an image size, and nothing carries over.
        app->showOptions(QStringLiteral("compress"), QStringLiteral("Compress"), {videoPath});
        QTRY_VERIFY_WITH_TIMEOUT(w->property("duration").toDouble() > 12.0, 10000);
        QVERIFY(!imageSection->property("visible").toBool());
        QVERIFY(videoSection->property("visible").toBool());
        QCOMPARE(result().value(QStringLiteral("quality")).toInt(), 80);
        QVERIFY(!result().contains(QStringLiteral("percentage")));
        w->setProperty("compressLimit", QStringLiteral("size"));
        w->setProperty("compressLimitValue", QStringLiteral("25"));
        saveEditorScreenshots(app, w, QStringLiteral("compress"));
        w->setProperty("compressLimit", QStringLiteral("none"));
        w->setProperty("compressVideoHeight", 720);
        QCOMPARE(result().value(QStringLiteral("max_height")).toInt(), 720);

        // PDFs and audio have neither.
        app->showOptions(QStringLiteral("compress"), QStringLiteral("Compress"), {pdfPath});
        QTRY_VERIFY_WITH_TIMEOUT(w->property("pageCount").toInt() > 0, 10000);
        QVERIFY(!imageSection->property("visible").toBool());
        QVERIFY(!videoSection->property("visible").toBool());
        QVERIFY(!result().contains(QStringLiteral("max_height")));

        w->hide();
        QVERIFY2(g_messages.isEmpty(), qPrintable(g_messages.join(QLatin1Char('\n'))));
    }

    void splitVideoFormChoosesClips()
    {
        g_messages.clear();
        QVERIFY(WheelModel::hasOptionsForm(QStringLiteral("splitVideo")));
        QQuickWindow *w = openEditor(QStringLiteral("splitVideo"), QStringLiteral("Split Video"), videoPath);
        QVERIFY(w);
        auto *timeline = w->findChild<QObject *>(QStringLiteral("timelineEditor"));
        QVERIFY(timeline);
        QTRY_VERIFY_WITH_TIMEOUT(w->property("duration").toDouble() > 12.0, 10000);
        QCOMPARE(timeline->property("mode").toString(), QStringLiteral("points"));
        const auto result = [w] { return w->property("result"); };
        const auto map = [w] { return w->property("result").toMap(); };
        QCOMPARE(result().typeId(), QMetaType::QString); // no cuts yet

        // Points stay sorted; ones too close together or at the ends are ignored.
        for (const double t : {6.0, 2.0, 2.01, 0.0, 12.5}) {
            QMetaObject::invokeMethod(timeline, "seekTo", Q_ARG(QVariant, t));
            QMetaObject::invokeMethod(w, "addSplitAtPlayhead");
        }
        const QVariantList points = map().value(QStringLiteral("split_points")).toList();
        QCOMPARE(points.size(), 2);
        QCOMPARE(points.at(0).toDouble(), 2.0);
        QCOMPARE(points.at(1).toDouble(), 6.0);
        QCOMPARE(timeline->property("markers").toList().size(), 2);
        QCOMPARE(w->property("clips").toList().size(), 3);

        // I and O must not edit a hidden range here.
        const auto hiddenRanges = [timeline] {
            return timeline->property("ranges").value<QJSValue>().toVariant();
        };
        const QVariant rangesBefore = hiddenRanges();
        QMetaObject::invokeMethod(timeline, "setInAtPlayhead");
        QMetaObject::invokeMethod(timeline, "setOutAtPlayhead");
        QCOMPARE(hiddenRanges(), rangesBefore);

        QMetaObject::invokeMethod(w, "toggleClip", Q_ARG(QVariant, 1));
        QCOMPARE(map().value(QStringLiteral("discard_clips")).toList().at(0).toInt(), 1);
        QMetaObject::invokeMethod(w, "toggleClip", Q_ARG(QVariant, 0));
        QMetaObject::invokeMethod(w, "toggleClip", Q_ARG(QVariant, 2));
        QCOMPARE(result().typeId(), QMetaType::QString); // nothing kept
        QMetaObject::invokeMethod(w, "toggleClip", Q_ARG(QVariant, 2));

        w->setProperty("joinClips", true);
        QCOMPARE(map().value(QStringLiteral("join_clips")).toBool(), true);
        saveEditorScreenshots(app, w, QStringLiteral("splitVideo"));

        // Changing the cuts renumbers the clips, so removals start over.
        QMetaObject::invokeMethod(w, "removeSplitPoint", Q_ARG(QVariant, 0));
        QVERIFY(!map().contains(QStringLiteral("discard_clips")));
        QCOMPARE(map().value(QStringLiteral("split_points")).toList().size(), 1);
        QMetaObject::invokeMethod(w, "setEqualClips", Q_ARG(QVariant, 5));
        QCOMPARE(map().value(QStringLiteral("split_points")).toList().size(), 4);
        QCOMPARE(map().value(QStringLiteral("split_points")).toList().at(0).toDouble(), 2.5);

        // Nothing carries over to the next file.
        app->showOptions(QStringLiteral("splitVideo"), QStringLiteral("Split Video"), {videoPath});
        QCOMPARE(result().typeId(), QMetaType::QString);
        QVERIFY(!w->property("joinClips").toBool());
        w->hide();
        QVERIFY2(g_messages.isEmpty(), qPrintable(g_messages.join(QLatin1Char('\n'))));
    }

    void videoSnapshotsFormChoosesFrames()
    {
        g_messages.clear();
        QVERIFY(WheelModel::hasOptionsForm(QStringLiteral("videoSnapshots")));
        QQuickWindow *w = openEditor(QStringLiteral("videoSnapshots"), QStringLiteral("Video Snapshots"), videoPath);
        QVERIFY(w);
        auto *timeline = w->findChild<QObject *>(QStringLiteral("timelineEditor"));
        QVERIFY(timeline);
        QTRY_VERIFY_WITH_TIMEOUT(w->property("duration").toDouble() > 12.0, 10000);
        const auto map = [w] { return w->property("result").toMap(); };
        QCOMPARE(w->property("result").typeId(), QMetaType::QString);

        for (const double t : {3.0, 1.5, 3.0}) {
            QMetaObject::invokeMethod(timeline, "seekTo", Q_ARG(QVariant, t));
            QMetaObject::invokeMethod(w, "addSnapshotAtPlayhead");
        }
        // The end of the video is a valid frame, and a repeat is not added twice.
        const QVariantList times = map().value(QStringLiteral("snapshot_times")).toList();
        QCOMPARE(times.size(), 2);
        QCOMPARE(times.at(0).toDouble(), 1.5);
        QMetaObject::invokeMethod(timeline, "stepFrame", Q_ARG(QVariant, 1));
        QVERIFY(timeline->property("playhead").toDouble() > 3.0);
        QMetaObject::invokeMethod(w, "removeSnapshot", Q_ARG(QVariant, 0));
        QCOMPARE(map().value(QStringLiteral("snapshot_times")).toList().size(), 1);

        w->setProperty("snapshotMode", QStringLiteral("interval"));
        w->setProperty("snapshotEvery", QStringLiteral("2"));
        QVERIFY(!map().contains(QStringLiteral("snapshot_times")));
        QCOMPARE(map().value(QStringLiteral("fps")).toDouble(), 0.5);
        w->setProperty("snapshotEvery", QStringLiteral("0"));
        QCOMPARE(w->property("result").typeId(), QMetaType::QString);
        w->hide();
        QVERIFY2(g_messages.isEmpty(), qPrintable(g_messages.join(QLatin1Char('\n'))));
    }

    void organizePdfReorderRotateDuplicateDelete()
    {
        g_messages.clear();
        QQuickWindow *w = openEditor(QStringLiteral("organizePDF"), QStringLiteral("Organize PDF"), pdfPath);
        QVERIFY(w);

        auto *pdf = w->findChild<QObject *>(QStringLiteral("pdfOrganizer"));
        QVERIFY(pdf);

        // Wait for inspect (4 pages) and page thumbnail PNGs from fake-nashi-cli
        QTRY_COMPARE_WITH_TIMEOUT(w->property("pageCount").toInt(), 4, 10000);
        QTRY_VERIFY_WITH_TIMEOUT(pdf->property("thumbFiles").toList().size() >= 3, 10000);

        // Initial state: 1,2,3,4 with all rotations 0 -> `rotations` key omitted
        QVariantMap res = w->property("result").toMap();
        QCOMPARE(res.value(QStringLiteral("pages")).toString(), QStringLiteral("1,2,3,4"));
        QVERIFY(!res.contains(QStringLiteral("rotations")));

        // Reorder page #3 (index 2) to front (index 0) -> "3,1,2,4", still no rotations
        QMetaObject::invokeMethod(pdf, "movePage", Q_ARG(QVariant, 2), Q_ARG(QVariant, 0));
        res = w->property("result").toMap();
        QCOMPARE(res.value(QStringLiteral("pages")).toString(), QStringLiteral("3,1,2,4"));
        QVERIFY(!res.contains(QStringLiteral("rotations")));

        // Rotate second card (#1) by 90° CW -> `rotations` array included
        QMetaObject::invokeMethod(pdf, "rotatePage", Q_ARG(QVariant, 1));
        res = w->property("result").toMap();
        QCOMPARE(res.value(QStringLiteral("pages")).toString(), QStringLiteral("3,1,2,4"));
        QCOMPARE(res.value(QStringLiteral("rotations")).toList(), (QVariantList{0, 90, 0, 0}));

        // Duplicate third card (#2) and delete last card (#4) -> "3,1,2,2" with [0, 90, 0, 0]
        QMetaObject::invokeMethod(pdf, "duplicatePage", Q_ARG(QVariant, 2));
        QMetaObject::invokeMethod(pdf, "deletePage", Q_ARG(QVariant, 4));
        res = w->property("result").toMap();
        QCOMPARE(res.value(QStringLiteral("pages")).toString(), QStringLiteral("3,1,2,2"));
        QCOMPARE(res.value(QStringLiteral("rotations")).toList(), (QVariantList{0, 90, 0, 0}));

        auto *pagesField = w->findChild<QObject *>(QStringLiteral("pdfPagesField"));
        QVERIFY(pagesField);
        QCOMPARE(pagesField->property("text").toString(), QStringLiteral("3,1,2,2"));

        saveEditorScreenshots(app, w, QStringLiteral("organizePDF"));

        // Reset restores 1..4 with 0° rotation and omits `rotations`
        QMetaObject::invokeMethod(pdf, "resetPages");
        res = w->property("result").toMap();
        QCOMPARE(res.value(QStringLiteral("pages")).toString(), QStringLiteral("1,2,3,4"));
        QVERIFY(!res.contains(QStringLiteral("rotations")));

        // Manual text edit syncs cards
        pagesField->setProperty("text", QStringLiteral("4,2"));
        res = w->property("result").toMap();
        QCOMPARE(res.value(QStringLiteral("pages")).toString(), QStringLiteral("4,2"));
        QCOMPARE(pdf->property("items").toList().size(), 2);

        w->hide();
        QVERIFY2(g_messages.isEmpty(), qPrintable(g_messages.join(QLatin1Char('\n'))));
    }

    void annotateImageExportAndRun()
    {
        g_messages.clear();
        QQuickWindow *w = openEditor(QStringLiteral("annotateImage"), QStringLiteral("Annotate Image"), photoPath);
        QVERIFY(w);

        auto *annotate = w->findChild<QObject *>(QStringLiteral("annotateEditor"));
        QVERIFY(annotate);

        QTRY_COMPARE_WITH_TIMEOUT(w->property("mediaWidth").toInt(), 640, 10000);
        QCOMPARE(w->property("mediaHeight").toInt(), 360);
        QTRY_VERIFY_WITH_TIMEOUT(!annotate->property("resolvedImageUrl").toString().isEmpty(), 10000);

        // Empty canvas is invalid until an annotation is added
        QCOMPARE(w->property("result").typeId(), QMetaType::QString);

        // Select Text tool, focus text input, and verify pressing Return does NOT close window
        annotate->setProperty("currentTool", QStringLiteral("text"));
        QTest::qWait(30);
        auto *textInput = w->findChild<QQuickItem *>(QStringLiteral("annotateTextInput"));
        QVERIFY(textInput);
        textInput->forceActiveFocus();
        textInput->setProperty("text", QStringLiteral("Key detail"));
        QTest::keyClick(w, Qt::Key_Return);
        QVERIFY(w->isVisible());
        QCOMPARE(annotate->property("items").toList().size(), 1); // Enter placed the text note!

        // Add freehand stroke + arrow programmatically
        annotate->setProperty("currentTool", QStringLiteral("arrow"));
        QVariantList strokePts;
        strokePts.append(QVariant(QVariantList{72, 66}));
        strokePts.append(QVariant(QVariantList{312, 66}));
        strokePts.append(QVariant(QVariantList{312, 206}));
        strokePts.append(QVariant(QVariantList{72, 206}));
        strokePts.append(QVariant(QVariantList{72, 66}));
        QMetaObject::invokeMethod(annotate, "addStroke", Q_ARG(QVariant, strokePts),
                                  Q_ARG(QVariant, QStringLiteral("#2f9e44")), Q_ARG(QVariant, 6));
        QMetaObject::invokeMethod(annotate, "addArrow", Q_ARG(QVariant, 440), Q_ARG(QVariant, 250),
                                  Q_ARG(QVariant, 316), Q_ARG(QVariant, 155),
                                  Q_ARG(QVariant, QStringLiteral("#e03131")), Q_ARG(QVariant, 7));

        QCOMPARE(annotate->property("items").toList().size(), 3);
        QTest::qWait(60);

        // Test Undo / Redo round-trip
        QMetaObject::invokeMethod(annotate, "undo");
        QCOMPARE(annotate->property("items").toList().size(), 2);
        QMetaObject::invokeMethod(annotate, "redo");
        QCOMPARE(annotate->property("items").toList().size(), 3);
        QTest::qWait(60);

        saveEditorScreenshots(app, w, QStringLiteral("annotateImage"));

        // Export transparent overlay PNG directly and inspect pixels
        QVariant exportedVar;
        QVERIFY(QMetaObject::invokeMethod(annotate, "exportOverlay", Q_RETURN_ARG(QVariant, exportedVar)));
        const QString overlayPath = exportedVar.toString();
        QVERIFY2(!overlayPath.isEmpty(), "exportOverlay returned an empty path");
        const QFileInfo overlayInfo(overlayPath);
        QVERIFY(overlayInfo.exists());
        QVERIFY(overlayInfo.size() > 100);

        const QImage overlayImg(overlayPath);
        QVERIFY(!overlayImg.isNull());
        QCOMPARE(overlayImg.width(), 640);
        QCOMPARE(overlayImg.height(), 360);
        QVERIFY(overlayImg.hasAlphaChannel());
        // Corner pixel (2, 2) must be completely transparent (background photo NOT included!)
        QCOMPARE(qAlpha(overlayImg.pixel(2, 2)), 0);
        // Stroke pixel at (180, 66) must be non-transparent
        QVERIFY(qAlpha(overlayImg.pixel(180, 66)) > 0);

        // Result now reflects {"overlay": overlayPath}
        const QVariantMap res = w->property("result").toMap();
        QCOMPARE(res.value(QStringLiteral("overlay")).toString(), overlayPath);

        // Clean up the manual test export before exercising win.run()
        app->preview()->removeAnnotationFile(overlayPath);

        // Invoking run() exports a fresh overlay, launches the job, closes the window,
        // and App::startJob deletes the temporary overlay PNG once the job finishes.
        const int jobsBefore = app->jobs()->rowCount();
        QMetaObject::invokeMethod(w, "run");
        QCOMPARE(app->jobs()->rowCount(), jobsBefore + 1);
        QVERIFY(!w->isVisible());
        const QString runOverlay = annotate->property("lastExportPath").toString();
        QVERIFY(!runOverlay.isEmpty());
        QTRY_VERIFY_WITH_TIMEOUT(app->jobs()->activeCount() == 0, 10000);
        QTRY_VERIFY_WITH_TIMEOUT(!QFileInfo::exists(runOverlay), 5000);

        QVERIFY2(g_messages.isEmpty(), qPrintable(g_messages.join(QLatin1Char('\n'))));
    }
};

QTEST_MAIN(TestEditors)
#include "tst_editors.moc"
