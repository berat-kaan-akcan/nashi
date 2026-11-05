// Settings that live outside nashi.conf: the XDG autostart entry and the
// chord mapping in nashi-inputd's config file; plus Settings::wheelScale.
#include "autostart.h"
#include "inputdclient.h"
#include "settings.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QTest>

namespace {
QByteArray readAll(const QString &path)
{
    QFile f(path);
    return f.open(QIODevice::ReadOnly) ? f.readAll() : QByteArray();
}

void writeFile(const QString &path, const QByteArray &data)
{
    QDir().mkpath(QFileInfo(path).absolutePath());
    QFile f(path);
    QVERIFY(f.open(QIODevice::WriteOnly | QIODevice::Truncate));
    f.write(data);
}

QMap<QString, QString> chords(const QString &shift, const QString &ctrl, const QString &alt, const QString &meta)
{
    return {{QStringLiteral("shift"), shift},
            {QStringLiteral("ctrl"), ctrl},
            {QStringLiteral("alt"), alt},
            {QStringLiteral("meta"), meta}};
}
} // namespace

class TestUserConfig : public QObject
{
    Q_OBJECT
private Q_SLOTS:
    // --- autostart -----------------------------------------------------------

    void autostartWithoutSystemEntry()
    {
        QTemporaryDir dir;
        const QString home = dir.filePath(QStringLiteral("config"));
        Autostart a(home, {dir.filePath(QStringLiteral("xdg"))}, QStringLiteral("nashi"));
        QCOMPARE(a.userEntryPath(), home + QStringLiteral("/autostart/io.github.beratkaanakcan.Nashi-autostart.desktop"));
        QVERIFY(a.systemEntryPath().isEmpty());
        QVERIFY(!a.isEnabled());

        QVERIFY(a.setEnabled(true));
        QVERIFY(a.isEnabled());
        const QByteArray content = readAll(a.userEntryPath());
        QVERIFY(content.startsWith("[Desktop Entry]\n"));
        QVERIFY(content.contains("\nExec=nashi --background\n"));
        QVERIFY(content.contains("\nTryExec=nashi\n"));
        QVERIFY(!content.contains("Hidden=true"));
        // No temp files left behind.
        QCOMPARE(QDir(home + QStringLiteral("/autostart")).entryList(QDir::Files | QDir::Hidden).size(), 1);

        QVERIFY(a.setEnabled(true)); // idempotent
        QVERIFY(a.setEnabled(false));
        QVERIFY(!QFileInfo::exists(a.userEntryPath()));
        QVERIFY(!a.isEnabled());
        QVERIFY(a.setEnabled(false));
    }

    void autostartOverridesSystemEntry()
    {
        QTemporaryDir dir;
        const QString home = dir.filePath(QStringLiteral("config"));
        const QString xdg1 = dir.filePath(QStringLiteral("xdg1"));
        const QString xdg2 = dir.filePath(QStringLiteral("xdg2"));
        const QString sys = xdg2 + QStringLiteral("/autostart/") + Autostart::kFileName;
        writeFile(sys, Autostart::entryContent(QStringLiteral("/usr/bin/nashi"), false));
        const QByteArray sysContent = readAll(sys);

        Autostart a(home, {xdg1, xdg2}, QStringLiteral("nashi"));
        QCOMPARE(a.systemEntryPath(), sys);
        QVERIFY(a.isEnabled()); // the package entry applies

        // Off: a Hidden=true user override, the system file stays untouched.
        QVERIFY(a.setEnabled(false));
        QVERIFY(!a.isEnabled());
        QVERIFY(readAll(a.userEntryPath()).contains("\nHidden=true\n"));
        QCOMPARE(readAll(sys), sysContent);

        // On again: the override goes away.
        QVERIFY(a.setEnabled(true));
        QVERIFY(!QFileInfo::exists(a.userEntryPath()));
        QVERIFY(a.isEnabled());
        QCOMPARE(readAll(sys), sysContent);
    }

