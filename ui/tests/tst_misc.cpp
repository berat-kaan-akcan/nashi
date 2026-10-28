#include "actions.h"
#include "depsmodel.h"
#include "language.h"
#include "recentmodel.h"
#include "settings.h"

#include <QJsonDocument>
#include <QMimeData>
#include <QSettings>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QTest>
#include <QUrl>

class TestMisc : public QObject
{
    Q_OBJECT
private Q_SLOTS:
    void copyMimeData()
    {
        std::unique_ptr<QMimeData> m(FileActions::copyMimeData({QStringLiteral("/tmp/a b.png"), QStringLiteral("/tmp/c.mp4")}));
        QCOMPARE(m->urls().size(), 2);
        QCOMPARE(m->urls().at(0), QUrl::fromLocalFile(QStringLiteral("/tmp/a b.png")));
        QCOMPARE(m->text(), QStringLiteral("/tmp/a b.png\n/tmp/c.mp4"));
        QCOMPARE(m->data(QStringLiteral("x-special/gnome-copied-files")),
                 QByteArray("copy\nfile:///tmp/a%20b.png\nfile:///tmp/c.mp4"));
        QCOMPARE(m->data(QStringLiteral("application/x-kde-cutselection")), QByteArray("0"));
    }

    void terminalCommand()
    {
        const auto has = [](QStringList available) {
            return [available](const QString &p) { return available.contains(p); };
        };
        QCOMPARE(FileActions::terminalCommand(QStringLiteral("/d"), QString(), has({QStringLiteral("konsole")})),
                 QStringList({QStringLiteral("konsole"), QStringLiteral("--workdir"), QStringLiteral("/d")}));
        QCOMPARE(FileActions::terminalCommand(QStringLiteral("/d"), QStringLiteral("alacritty --class x"),
                                              has({QStringLiteral("alacritty"), QStringLiteral("xterm")})),
                 QStringList({QStringLiteral("alacritty"), QStringLiteral("--class"), QStringLiteral("x")}));
        QCOMPARE(FileActions::terminalCommand(QStringLiteral("/d"), QStringLiteral("missing-term"),
                                              has({QStringLiteral("x-terminal-emulator")})),
                 QStringList{QStringLiteral("x-terminal-emulator")});
        QVERIFY(FileActions::terminalCommand(QStringLiteral("/d"), QString(), has({})).isEmpty());
        // $TERMINAL is tokenised, never handed to a shell.
        const QStringList evil = FileActions::terminalCommand(QStringLiteral("/d"), QStringLiteral("xterm; rm -rf ~"),
                                                              has({QStringLiteral("xterm;")}));
        QCOMPARE(evil.constFirst(), QStringLiteral("xterm;"));
    }

    void actionIds()
    {
        FileActions a;
        QCOMPARE(FileActions::knownIds().size(), 5);
        QVERIFY(!FileActions::knownIds().contains(QStringLiteral("move")));
        QVERIFY(!a.run(QStringLiteral("move"), {QStringLiteral("/tmp/x")}));
        QVERIFY(!a.run(QStringLiteral("copy"), {}));
    }

