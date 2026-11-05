// Every options form against the real nashi-cli: what a form builds must be what
// the engine accepts, and the output must show the choice. Other UI tests use the
// fake CLI, which accepts anything. Skipped when nashi-cli is not built, and per
// case when an external tool is missing.
#include "app.h"
#include "jobmanager.h"
#include "jobrunner.h"
#include "settings.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QImage>
#include <QProcess>
#include <QQmlEngine>
#include <QQuickWindow>
#include <QStandardPaths>
#include <QTemporaryDir>
#include <QTest>

#include <functional>

namespace {

bool have(const char *tool)
{
    return !QStandardPaths::findExecutable(QString::fromLatin1(tool)).isEmpty();
}

QString output(const QString &program, const QStringList &args)
{
    QProcess p;
    p.start(program, args);
    if (!p.waitForFinished(60000))
        return {};
    return QString::fromUtf8(p.readAllStandardOutput()).trimmed();
}

bool make(const QString &program, const QStringList &args)
{
    QProcess p;
    p.setProcessChannelMode(QProcess::MergedChannels);
    p.start(program, args);
    if (!p.waitForFinished(60000) || p.exitCode() != 0) {
        qWarning().noquote() << program << args << p.readAll();
        return false;
    }
    return true;
}

double seconds(const QString &file)
{
    return output(QStringLiteral("ffprobe"), {QStringLiteral("-v"), QStringLiteral("error"), QStringLiteral("-show_entries"),
                                              QStringLiteral("format=duration"), QStringLiteral("-of"),
                                              QStringLiteral("csv=p=0"), file})
        .toDouble();
}

QString videoSize(const QString &file)
{
    return output(QStringLiteral("ffprobe"),
                  {QStringLiteral("-v"), QStringLiteral("error"), QStringLiteral("-select_streams"), QStringLiteral("v:0"),
                   QStringLiteral("-show_entries"), QStringLiteral("stream=width,height"), QStringLiteral("-of"),
                   QStringLiteral("csv=p=0"), file});
}

// Lines ffmpeg's measuring filters print, which go to stderr.
QString measured(const QStringList &args)
{
    QProcess p;
    p.start(QStringLiteral("ffmpeg"), QStringList{QStringLiteral("-hide_banner"), QStringLiteral("-nostats")} + args);
    if (!p.waitForFinished(60000))
        return {};
    return QString::fromUtf8(p.readAllStandardError());
}

double lufs(const QString &file)
{
    const QString report = measured({QStringLiteral("-i"), file, QStringLiteral("-af"), QStringLiteral("ebur128=framelog=quiet"),
                                     QStringLiteral("-f"), QStringLiteral("null"), QStringLiteral("-")});
    const QStringList lines = report.split(QLatin1Char('\n'));
    for (auto it = lines.crbegin(); it != lines.crend(); ++it) {
        const QString line = it->trimmed();
        if (line.startsWith(QLatin1String("I:")))
            return line.mid(2).trimmed().split(QLatin1Char(' ')).first().toDouble();
    }
    return 0;
}

// Mean volume in dB of a part of a file.
double meanVolume(const QString &file, double start, double length)
{
    const QString report = measured({QStringLiteral("-ss"), QString::number(start), QStringLiteral("-t"), QString::number(length),
                                     QStringLiteral("-i"), file, QStringLiteral("-af"), QStringLiteral("volumedetect"),
                                     QStringLiteral("-f"), QStringLiteral("null"), QStringLiteral("-")});
    const qsizetype at = report.indexOf(QLatin1String("mean_volume:"));
    if (at < 0)
        return 0;
    return report.mid(at + 12).trimmed().split(QLatin1Char(' ')).first().toDouble();
}

double audioKbps(const QString &file)
{
    return output(QStringLiteral("ffprobe"),
                  {QStringLiteral("-v"), QStringLiteral("error"), QStringLiteral("-show_entries"), QStringLiteral("format=bit_rate"),
                   QStringLiteral("-of"), QStringLiteral("csv=p=0"), file})
               .toDouble()
           / 1000.0;
}

// Width of each PDF page in points.
QList<double> pageWidths(const QString &file)
{
    QList<double> widths;
    const QString info = output(QStringLiteral("pdfinfo"), {QStringLiteral("-f"), QStringLiteral("1"), QStringLiteral("-l"), QStringLiteral("999"), file});
    for (const QString &line : info.split(QLatin1Char('\n')))
        if (line.startsWith(QLatin1String("Page ")) && line.contains(QLatin1String(" size:")))
            widths.append(line.section(QLatin1Char(':'), 1).trimmed().section(QLatin1Char(' '), 0, 0).toDouble());
    return widths;
}

int pdfPages(const QString &file)
{
    const QString info = output(QStringLiteral("pdfinfo"), {file});
    for (const QString &line : info.split(QLatin1Char('\n')))
        if (line.startsWith(QLatin1String("Pages:")))
            return line.mid(6).trimmed().toInt();
    return -1;
}

} // namespace

