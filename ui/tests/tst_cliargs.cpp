#include "cliargs.h"

#include <QCoreApplication>
#include <QTest>
#include <QUrl>
#include <QVariantMap>

class TestCliArgs : public QObject
{
    Q_OBJECT

    static CommandLine p(const QStringList &args)
    {
        return CliArgs::parse(QStringList{QStringLiteral("nashi")} + args, QStringLiteral("/home/u/work"));
    }

private Q_SLOTS:
    void plainStart()
    {
        const auto c = p({});
        QCOMPARE(c.action, CommandLine::Start);
        const ForwardCall f = CliArgs::forwardCallFor(c);
        QCOMPARE(f.interface, CliArgs::kInterface);
        QCOMPARE(f.method, QStringLiteral("Activate"));
        QVERIFY(f.args.isEmpty());
    }

    void background()
    {
        const auto c = p({QStringLiteral("--background")});
        QCOMPARE(c.action, CommandLine::Background);
        QVERIFY(!CliArgs::forwardCallFor(c).valid());
    }

    void wheelWithFilesAndMode()
    {
        const auto c = p({QStringLiteral("--wheel"), QStringLiteral("--mode"), QStringLiteral("tools"),
                          QStringLiteral("a.mp4"), QStringLiteral("/abs/b.png"), QStringLiteral("../c d.pdf")});
        QCOMPARE(c.action, CommandLine::Wheel);
        QCOMPARE(c.mode, QStringLiteral("tools"));
        QCOMPARE(c.files, QStringList({QStringLiteral("/home/u/work/a.mp4"), QStringLiteral("/abs/b.png"),
                                       QStringLiteral("/home/u/c d.pdf")}));
        const ForwardCall f = CliArgs::forwardCallFor(c);
        QCOMPARE(f.method, QStringLiteral("ShowWheel"));
        QCOMPARE(f.args.size(), 2);
        QCOMPARE(f.args.at(0).toStringList(), c.files);
        QCOMPARE(f.args.at(1).toString(), QStringLiteral("tools"));
    }

    void wheelDefaultsToConvert()
    {
        const auto c = p({QStringLiteral("--wheel"), QStringLiteral("x.jpg")});
        QCOMPARE(c.mode, QStringLiteral("convert"));
        const auto e = p({QStringLiteral("--mode=actions"), QStringLiteral("--wheel"), QStringLiteral("x.jpg")});
        QCOMPARE(e.mode, QStringLiteral("actions"));
    }

    void wheelWithoutFilesUsesClipboard()
    {
        const auto c = p({QStringLiteral("--wheel")});
        QCOMPARE(c.action, CommandLine::WheelClipboard);
        const ForwardCall f = CliArgs::forwardCallFor(c);
        QCOMPARE(f.method, QStringLiteral("ShowWheelFromClipboard"));
        QCOMPARE(f.args, QVariantList{QStringLiteral("convert")});
    }

    void openFilesForwardsViaFdoApplication()
    {
        const auto c = p({QStringLiteral("file:///tmp/a%20b.png"), QStringLiteral("rel.txt")});
        QCOMPARE(c.action, CommandLine::OpenFiles);
        QCOMPARE(c.files, QStringList({QStringLiteral("/tmp/a b.png"), QStringLiteral("/home/u/work/rel.txt")}));
        const ForwardCall f = CliArgs::forwardCallFor(c);
        QCOMPARE(f.interface, CliArgs::kFdoAppInterface);
        QCOMPARE(f.method, QStringLiteral("Open"));
        QCOMPARE(f.args.at(0).toStringList(),
                 QStringList({QStringLiteral("file:///tmp/a%20b.png"), QStringLiteral("file:///home/u/work/rel.txt")}));
        QVERIFY(f.args.at(1).canConvert<QVariantMap>());
    }

    void dashFilesAfterSeparator()
    {
        const auto c = p({QStringLiteral("--wheel"), QStringLiteral("--"), QStringLiteral("-odd.png")});
        QCOMPARE(c.action, CommandLine::Wheel);
        QCOMPARE(c.files, QStringList{QStringLiteral("/home/u/work/-odd.png")});
    }

    void downloadAndShelfFlags()
    {
        const auto dl = p({QStringLiteral("--download"), QStringLiteral("https://example.org/watch?v=42")});
        QCOMPARE(dl.action, CommandLine::Download);
        QCOMPARE(dl.url, QStringLiteral("https://example.org/watch?v=42"));
        const ForwardCall fdl = CliArgs::forwardCallFor(dl);
        QCOMPARE(fdl.interface, CliArgs::kInterface);
        QCOMPARE(fdl.method, QStringLiteral("Download"));
        QCOMPARE(fdl.args, QVariantList{QStringLiteral("https://example.org/watch?v=42")});

        const auto dlc = p({QStringLiteral("--download-clipboard")});
        QCOMPARE(dlc.action, CommandLine::DownloadClipboard);
        const ForwardCall fdlc = CliArgs::forwardCallFor(dlc);
        QCOMPARE(fdlc.interface, CliArgs::kInterface);
        QCOMPARE(fdlc.method, QStringLiteral("DownloadClipboard"));
        QVERIFY(fdlc.args.isEmpty());

        const auto st = p({QStringLiteral("--shelf")});
        QCOMPARE(st.action, CommandLine::ToggleShelf);
        const ForwardCall fst = CliArgs::forwardCallFor(st);
        QCOMPARE(fst.interface, CliArgs::kInterface);
        QCOMPARE(fst.method, QStringLiteral("ToggleShelf"));
        QVERIFY(fst.args.isEmpty());

        const auto sa = p({QStringLiteral("--shelf"), QStringLiteral("a.png"), QStringLiteral("/tmp/b.pdf")});
        QCOMPARE(sa.action, CommandLine::AddToShelf);
        QCOMPARE(sa.files, QStringList({QStringLiteral("/home/u/work/a.png"), QStringLiteral("/tmp/b.pdf")}));
        const ForwardCall fsa = CliArgs::forwardCallFor(sa);
        QCOMPARE(fsa.interface, CliArgs::kInterface);
        QCOMPARE(fsa.method, QStringLiteral("AddToShelf"));
        QCOMPARE(fsa.args, QVariantList{sa.files});

        QCOMPARE(p({QStringLiteral("--download"), QStringLiteral("ftp://example.org/a")}).action, CommandLine::Error);
        QCOMPARE(p({QStringLiteral("--download"), QStringLiteral("https://example.org"), QStringLiteral("extra.png")}).action,
                 CommandLine::Error);
        QCOMPARE(p({QStringLiteral("--download-clipboard"), QStringLiteral("extra.png")}).action, CommandLine::Error);
        QCOMPARE(p({QStringLiteral("--shelf"), QStringLiteral("--wheel")}).action, CommandLine::Error);
    }

    void errors()
    {
        QCOMPARE(p({QStringLiteral("--wheel"), QStringLiteral("--mode"), QStringLiteral("explode")}).action, CommandLine::Error);
        QCOMPARE(p({QStringLiteral("--mode"), QStringLiteral("tools")}).action, CommandLine::Error);
        QCOMPARE(p({QStringLiteral("--nope")}).action, CommandLine::Error);
        QVERIFY(!CliArgs::forwardCallFor(p({QStringLiteral("--nope")})).valid());
        QCOMPARE(p({QStringLiteral("--help")}).action, CommandLine::ShowHelp);
        QCOMPARE(p({QStringLiteral("--version")}).action, CommandLine::ShowVersion);
    }
};

QTEST_GUILESS_MAIN(TestCliArgs)
#include "tst_cliargs.moc"