    void depsParsing()
    {
        const QJsonObject deps = QJsonDocument::fromJson(R"({"tools": [
          {"id": "ffmpeg", "available": true, "version": "7.1", "path": "/usr/bin/ffmpeg", "package": "ffmpeg", "optional": false, "used_for": ["audio"]},
          {"id": "exiftool", "available": false, "version": null, "path": null, "package": "perl-image-exiftool", "optional": false, "used_for": ["removeMetadata"]},
          {"id": "pandoc", "available": false, "version": null, "path": null, "package": "pandoc-cli", "optional": true, "used_for": []},
          {"id": "ffprobe", "available": false, "package": "ffmpeg", "optional": false}
        ]})").object();
        DepsModel m;
        m.setData(deps);
        QCOMPARE(m.rowCount(), 4);
        QCOMPARE(m.missingCount(), 3);
        // Missing required first.
        QCOMPARE(m.data(m.index(0), DepsModel::AvailableRole).toBool(), false);
        QCOMPARE(m.data(m.index(0), DepsModel::OptionalRole).toBool(), false);
        QCOMPARE(m.data(m.index(3), DepsModel::IdRole).toString(), QStringLiteral("ffmpeg"));
        QCOMPARE(m.missingPackages(false), QStringList({QStringLiteral("perl-image-exiftool"), QStringLiteral("ffmpeg")}));
        QCOMPARE(m.installCommand(), QStringLiteral("sudo pacman -S --needed perl-image-exiftool ffmpeg pandoc-cli"));
        QCOMPARE(m.requiredInstallCommand(), QStringLiteral("sudo pacman -S --needed perl-image-exiftool ffmpeg"));
        QCOMPARE(DepsModel::pacmanCommand({}), QString());
    }

    void depsFromFakeCli()
    {
        DepsModel m;
        m.setProgram(QStringLiteral(FAKE_CLI));
        m.refresh();
        QVERIFY(m.loading());
        QTRY_VERIFY_WITH_TIMEOUT(!m.loading(), 10000);
        QCOMPARE(m.rowCount(), 3);
        QVERIFY(m.error().isEmpty());
        QVERIFY(m.installCommand().contains(QStringLiteral("perl-image-exiftool")));
    }

    void recentPersists()
    {
        QTemporaryDir dir;
        const QString file = dir.filePath(QStringLiteral("nashi.conf"));
        {
            QSettings s(file, QSettings::IniFormat);
            RecentModel r(&s);
            r.add({QStringLiteral("/a.mp3"), QStringLiteral("/b.mp3")});
            r.add({QStringLiteral("/a.mp3")}); // moves to the top, no duplicate
            QCOMPARE(r.rowCount(), 2);
            QCOMPARE(r.data(r.index(0), RecentModel::PathRole).toString(), QStringLiteral("/a.mp3"));
        }
        QSettings s(file, QSettings::IniFormat);
        RecentModel r(&s);
        QCOMPARE(r.rowCount(), 2);
        QCOMPARE(r.data(r.index(1), RecentModel::NameRole).toString(), QStringLiteral("b.mp3"));
        r.remove(0);
        QCOMPARE(r.rowCount(), 1);
        QStringList many;
        for (int i = 0; i < 80; ++i)
            many << QStringLiteral("/f%1").arg(i);
        r.add(many);
        QCOMPARE(r.rowCount(), RecentModel::kMaxEntries);
        r.clear();
        QCOMPARE(r.rowCount(), 0);
    }

    void settingsDefaultsAndValidation()
    {
        QTemporaryDir dir;
        Settings s(dir.filePath(QStringLiteral("nashi.conf")), nullptr);
        QCOMPARE(s.language(), QStringLiteral("auto"));
        QVERIFY(s.notifications());
        QCOMPARE(s.effectiveOutputDir(), QString());
        QSignalSpy out(&s, &Settings::outputChanged);
        s.setCustomOutputDir(QStringLiteral("/out"));
        QCOMPARE(s.effectiveOutputDir(), QString()); // not enabled yet
        s.setUseCustomOutputDir(true);
        QCOMPARE(s.effectiveOutputDir(), QStringLiteral("/out"));
        QCOMPARE(out.count(), 2);
        s.store().setValue(QStringLiteral("ui/language"), QStringLiteral("klingon"));
        QCOMPARE(s.language(), QStringLiteral("auto"));
    }

    void languageResolution()
    {
        QCOMPARE(Language::resolve(QStringLiteral("auto"), QLocale(QLocale::Turkish, QLocale::Turkey)), QStringLiteral("tr"));
        QCOMPARE(Language::resolve(QStringLiteral("auto"), QLocale(QLocale::German)), QStringLiteral("en"));
        QCOMPARE(Language::resolve(QStringLiteral("en"), QLocale(QLocale::Turkish)), QStringLiteral("en"));
        QCOMPARE(Language::resolve(QStringLiteral("tr"), QLocale(QLocale::English)), QStringLiteral("tr"));
    }

    void turkishTranslationLoads()
    {
        Language lang;
        lang.apply(QStringLiteral("tr"));
        QCOMPARE(lang.code(), QStringLiteral("tr"));
        QCOMPARE(QCoreApplication::translate("JobCard", "Show in folder"), QStringLiteral("Klasörde göster"));
        QCOMPARE(QCoreApplication::translate("Selection", "%n file(s)", nullptr, 3), QStringLiteral("3 dosya"));
        lang.apply(QStringLiteral("en"));
        QCOMPARE(QCoreApplication::translate("JobCard", "Show in folder"), QStringLiteral("Show in folder"));
    }
};

QTEST_MAIN(TestMisc)
#include "tst_misc.moc"