#define NEED(tool)                                                                                                     \
    if (!have(tool))                                                                                                   \
    QSKIP(tool " is not installed")

class TestFormsEngine : public QObject
{
    Q_OBJECT

    App *app = nullptr;
    QQmlEngine *engine = nullptr;
    QTemporaryDir dir;

    QString at(const char *name) const { return dir.filePath(QString::fromUtf8(name)); }

    using Setup = std::function<void(QQuickWindow *)>;

    // Open the form, let `setup` fill it, press its run button and wait for the job.
    bool runForm(const QString &tool, const QStringList &files, const Setup &setup, QStringList *outputs)
    {
        app->showOptions(tool, tool, files);
        auto *w = qobject_cast<QQuickWindow *>(app->optionsWindow());
        if (!w) {
            qWarning() << "no options window for" << tool;
            return false;
        }
        w->show();
        if (!QTest::qWaitForWindowExposed(w))
            return false;
        if (setup)
            setup(w);
        const QVariant result = w->property("result");
        if (result.typeId() == QMetaType::QString) {
            qWarning().noquote() << tool << "form refuses:" << result.toString();
            w->close();
            return false;
        }
        bool done = false;
        JobRunner::State state = JobRunner::Idle;
        QString errors;
        const auto connection = connect(app->jobs(), &JobManager::jobFinished, this, [&](JobRunner *job) {
            state = job->state();
            *outputs = job->outputs();
            errors = job->errorSummary();
            done = true;
        });
        QMetaObject::invokeMethod(w, "run");
        const bool finished = QTest::qWaitFor([&] { return done; }, 120000);
        disconnect(connection);
        if (!finished || state != JobRunner::Succeeded) {
            qWarning().noquote() << tool << "failed:" << errors;
            return false;
        }
        for (const QString &o : std::as_const(*outputs)) {
            if (!QFileInfo::exists(o)) {
                qWarning().noquote() << tool << "listed a missing output:" << o;
                return false;
            }
        }
        return !outputs->isEmpty();
    }