    void autostartRespectsExistingUserOverride()
    {
        QTemporaryDir dir;
        const QString home = dir.filePath(QStringLiteral("config"));
        Autostart a(home, {}, QStringLiteral("nashi"));
        writeFile(a.userEntryPath(), "[Desktop Entry]\nType=Application\nExec=nashi\nX-GNOME-Autostart-enabled=false\n");
        QVERIFY(!a.isEnabled());
        QVERIFY(a.setEnabled(true));
        QVERIFY(a.isEnabled());
        QVERIFY(readAll(a.userEntryPath()).contains("X-GNOME-Autostart-enabled=true"));
    }

    void autostartQuotesExec()
    {
        const QByteArray c = Autostart::entryContent(QStringLiteral("/opt/My Apps/nashi"), false);
        QVERIFY(c.contains("\nExec=\"/opt/My Apps/nashi\" --background\n"));
        QVERIFY(Autostart::entryContent(QStringLiteral("nashi"), true).contains("\nHidden=true\n"));
    }

    void settingsStartAtLoginUsesXdgConfigHome()
    {
        QTemporaryDir dir;
        const QByteArray old = qgetenv("XDG_CONFIG_HOME");
        const QByteArray oldDirs = qgetenv("XDG_CONFIG_DIRS");
        qputenv("XDG_CONFIG_HOME", dir.filePath(QStringLiteral("cfg")).toUtf8());
        qputenv("XDG_CONFIG_DIRS", dir.filePath(QStringLiteral("none")).toUtf8());
        {
            Settings s(dir.filePath(QStringLiteral("nashi.conf")), nullptr);
            QSignalSpy spy(&s, &Settings::startAtLoginChanged);
            QVERIFY(!s.startAtLogin());
            s.setStartAtLogin(true);
            QCOMPARE(spy.size(), 1);
            QVERIFY(s.startAtLogin());
            QVERIFY(s.startAtLoginError().isEmpty());
            QVERIFY(QFileInfo::exists(dir.filePath(QStringLiteral("cfg/autostart/")) + Autostart::kFileName));
            s.setStartAtLogin(false);
            QVERIFY(!s.startAtLogin());
        }
        qputenv("XDG_CONFIG_HOME", old);
        qputenv("XDG_CONFIG_DIRS", oldDirs);
    }

    // --- wheel scale ---------------------------------------------------------

    void pdfImageDpi()
    {
        QTemporaryDir dir;
        Settings s(dir.filePath(QStringLiteral("nashi.conf")), nullptr);
        QSignalSpy spy(&s, &Settings::pdfImageDpiChanged);
        QCOMPARE(s.pdfImageDpi(), 150);
        s.setPdfImageDpi(300);
        QCOMPARE(s.pdfImageDpi(), 300);
        QCOMPARE(spy.count(), 1);
        s.setPdfImageDpi(300);
        QCOMPARE(spy.count(), 1);
        s.setPdfImageDpi(5000);
        QCOMPARE(s.pdfImageDpi(), 1200);
        s.setPdfImageDpi(1);
        QCOMPARE(s.pdfImageDpi(), 36);
    }

    void wheelSound()
    {
        QTemporaryDir dir;
        const QString file = dir.filePath(QStringLiteral("nashi.conf"));
        {
            Settings s(file, nullptr);
            QSignalSpy spy(&s, &Settings::wheelSoundChanged);
            QVERIFY(s.wheelSound());
            s.setWheelSound(false);
            QCOMPARE(spy.count(), 1);
            s.setWheelSound(false);
            QCOMPARE(spy.count(), 1);
        }
        Settings again(file, nullptr);
        QVERIFY(!again.wheelSound());
    }

    void formatDefaultsSendNothingUntilChanged()
    {
        QTemporaryDir dir;
        Settings s(dir.filePath(QStringLiteral("nashi.conf")), nullptr);
        for (const QString &format : s.formatDefaultFormats())
            QVERIFY2(s.conversionOptions(format).isEmpty(), qPrintable(format));
        QVERIFY(s.conversionOptions(QStringLiteral("pdf")).isEmpty());
    }

