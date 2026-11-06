#include "windowplacer.h"

#include <QJSEngine>
#include <QSignalSpy>
#include <QTest>

// The KWin script is run here against stand-ins for KWin's `workspace` and
// `KWin` objects, so both its syntax and which window it picks are checked.
class TestWindowPlacer : public QObject
{
    Q_OBJECT

    static QString mockWindows()
    {
        return QStringLiteral(R"JS(
var log = [];
function win(name, cls, extra) {
  var w = { name: name, resourceClass: cls, desktopFileName: cls, normalWindow: true, minimized: false,
            fullScreen: false, moveable: true, resizeable: true, onAllDesktops: false,
            desktops: [{ id: "d1" }], geometry: null, maximized: null };
  for (var k in extra) w[k] = extra[k];
  w.setMaximize = function (v, h) { this.maximized = [v, h]; };
  return w;
}
var KWin = { MaximizeArea: 7 };
var workspace = {
  currentDesktop: { id: "d1" },
  clientArea: function (area, w) { return { x: 0, y: 30, width: 1200, height: 800 }; },
  stackingOrder: []
};
)JS");
    }

private Q_SLOTS:
    void everyLayoutHasFractionsInsideTheScreen()
    {
        const QStringList ids = WindowPlacer::layouts();
        QVERIFY(ids.size() >= 12);
        QCOMPARE(QSet<QString>(ids.begin(), ids.end()).size(), ids.size());
        for (const QString &id : ids) {
            const QVariantList f = WindowPlacer::fractions(id);
            QCOMPARE(f.size(), 4);
            QVERIFY2(f[0].toDouble() >= 0 && f[1].toDouble() >= 0, qPrintable(id));
            QVERIFY2(f[0].toDouble() + f[2].toDouble() <= 1.0001, qPrintable(id));
            QVERIFY2(f[1].toDouble() + f[3].toDouble() <= 1.0001, qPrintable(id));
            QVERIFY2(!WindowPlacer::scriptFor(id).isEmpty(), qPrintable(id));
        }
        QVERIFY(WindowPlacer::fractions(QStringLiteral("nope")).isEmpty());
        QVERIFY(WindowPlacer::scriptFor(QStringLiteral("left\"); evil(\"")).isEmpty());
    }

    void movesTheTopmostWindowThatIsNotNashi()
    {
        QJSEngine js;
        QVERIFY(!js.evaluate(mockWindows()).isError());
        const QJSValue r = js.evaluate(QStringLiteral(R"JS(
var below = win("below", "firefox");
var top = win("top", "org.kde.kcalc");
var mine = win("mine", "io.github.beratkaanakcan.nashi");
var minimized = win("min", "kate", { minimized: true });
var other = win("other", "dolphin", { desktops: [{ id: "d2" }] });
workspace.stackingOrder = [below, top, minimized, other, mine];
)JS"));
        QVERIFY2(!r.isError(), qPrintable(r.toString()));
        const QJSValue run = js.evaluate(WindowPlacer::scriptFor(QStringLiteral("rightThird")));
        QVERIFY2(!run.isError(), qPrintable(run.toString()));
        const QJSValue g = js.evaluate(QStringLiteral("top.frameGeometry"));
        QCOMPARE(g.property(QStringLiteral("x")).toInt(), 800);
        QCOMPARE(g.property(QStringLiteral("y")).toInt(), 30);
        QCOMPARE(g.property(QStringLiteral("width")).toInt(), 400);
        QCOMPARE(g.property(QStringLiteral("height")).toInt(), 800);
        QCOMPARE(js.evaluate(QStringLiteral("below.frameGeometry === undefined")).toBool(), true);
        QCOMPARE(js.evaluate(QStringLiteral("mine.frameGeometry === undefined")).toBool(), true);
        QCOMPARE(js.evaluate(QStringLiteral("minimized.frameGeometry === undefined")).toBool(), true);
        QCOMPARE(js.evaluate(QStringLiteral("other.frameGeometry === undefined")).toBool(), true);
        QCOMPARE(js.evaluate(QStringLiteral("top.maximized[0] || top.maximized[1]")).toBool(), false);
    }

    void maximizeAsksKwinToMaximize()
    {
        QJSEngine js;
        QVERIFY(!js.evaluate(mockWindows()).isError());
        QVERIFY(!js.evaluate(QStringLiteral("var a = win('a', 'kate'); workspace.stackingOrder = [a];")).isError());
        QVERIFY(!js.evaluate(WindowPlacer::scriptFor(QStringLiteral("maximize"))).isError());
        QCOMPARE(js.evaluate(QStringLiteral("a.maximized.join()")).toString(), QStringLiteral("true,true"));
    }

    void doesNothingWhenOnlyNashiIsOpen()
    {
        QJSEngine js;
        QVERIFY(!js.evaluate(mockWindows()).isError());
        QVERIFY(!js.evaluate(QStringLiteral("var m = win('m', 'nashi'); workspace.stackingOrder = [m];")).isError());
        const QJSValue run = js.evaluate(WindowPlacer::scriptFor(QStringLiteral("left")));
        QVERIFY2(!run.isError(), qPrintable(run.toString()));
        QCOMPARE(js.evaluate(QStringLiteral("m.frameGeometry === undefined")).toBool(), true);
    }

    void reportsFailureWithoutKwin()
    {
        WindowPlacer placer;
        QSignalSpy spy(&placer, &WindowPlacer::placed);
        // The test bus does not exist, so there is no KWin.
        QVERIFY(!placer.available());
        QVERIFY(!placer.place(QStringLiteral("left")));
        QVERIFY(!placer.place(QStringLiteral("unknown")));
        QCOMPARE(spy.size(), 2);
        QCOMPARE(spy.at(0).at(1).toBool(), false);
    }
};

QTEST_MAIN(TestWindowPlacer)
#include "tst_windowplacer.moc"