    // The options the app would send for a wheel conversion, through the real engine.
    bool convert(const QString &target, const QString &file, QStringList *outputs)
    {
        bool done = false;
        JobRunner::State state = JobRunner::Idle;
        QString errors;
        const auto connection = connect(app->jobs(), &JobManager::jobFinished, this, [&](JobRunner *job) {
            state = job->state();
            *outputs = job->outputs();
            errors = job->errorSummary();
            done = true;
        });
        app->runItem(QStringLiteral("convert"), target, target, {file});
        const bool finished = QTest::qWaitFor([&] { return done; }, 120000);
        disconnect(connection);
        if (!finished || state != JobRunner::Succeeded)
            qWarning().noquote() << "convert to" << target << "failed:" << errors;
        return finished && state == JobRunner::Succeeded && !outputs->isEmpty();
    }

private Q_SLOTS:
    void initTestCase()
    {
        // Outside ctest this would use the real settings and library.
        if (qEnvironmentVariableIsEmpty("NASHI_CLI") || qEnvironmentVariableIsEmpty("XDG_CONFIG_HOME"))
            QFAIL("Run this test through ctest");
        if (!QFileInfo(qEnvironmentVariable("NASHI_CLI")).isExecutable())
            QSKIP("nashi-cli is not built; run cargo build first");
        QVERIFY(dir.isValid());
        app = new App(App::Options{false, false}, this);
        app->finishWelcome();
        engine = new QQmlEngine(this);
        app->init(engine);

        if (have("magick")) {
            QVERIFY(make(QStringLiteral("magick"), {QStringLiteral("-size"), QStringLiteral("640x360"), QStringLiteral("gradient:red-blue"), at("photo.png")}));
            QVERIFY(make(QStringLiteral("magick"), {QStringLiteral("-size"), QStringLiteral("640x360"), QStringLiteral("gradient:green-yellow"), at("second.png")}));
            QVERIFY(make(QStringLiteral("magick"), {QStringLiteral("-size"), QStringLiteral("600x300"), QStringLiteral("xc:white"), QStringLiteral("xc:white"),
                                                    QStringLiteral("-size"), QStringLiteral("200x400"), QStringLiteral("xc:white"), QStringLiteral("xc:white"),
                                                    QStringLiteral("xc:white"), QStringLiteral("-units"), QStringLiteral("PixelsPerInch"),
                                                    QStringLiteral("-density"), QStringLiteral("72"), at("book.pdf")}));
            QVERIFY(QFile::copy(at("book.pdf"), at("book2.pdf")));
        }
        if (have("ffmpeg")) {
            QVERIFY(make(QStringLiteral("ffmpeg"), {QStringLiteral("-v"), QStringLiteral("error"), QStringLiteral("-f"), QStringLiteral("lavfi"), QStringLiteral("-i"), QStringLiteral("testsrc=d=6:s=640x360:r=25"),
                                                    QStringLiteral("-f"), QStringLiteral("lavfi"), QStringLiteral("-i"), QStringLiteral("sine=d=6"), QStringLiteral("-shortest"),
                                                    QStringLiteral("-c:v"), QStringLiteral("libx264"), QStringLiteral("-pix_fmt"), QStringLiteral("yuv420p"), QStringLiteral("-c:a"), QStringLiteral("aac"),
                                                    QStringLiteral("-metadata"), QStringLiteral("title=Secret"), at("clip.mp4")}));
            QVERIFY(make(QStringLiteral("ffmpeg"), {QStringLiteral("-v"), QStringLiteral("error"), QStringLiteral("-f"), QStringLiteral("lavfi"), QStringLiteral("-i"), QStringLiteral("sine=d=6"),
                                                    QStringLiteral("-metadata"), QStringLiteral("title=Secret"), at("tone.mp3")}));
        }
    }

    void cleanupTestCase()
    {
        delete engine;
        delete app;
    }

    void speedChangesTheLength()
    {
        NEED("ffmpeg");
        QStringList out;
        QVERIFY(runForm(QStringLiteral("changeVideoSpeed"), {at("clip.mp4")}, [](QQuickWindow *w) { w->setProperty("speed", 2.0); }, &out));
        QVERIFY2(qAbs(seconds(out.first()) - 3.0) < 0.5, qPrintable(out.first()));
    }