    void formatDefaultsBecomeEngineOptions()
    {
        QTemporaryDir dir;
        Settings s(dir.filePath(QStringLiteral("nashi.conf")), nullptr);
        const auto setting = [&](const char *format, const char *key, const QVariant &value) {
            s.setFormatDefault(QString::fromLatin1(format), QString::fromLatin1(key), value);
        };
        const auto options = [&](const char *format) { return s.conversionOptions(QString::fromLatin1(format)); };

        setting("jpg", "quality", QStringLiteral("smaller"));
        QCOMPARE(options("jpg").value(QStringLiteral("quality")).toInt(), 45);
        setting("jpg", "quality", QStringLiteral("higher"));
        QCOMPARE(options("jpg").value(QStringLiteral("quality")).toInt(), 95);
        setting("jpg", "keepMetadata", false);
        QCOMPARE(options("jpg").value(QStringLiteral("strip_metadata")).toBool(), true);

        // Each container has its own neutral CRF.
        setting("mp4", "quality", QStringLiteral("smaller"));
        QCOMPARE(options("mp4").value(QStringLiteral("crf")).toInt(), 29);
        setting("webm", "quality", QStringLiteral("higher"));
        QCOMPARE(options("webm").value(QStringLiteral("crf")).toInt(), 27);
        setting("mp4", "videoHeight", 720);
        QCOMPARE(options("mp4").value(QStringLiteral("max_height")).toInt(), 720);
        setting("mp4", "audioKbps", 128);
        QCOMPARE(options("mp4").value(QStringLiteral("audio_kbps")).toInt(), 128);

        setting("mp3", "audioKbps", 320);
        QCOMPARE(options("mp3").value(QStringLiteral("audio_kbps")).toInt(), 320);

        setting("gif", "gifFps", 24);
        setting("gif", "gifWidth", 960);
        QCOMPARE(options("gif").value(QStringLiteral("gif_fps")).toInt(), 24);
        QCOMPARE(options("gif").value(QStringLiteral("gif_max_width")).toInt(), 960);

        // One format's choice stays out of the others.
        QVERIFY(options("mkv").isEmpty());
        QVERIFY(options("png").isEmpty());
    }

    void pcmDepthAndFlacCompressionBecomeEngineOptions()
    {
        QTemporaryDir dir;
        Settings s(dir.filePath(QStringLiteral("nashi.conf")), nullptr);
        QVERIFY(s.formatDefaultKinds(QStringLiteral("wav")).contains(QStringLiteral("sampleDepth")));
        QVERIFY(s.formatDefaultKinds(QStringLiteral("aiff")).contains(QStringLiteral("sampleDepth")));
        QVERIFY(!s.formatDefaultKinds(QStringLiteral("flac")).contains(QStringLiteral("sampleDepth")));
        QVERIFY(s.formatDefaultKinds(QStringLiteral("flac")).contains(QStringLiteral("audioCompression")));
        QVERIFY(!s.formatDefaultKinds(QStringLiteral("wav")).contains(QStringLiteral("audioCompression")));
        QVERIFY(s.conversionOptions(QStringLiteral("wav")).isEmpty());
        s.setFormatDefault(QStringLiteral("wav"), QStringLiteral("sampleDepth"), QStringLiteral("24"));
        QCOMPARE(s.conversionOptions(QStringLiteral("wav")).value(QStringLiteral("sample_depth")).toString(), QStringLiteral("24"));
        s.setFormatDefault(QStringLiteral("flac"), QStringLiteral("audioCompression"), QStringLiteral("maximum"));
        QCOMPARE(s.conversionOptions(QStringLiteral("flac")).value(QStringLiteral("audio_compression")).toString(), QStringLiteral("maximum"));
        // An unknown value in the file means the default.
        s.store().setValue(QStringLiteral("formats/wav/sampleDepth"), QStringLiteral("8"));
        QVERIFY(!s.conversionOptions(QStringLiteral("wav")).contains(QStringLiteral("sample_depth")));
    }