    void trimKeepsTheChosenPart()
    {
        NEED("ffmpeg");
        const auto setup = [](QQuickWindow *w) {
            QTRY_VERIFY_WITH_TIMEOUT(w->property("duration").toDouble() > 5.0, 15000);
            auto *timeline = w->findChild<QObject *>(QStringLiteral("timelineEditor"));
            QVERIFY(timeline);
            QMetaObject::invokeMethod(timeline, "seekTo", Q_ARG(QVariant, 1.0));
            QMetaObject::invokeMethod(timeline, "setInAtPlayhead");
            QMetaObject::invokeMethod(timeline, "seekTo", Q_ARG(QVariant, 3.0));
            QMetaObject::invokeMethod(timeline, "setOutAtPlayhead");
        };
        QStringList out;
        QVERIFY(runForm(QStringLiteral("trimVideo"), {at("clip.mp4")}, setup, &out));
        QVERIFY2(qAbs(seconds(out.first()) - 2.0) < 0.5, qPrintable(QString::number(seconds(out.first()))));
        QVERIFY(runForm(QStringLiteral("trimAudio"), {at("tone.mp3")}, setup, &out));
        QVERIFY2(qAbs(seconds(out.first()) - 2.0) < 0.5, qPrintable(QString::number(seconds(out.first()))));
    }

    void cropKeepsTheChosenArea()
    {
        NEED("magick");
        NEED("ffmpeg");
        const auto setup = [](QQuickWindow *w) {
            QTRY_VERIFY_WITH_TIMEOUT(w->property("mediaWidth").toInt() > 0, 15000);
            auto *editor = w->findChild<QObject *>(QStringLiteral("rectEditor"));
            QVERIFY(editor);
            QMetaObject::invokeMethod(editor, "setCropRect", Q_ARG(QVariant, 60), Q_ARG(QVariant, 40), Q_ARG(QVariant, 200), Q_ARG(QVariant, 100));
        };
        QStringList out;
        QVERIFY(runForm(QStringLiteral("cropImage"), {at("photo.png")}, setup, &out));
        QCOMPARE(QImage(out.first()).size(), QSize(200, 100));
        QVERIFY(runForm(QStringLiteral("cropVideo"), {at("clip.mp4")}, setup, &out));
        QCOMPARE(videoSize(out.first()), QStringLiteral("200,100"));
    }

    void redactCoversTheArea()
    {
        NEED("magick");
        NEED("ffmpeg");
        const auto setup = [](QQuickWindow *w) {
            QTRY_VERIFY_WITH_TIMEOUT(w->property("mediaWidth").toInt() > 0, 15000);
            auto *editor = w->findChild<QObject *>(QStringLiteral("rectEditor"));
            QVERIFY(editor);
            QMetaObject::invokeMethod(editor, "updateSelectedRect", Q_ARG(QVariant, 100), Q_ARG(QVariant, 100), Q_ARG(QVariant, 200), Q_ARG(QVariant, 100));
        };
        QStringList out;
        QVERIFY(runForm(QStringLiteral("redactImage"), {at("photo.png")}, setup, &out));
        QImage image(out.first());
        QCOMPARE(image.pixelColor(200, 150).rgb() & 0xffffff, 0u);
        QVERIFY((image.pixelColor(20, 20).rgb() & 0xffffff) != 0u);

        QVERIFY(runForm(QStringLiteral("redactVideo"), {at("clip.mp4")}, setup, &out));
        QVERIFY(make(QStringLiteral("ffmpeg"), {QStringLiteral("-v"), QStringLiteral("error"), QStringLiteral("-ss"), QStringLiteral("2"), QStringLiteral("-i"), out.first(),
                                                QStringLiteral("-frames:v"), QStringLiteral("1"), at("frame.png")}));
        const QColor covered = QImage(at("frame.png")).pixelColor(200, 150);
        QVERIFY2(covered.red() < 30 && covered.green() < 30 && covered.blue() < 30, qPrintable(covered.name()));
    }

    void redactAudioMutesTheRange()
    {
        NEED("ffmpeg");
        const auto setup = [](QQuickWindow *w) {
            QTRY_VERIFY_WITH_TIMEOUT(w->property("duration").toDouble() > 5.0, 15000);
            auto *timeline = w->findChild<QObject *>(QStringLiteral("timelineEditor"));
            QVERIFY(timeline);
            QMetaObject::invokeMethod(timeline, "seekTo", Q_ARG(QVariant, 2.0));
            QMetaObject::invokeMethod(timeline, "addRangeAtPlayhead");
        };
        QStringList out;
        QVERIFY(runForm(QStringLiteral("redactAudio"), {at("tone.mp3")}, setup, &out));
        QVERIFY(qAbs(seconds(out.first()) - seconds(at("tone.mp3"))) < 0.5);
        // The form's first range is 0..1 s, and the added one starts at 2 s.
        QVERIFY2(meanVolume(out.first(), 2.2, 0.5) < -60.0, qPrintable(QString::number(meanVolume(out.first(), 2.2, 0.5))));
        QVERIFY2(meanVolume(out.first(), 4.0, 0.5) > -40.0, qPrintable(QString::number(meanVolume(out.first(), 4.0, 0.5))));
    }

    void editFrameAndCollageWrite()
    {
        NEED("magick");
        QStringList out;
        QVERIFY(runForm(QStringLiteral("editImage"), {at("photo.png")}, [](QQuickWindow *w) {
            QMetaObject::invokeMethod(w, "setAdjustment", Q_ARG(QVariant, QStringLiteral("brightness")), Q_ARG(QVariant, 40));
            QMetaObject::invokeMethod(w, "setAdjustment", Q_ARG(QVariant, QStringLiteral("vignette")), Q_ARG(QVariant, 30));
        }, &out));
        QCOMPARE(QImage(out.first()).size(), QSize(640, 360));
        QVERIFY(QImage(out.first()).pixelColor(320, 180) != QImage(at("photo.png")).pixelColor(320, 180));

        QVERIFY(runForm(QStringLiteral("frameImage"), {at("photo.png")}, [](QQuickWindow *w) {
            w->setProperty("frameBackdrop", QStringLiteral("gradient"));
            w->setProperty("frameColor2", QStringLiteral("#000000"));
        }, &out));
        QCOMPARE(QImage(out.first()).size(), QSize(720, 440));

        QVERIFY(runForm(QStringLiteral("createCollage"), {at("photo.png"), at("second.png")}, [](QQuickWindow *w) {
            w->setProperty("collageLayout", QStringLiteral("featured"));
            w->setProperty("collageSpacing", 12);
        }, &out));
        QCOMPARE(QImage(out.first()).width(), 2048);
    }

    void annotationReachesTheImage()
    {
        NEED("magick");
        QStringList out;
        QVERIFY(runForm(QStringLiteral("annotateImage"), {at("photo.png")}, [](QQuickWindow *w) {
            QTRY_VERIFY_WITH_TIMEOUT(w->property("mediaWidth").toInt() > 0, 15000);
            auto *annotate = w->findChild<QObject *>(QStringLiteral("annotateEditor"));
            QVERIFY(annotate);
            QTRY_VERIFY_WITH_TIMEOUT(!annotate->property("resolvedImageUrl").toString().isEmpty(), 15000);
            QMetaObject::invokeMethod(annotate, "addNumberAt", Q_ARG(QVariant, 100.0), Q_ARG(QVariant, 100.0));
        }, &out));
        QVERIFY(QImage(out.first()) != QImage(at("photo.png")).convertToFormat(QImage(out.first()).format()));
        QCOMPARE(QImage(out.first()).size(), QSize(640, 360));
    }