    void compressDefaults()
    {
        QTemporaryDir dir;
        Settings s(dir.filePath(QStringLiteral("nashi.conf")), nullptr);
        QCOMPARE(s.compressPreset(), QStringLiteral("balanced"));
        QCOMPARE(s.compressImageSize(), QStringLiteral("original"));
        s.setCompressPreset(QStringLiteral("strong"));
        s.setCompressImageSize(QStringLiteral("1920"));
        QCOMPARE(s.compressPreset(), QStringLiteral("strong"));
        QCOMPARE(s.compressImageSize(), QStringLiteral("1920"));
        s.setCompressPreset(QStringLiteral("nonsense"));
        QCOMPARE(s.compressPreset(), QStringLiteral("strong"));
        s.setCompressImageSize(QStringLiteral("nonsense"));
        QCOMPARE(s.compressImageSize(), QStringLiteral("1920"));
    }

    void formatDefaultsOnlyOfferWhatTheFormatCanUse()
    {
        QTemporaryDir dir;
        Settings s(dir.filePath(QStringLiteral("nashi.conf")), nullptr);
        const auto kinds = [&](const char *format) { return s.formatDefaultKinds(QString::fromLatin1(format)); };
        QVERIFY(kinds("jpg").contains(QStringLiteral("quality")));
        QVERIFY(!kinds("png").contains(QStringLiteral("quality")));
        QVERIFY(kinds("png").contains(QStringLiteral("keepMetadata")));
        QVERIFY(kinds("mp3").contains(QStringLiteral("audioKbps")));
        QVERIFY(!kinds("flac").contains(QStringLiteral("audioKbps")));
        QVERIFY(kinds("flac").contains(QStringLiteral("keepMetadata")));
        QVERIFY(kinds("mp4").contains(QStringLiteral("videoHeight")));
        QVERIFY(kinds("gif").contains(QStringLiteral("gifFps")));
        QVERIFY(kinds("pdf").isEmpty());

        // A choice the format cannot use has no effect, even if the file says otherwise.
        s.setFormatDefault(QStringLiteral("flac"), QStringLiteral("audioKbps"), 128);
        QVERIFY(s.conversionOptions(QStringLiteral("flac")).isEmpty());
        s.setFormatDefault(QStringLiteral("pdf"), QStringLiteral("keepMetadata"), false);
        QVERIFY(s.conversionOptions(QStringLiteral("pdf")).isEmpty());
    }

    void formatDefaultsIgnoreBadValuesAndRestore()
    {
        QTemporaryDir dir;
        const QString file = dir.filePath(QStringLiteral("nashi.conf"));
        {
            QSettings raw(file, QSettings::IniFormat);
            raw.setValue(QStringLiteral("formats/mp3/audioKbps"), 7);
            raw.setValue(QStringLiteral("formats/mp4/videoHeight"), 1);
            raw.setValue(QStringLiteral("formats/jpg/quality"), QStringLiteral("nonsense"));
            raw.setValue(QStringLiteral("formats/gif/gifFps"), 500);
            raw.setValue(QStringLiteral("formats/gif/gifWidth"), 1);
        }
        Settings s(file, nullptr);
        for (const char *format : {"mp3", "mp4", "jpg", "gif"})
            QVERIFY2(s.conversionOptions(QString::fromLatin1(format)).isEmpty(), format);

        QSignalSpy spy(&s, &Settings::formatDefaultsChanged);
        s.setFormatDefault(QStringLiteral("mp4"), QStringLiteral("videoHeight"), 480);
        QCOMPARE(spy.count(), 1);
        QCOMPARE(s.formatDefaults(QStringLiteral("mp4")).value(QStringLiteral("videoHeight")).toInt(), 480);
        s.restoreFormatDefaults(QStringLiteral("mp4"));
        QCOMPARE(spy.count(), 2);
        QCOMPARE(s.formatDefaults(QStringLiteral("mp4")).value(QStringLiteral("videoHeight")).toInt(), 0);
        QVERIFY(s.conversionOptions(QStringLiteral("mp4")).isEmpty());
    }