    void compressWorksForEachKindOfFile()
    {
        NEED("magick");
        NEED("ffmpeg");
        NEED("gs");
        QVERIFY(make(QStringLiteral("magick"), {QStringLiteral("-size"), QStringLiteral("640x360"), QStringLiteral("plasma:"), at("noisy.png")}));
        const auto compressed = [this](const QString &file, const QString &preset, QStringList *out) {
            return runForm(QStringLiteral("compress"), {file}, [&](QQuickWindow *w) {
                QTRY_VERIFY_WITH_TIMEOUT(!w->property("fileCategory").toString().isEmpty(), 15000);
                w->setProperty("compressPreset", preset);
            }, out);
        };
        QStringList out;
        for (const QString &file : {at("noisy.png"), at("book.pdf")})
            QVERIFY2(compressed(file, QStringLiteral("strong"), &out), qPrintable(file));

        // Each kind of file gets its own part of what the form sends.
        QVERIFY(compressed(at("tone.mp3"), QStringLiteral("strong"), &out));
        QVERIFY2(qAbs(audioKbps(out.first()) - 64.0) < 20.0, qPrintable(QString::number(audioKbps(out.first()))));
        QVERIFY(compressed(at("clip.mp4"), QStringLiteral("strong"), &out));
        const qint64 strong = QFileInfo(out.first()).size();
        QVERIFY(compressed(at("clip.mp4"), QStringLiteral("balanced"), &out));
        QVERIFY(strong < QFileInfo(out.first()).size());

        QVERIFY(runForm(QStringLiteral("compress"), {at("clip.mp4")}, [](QQuickWindow *w) {
            QTRY_COMPARE_WITH_TIMEOUT(w->property("fileCategory").toString(), QStringLiteral("video"), 15000);
            w->setProperty("compressVideoHeight", 180);
        }, &out));
        QCOMPARE(videoSize(out.first()), QStringLiteral("320,180"));
    }

    void splitAndSnapshotsWriteThePieces()
    {
        NEED("ffmpeg");
        QStringList out;
        QVERIFY(runForm(QStringLiteral("splitVideo"), {at("clip.mp4")}, [](QQuickWindow *w) {
            QTRY_VERIFY_WITH_TIMEOUT(w->property("duration").toDouble() > 5.0, 15000);
            QMetaObject::invokeMethod(w, "setEqualClips", Q_ARG(QVariant, 3));
        }, &out));
        QCOMPARE(out.size(), 1);
        QCOMPARE(QDir(out.first()).entryList(QDir::Files).size(), 3);

        QVERIFY(runForm(QStringLiteral("videoSnapshots"), {at("clip.mp4")}, [](QQuickWindow *w) {
            QTRY_VERIFY_WITH_TIMEOUT(w->property("duration").toDouble() > 5.0, 15000);
            auto *timeline = w->findChild<QObject *>(QStringLiteral("timelineEditor"));
            QVERIFY(timeline);
            for (const double t : {1.0, 2.5}) {
                QMetaObject::invokeMethod(timeline, "seekTo", Q_ARG(QVariant, t));
                QMetaObject::invokeMethod(w, "addSnapshotAtPlayhead");
            }
        }, &out));
        QCOMPARE(out.size(), 2);
        QCOMPARE(QImage(out.first()).size(), QSize(640, 360));
    }

    void metadataFormRemovesAndEdits()
    {
        NEED("ffmpeg");
        const auto title = [](const QString &file) {
            return output(QStringLiteral("ffprobe"), {QStringLiteral("-v"), QStringLiteral("error"), QStringLiteral("-show_entries"),
                                                      QStringLiteral("format_tags=title"), QStringLiteral("-of"), QStringLiteral("csv=p=0"), file});
        };
        QCOMPARE(title(at("tone.mp3")), QStringLiteral("Secret"));
        QStringList out;
        QVERIFY(runForm(QStringLiteral("removeMetadata"), {at("tone.mp3")}, {}, &out));
        QCOMPARE(title(out.first()), QString());

        QVERIFY(runForm(QStringLiteral("removeMetadata"), {at("tone.mp3")}, [](QQuickWindow *w) {
            QTRY_VERIFY_WITH_TIMEOUT(!w->property("metadataFields").toList().isEmpty(), 15000);
            w->setProperty("metadataMode", QStringLiteral("edit"));
            QMetaObject::invokeMethod(w, "editMetadata", Q_ARG(QVariant, QStringLiteral("title")), Q_ARG(QVariant, QStringLiteral("Public")));
        }, &out));
        QCOMPARE(title(out.first()), QStringLiteral("Public"));
    }

    void normalizeReachesTheEngine()
    {
        NEED("ffmpeg");
        QStringList out;
        QVERIFY(runForm(QStringLiteral("normalizeAudio"), {at("tone.mp3")}, [](QQuickWindow *w) {
            w->setProperty("targetLufs", -23);
            w->setProperty("loudnessRange", 7);
            w->setProperty("truePeak", -3.0);
        }, &out));
        QVERIFY2(qAbs(lufs(out.first()) + 23.0) < 2.5, qPrintable(QString::number(lufs(out.first()))));
    }

    void pdfFormsWrite()
    {
        NEED("magick");
        NEED("qpdf");
        NEED("pdfinfo");
        NEED("gs");
        QStringList out;
        QVERIFY(runForm(QStringLiteral("splitPDF"), {at("book.pdf")}, [](QQuickWindow *w) {
            QTRY_VERIFY_WITH_TIMEOUT(w->property("pageCount").toInt() > 0, 15000);
            w->setProperty("pagesPerFile", 2);
        }, &out));
        QCOMPARE(QDir(out.first()).entryList(QDir::Files).size(), 3);
        QVERIFY(runForm(QStringLiteral("mergePDF"), {at("book.pdf"), at("book2.pdf")}, [](QQuickWindow *w) {
            w->setProperty("uniformWidth", true);
        }, &out));
        QCOMPARE(pdfPages(out.first()), 10);
        for (const double width : pageWidths(out.first()))
            QVERIFY2(qAbs(width - 200.0) < 1.0, qPrintable(QString::number(width)));
        QVERIFY(runForm(QStringLiteral("organizePDF"), {at("book.pdf")}, [](QQuickWindow *w) {
            QTRY_VERIFY_WITH_TIMEOUT(w->property("pageCount").toInt() > 0, 15000);
            auto *field = w->findChild<QObject *>(QStringLiteral("pdfPagesField"));
            QVERIFY(field);
            field->setProperty("text", QStringLiteral("3,1"));
        }, &out));
        QCOMPARE(pdfPages(out.first()), 2);
        // Page 3 of the book is the 200 pt wide one.
        QVERIFY(qAbs(pageWidths(out.first()).first() - 200.0) < 1.0);
    }

    void mergeAndJoinFollowTheChosenOrder()
    {
        NEED("magick");
        NEED("ffmpeg");
        NEED("pdfinfo");
        NEED("qpdf");
        QVERIFY(make(QStringLiteral("magick"), {QStringLiteral("-size"), QStringLiteral("300x300"), QStringLiteral("xc:white"), QStringLiteral("-units"), QStringLiteral("PixelsPerInch"),
                                                QStringLiteral("-density"), QStringLiteral("72"), at("square.pdf")}));
        QStringList out;
        QVERIFY(runForm(QStringLiteral("mergePDF"), {at("book.pdf"), at("square.pdf")}, [](QQuickWindow *w) {
            QMetaObject::invokeMethod(w, "moveFile", Q_ARG(QVariant, 1), Q_ARG(QVariant, -1));
        }, &out));
        QVERIFY(qAbs(pageWidths(out.first()).first() - 300.0) < 1.0);

        QVERIFY(make(QStringLiteral("ffmpeg"), {QStringLiteral("-v"), QStringLiteral("error"), QStringLiteral("-f"), QStringLiteral("lavfi"), QStringLiteral("-i"), QStringLiteral("color=red:d=1:s=640x360:r=25"),
                                                QStringLiteral("-f"), QStringLiteral("lavfi"), QStringLiteral("-i"), QStringLiteral("sine=d=1"), QStringLiteral("-shortest"),
                                                QStringLiteral("-c:v"), QStringLiteral("libx264"), QStringLiteral("-pix_fmt"), QStringLiteral("yuv420p"), QStringLiteral("-c:a"), QStringLiteral("aac"), at("red.mp4")}));
        QVERIFY(runForm(QStringLiteral("joinVideos"), {at("clip.mp4"), at("red.mp4")}, [](QQuickWindow *w) {
            QMetaObject::invokeMethod(w, "moveFile", Q_ARG(QVariant, 1), Q_ARG(QVariant, -1));
        }, &out));
        QVERIFY(make(QStringLiteral("ffmpeg"), {QStringLiteral("-v"), QStringLiteral("error"), QStringLiteral("-y"), QStringLiteral("-i"), out.first(), QStringLiteral("-frames:v"), QStringLiteral("1"), at("first.png")}));
        const QColor first = QImage(at("first.png")).pixelColor(320, 180);
        QVERIFY2(first.red() > 200 && first.green() < 60, qPrintable(first.name()));
    }