    void wheelScale()
    {
        QTemporaryDir dir;
        Settings s(dir.filePath(QStringLiteral("nashi.conf")), nullptr);
        QSignalSpy spy(&s, &Settings::wheelScaleChanged);
        QCOMPARE(s.wheelScale(), 1.0);
        s.setWheelScale(1.2);
        QCOMPARE(s.wheelScale(), 1.2);
        QCOMPARE(spy.size(), 1);
        s.setWheelScale(1.2);
        QCOMPARE(spy.size(), 1);
        s.setWheelScale(5.0);
        QCOMPARE(s.wheelScale(), Settings::kMaxWheelScale);
        s.setWheelScale(0.0);
        QCOMPARE(s.wheelScale(), Settings::kMinWheelScale);
        s.store().setValue(QStringLiteral("wheel/scale"), QStringLiteral("garbage"));
        QCOMPARE(s.wheelScale(), 1.0);
        s.store().setValue(QStringLiteral("wheel/scale"), 99);
        QCOMPARE(s.wheelScale(), Settings::kMaxWheelScale);
    }

    // --- inputd.toml chords --------------------------------------------------

    void parseChords()
    {
        QCOMPARE(InputdConfig::parseChords(QString()),
                 chords(QStringLiteral("convert"), QStringLiteral("tools"), QStringLiteral("actions"), QString()));
        const QString text = QStringLiteral("enabled = true # [chords] in a comment\n"
                                            "[chords]\n"
                                            "shift = 'tools' # comment\n"
                                            "\"ctrl\" = \"none\"\n"
                                            "meta=\"convert\"\n"
                                            "[other]\n"
                                            "alt = \"convert\"\n");
        QCOMPARE(InputdConfig::parseChords(text),
                 chords(QStringLiteral("tools"), QString(), QStringLiteral("actions"), QStringLiteral("convert")));
        QCOMPARE(InputdConfig::parseChords(QStringLiteral("chords = { alt = \"tools\", meta = \"actions\" }\n")),
                 chords(QStringLiteral("convert"), QStringLiteral("tools"), QStringLiteral("tools"), QStringLiteral("actions")));
        QCOMPARE(InputdConfig::parseChords(QStringLiteral("chords.shift = \"none\"\n")).value(QStringLiteral("shift")), QString());
    }

    void rewriteKeepsEverythingElse()
    {
        const QString text = QStringLiteral("# my settings\n"
                                            "enabled = false\n"
                                            "drag_hold_ms = 300 # slow\n"
                                            "\n"
                                            "[chords]\n"
                                            "# which key does what\n"
                                            "shift = \"convert\"\n"
                                            "alt = \"actions\"\n");
        const auto wanted = chords(QStringLiteral("tools"), QString(), QStringLiteral("convert"), QStringLiteral("actions"));
        const QString out = InputdConfig::rewriteChords(text, wanted);
        QCOMPARE(out, QStringLiteral("# my settings\n"
                                     "enabled = false\n"
                                     "drag_hold_ms = 300 # slow\n"
                                     "\n"
                                     "[chords]\n"
                                     "shift = \"tools\"\n"
                                     "ctrl = \"none\"\n"
                                     "alt = \"convert\"\n"
                                     "meta = \"actions\"\n"
                                     "# which key does what\n"));
        QCOMPARE(InputdConfig::parseChords(out), wanted);
        // Idempotent.
        QCOMPARE(InputdConfig::rewriteChords(out, wanted), out);
    }