    void formatDefaultsReachTheEngine()
    {
        NEED("ffmpeg");
        NEED("magick");
        Settings *s = app->settings();
        const auto bits = [](const QString &file) {
            return output(QStringLiteral("ffprobe"), {QStringLiteral("-v"), QStringLiteral("error"), QStringLiteral("-select_streams"), QStringLiteral("a:0"),
                                                      QStringLiteral("-show_entries"), QStringLiteral("stream=bit_rate"), QStringLiteral("-of"), QStringLiteral("csv=p=0"), file})
                .toDouble();
        };
        QStringList out;
        s->setFormatDefault(QStringLiteral("m4a"), QStringLiteral("audioKbps"), 96);
        s->setFormatDefault(QStringLiteral("m4a"), QStringLiteral("keepMetadata"), false);
        QVERIFY(convert(QStringLiteral("m4a"), at("tone.mp3"), &out));
        QVERIFY2(qAbs(bits(out.first()) / 1000.0 - 96.0) < 30.0, qPrintable(QString::number(bits(out.first()))));

        s->setFormatDefault(QStringLiteral("mkv"), QStringLiteral("videoHeight"), 180);
        s->setFormatDefault(QStringLiteral("mkv"), QStringLiteral("quality"), QStringLiteral("smaller"));
        QVERIFY(convert(QStringLiteral("mkv"), at("clip.mp4"), &out));
        QCOMPARE(videoSize(out.first()), QStringLiteral("320,180"));

        s->setFormatDefault(QStringLiteral("gif"), QStringLiteral("gifWidth"), 320);
        s->setFormatDefault(QStringLiteral("gif"), QStringLiteral("gifFps"), 5);
        QVERIFY(convert(QStringLiteral("gif"), at("clip.mp4"), &out));
        QCOMPARE(videoSize(out.first()), QStringLiteral("320,180"));

        s->setFormatDefault(QStringLiteral("wav"), QStringLiteral("sampleDepth"), QStringLiteral("24"));
        QVERIFY(convert(QStringLiteral("wav"), at("tone.mp3"), &out));
        QCOMPARE(output(QStringLiteral("ffprobe"), {QStringLiteral("-v"), QStringLiteral("error"), QStringLiteral("-select_streams"), QStringLiteral("a:0"),
                                                    QStringLiteral("-show_entries"), QStringLiteral("stream=codec_name"), QStringLiteral("-of"), QStringLiteral("csv=p=0"), out.first()}),
                 QStringLiteral("pcm_s24le"));
        s->setFormatDefault(QStringLiteral("flac"), QStringLiteral("audioCompression"), QStringLiteral("fast"));
        QVERIFY(convert(QStringLiteral("flac"), at("tone.mp3"), &out));

        s->setFormatDefault(QStringLiteral("jpg"), QStringLiteral("quality"), QStringLiteral("smaller"));
        QVERIFY(convert(QStringLiteral("jpg"), at("photo.png"), &out));
        const qint64 small = QFileInfo(out.first()).size();
        s->setFormatDefault(QStringLiteral("jpg"), QStringLiteral("quality"), QStringLiteral("higher"));
        QVERIFY(convert(QStringLiteral("jpg"), at("photo.png"), &out));
        QVERIFY(small < QFileInfo(out.first()).size());
    }
};

QTEST_MAIN(TestFormsEngine)
#include "tst_forms_engine.moc"