    void rewriteAddsSectionAndReplacesInlineForms()
    {
        const auto wanted = chords(QStringLiteral("convert"), QStringLiteral("tools"), QString(), QStringLiteral("actions"));
        const QString added = InputdConfig::rewriteChords(QStringLiteral("drag_move_px = 12"), wanted);
        QCOMPARE(added, QStringLiteral("drag_move_px = 12\n\n[chords]\nshift = \"convert\"\nctrl = \"tools\"\nalt = \"none\"\nmeta = \"actions\"\n"));
        QCOMPARE(InputdConfig::rewriteChords(QString(), wanted),
                 QStringLiteral("[chords]\nshift = \"convert\"\nctrl = \"tools\"\nalt = \"none\"\nmeta = \"actions\"\n"));

        const QString inlineText = QStringLiteral("chords = { shift = \"tools\" }\nchords.alt = \"convert\"\nenabled = true\n");
        const QString out = InputdConfig::rewriteChords(inlineText, wanted);
        QVERIFY(!out.contains(QStringLiteral("chords =")));
        QVERIFY(!out.contains(QStringLiteral("chords.alt")));
        QVERIFY(out.contains(QStringLiteral("enabled = true\n")));
        QCOMPARE(InputdConfig::parseChords(out), wanted);
    }

    void clientSetChordsWritesAtomically()
    {
        QTemporaryDir dir;
        const QString path = dir.filePath(QStringLiteral("nashi/inputd.toml"));
        writeFile(path, "enabled = false\ndrag_move_px = 4\n");
        InputdClient client(path, nullptr);
        QCOMPARE(client.convertKey(), QStringLiteral("shift"));
        QCOMPARE(client.toolsKey(), QStringLiteral("ctrl"));
        QCOMPARE(client.actionsKey(), QStringLiteral("alt"));

        QSignalSpy spy(&client, &InputdClient::chordsChanged);
        QVERIFY(client.setChords(QStringLiteral("ctrl"), QStringLiteral("meta"), QStringLiteral("shift")));
        QCOMPARE(spy.size(), 1);
        QCOMPARE(client.convertKey(), QStringLiteral("ctrl"));
        QCOMPARE(client.toolsKey(), QStringLiteral("meta"));
        QCOMPARE(client.actionsKey(), QStringLiteral("shift"));
        QVERIFY(client.configError().isEmpty());
        const QByteArray written = readAll(path);
        QVERIFY(written.startsWith("enabled = false\ndrag_move_px = 4\n"));
        QVERIFY(written.contains("alt = \"none\"\n"));
        QCOMPARE(QDir(dir.filePath(QStringLiteral("nashi"))).entryList(QDir::Files | QDir::Hidden).size(), 1);

        // Duplicates and unknown keys are refused without touching the file.
        QVERIFY(!client.setChords(QStringLiteral("ctrl"), QStringLiteral("ctrl"), QStringLiteral("alt")));
        QVERIFY(!client.setChords(QStringLiteral("ctrl"), QStringLiteral("hyper"), QStringLiteral("alt")));
        QVERIFY(!client.configError().isEmpty());
        QCOMPARE(readAll(path), written);
    }

    void clientCreatesMissingConfig()
    {
        QTemporaryDir dir;
        const QString path = dir.filePath(QStringLiteral("deep/nashi/inputd.toml"));
        InputdClient client(path, nullptr);
        QVERIFY(client.setChords(QStringLiteral("alt"), QStringLiteral("ctrl"), QStringLiteral("shift")));
        QCOMPARE(InputdConfig::parseChords(QString::fromUtf8(readAll(path))),
                 chords(QStringLiteral("actions"), QStringLiteral("tools"), QStringLiteral("convert"), QString()));
    }

    void defaultPathFollowsXdg()
    {
        const QByteArray old = qgetenv("XDG_CONFIG_HOME");
        qputenv("XDG_CONFIG_HOME", "/tmp/xdg-test");
        QCOMPARE(InputdConfig::defaultPath(), QStringLiteral("/tmp/xdg-test/nashi/inputd.toml"));
        qputenv("XDG_CONFIG_HOME", "relative/ignored");
        QCOMPARE(InputdConfig::defaultPath(), QDir::homePath() + QStringLiteral("/.config/nashi/inputd.toml"));
        qputenv("XDG_CONFIG_HOME", old);
    }
};

QTEST_GUILESS_MAIN(TestUserConfig)
#include "tst_userconfig.moc"
