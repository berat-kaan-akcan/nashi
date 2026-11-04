#include "wheelgeometry.h"

#include <QTest>
#include <QtMath>

#include <cmath>

class TestWheelGeometry : public QObject
{
    Q_OBJECT

    static int at(double angleFromTopDeg, double r, int n, double inner = 50, double outer = 200)
    {
        const double rad = qDegreesToRadians(angleFromTopDeg);
        return WheelGeometry::sliceAt(r * std::sin(rad), -r * std::cos(rad), n, inner, outer);
    }

private Q_SLOTS:
    void centreIsCancel()
    {
        QCOMPARE(WheelGeometry::sliceAt(0, 0, 6, 50, 200), int(WheelGeometry::Cancel));
        QCOMPARE(WheelGeometry::sliceAt(30, 30, 6, 50, 200), int(WheelGeometry::Cancel));
        QCOMPARE(WheelGeometry::sliceAt(0, 0, 0, 50, 200), int(WheelGeometry::Cancel));
    }

    void farIsOutside()
    {
        QCOMPARE(at(0, 200 * WheelGeometry::kOuterReach + 1, 6), int(WheelGeometry::Outside));
        QCOMPARE(at(0, 200 * WheelGeometry::kOuterReach - 1, 6), 0);
        QCOMPARE(WheelGeometry::sliceAt(100, 0, 0, 50, 200), int(WheelGeometry::Outside));
    }

    void cardinalDirections()
    {
        // 4 slices: 0 = top, 1 = right, 2 = bottom, 3 = left.
        QCOMPARE(at(0, 100, 4), 0);
        QCOMPARE(at(90, 100, 4), 1);
        QCOMPARE(at(180, 100, 4), 2);
        QCOMPARE(at(270, 100, 4), 3);
        // Screen coordinates: +x right, +y down.
        QCOMPARE(WheelGeometry::sliceAt(120, 0, 4, 50, 200), 1);
        QCOMPARE(WheelGeometry::sliceAt(0, 120, 4, 50, 200), 2);
        QCOMPARE(WheelGeometry::sliceAt(-120, 0, 4, 50, 200), 3);
        QCOMPARE(WheelGeometry::sliceAt(0, -120, 4, 50, 200), 0);
    }

    void sliceBoundaries()
    {
        // 4 slices: slice 0 spans [-45°, 45°).
        QCOMPARE(at(44.9, 100, 4), 0);
        QCOMPARE(at(45.1, 100, 4), 1);
        QCOMPARE(at(-44.9, 100, 4), 0);
        QCOMPARE(at(-45.1, 100, 4), 3);
        QCOMPARE(at(359.9, 100, 4), 0);
    }

    void everySliceReachable_data()
    {
        QTest::addColumn<int>("n");
        for (int n : {1, 2, 3, 5, 8, 13, 24})
            QTest::addRow("%d", n) << n;
    }
    void everySliceReachable()
    {
        QFETCH(int, n);
        const double sweep = 360.0 / n;
        for (int i = 0; i < n; ++i)
            QCOMPARE(at(i * sweep, 120, n), i);
    }

    void drawingMatchesHitTest()
    {
        // The midpoint of each drawn slice (Qt angles: 0 = 3 o'clock, clockwise)
        // must hit-test to the same index.
        for (int n : {3, 7, 12}) {
            for (int i = 0; i < n; ++i) {
                const double mid = qDegreesToRadians(WheelGeometry::sliceStartAngle(i, n) + WheelGeometry::sliceSweep(n) / 2);
                QCOMPARE(WheelGeometry::sliceAt(std::cos(mid) * 100, std::sin(mid) * 100, n, 50, 200), i);
            }
        }
    }

    void outerRadiusGrowsAndIsBounded()
    {
        QCOMPARE(WheelGeometry::outerRadiusFor(1), 150.0);
        QVERIFY(WheelGeometry::outerRadiusFor(20) > WheelGeometry::outerRadiusFor(8));
        QCOMPARE(WheelGeometry::outerRadiusFor(500), 360.0);
        QVERIFY(WheelGeometry::outerRadiusFor(6) > WheelGeometry::kInnerRadius * 2);
    }

    void scaleIsClampedAndDefaultsToOne()
    {
        QCOMPARE(WheelGeometry::clampScale(1.0), 1.0);
        QCOMPARE(WheelGeometry::clampScale(0.1), WheelGeometry::kMinScale);
        QCOMPARE(WheelGeometry::clampScale(9.0), WheelGeometry::kMaxScale);
        QCOMPARE(WheelGeometry::clampScale(std::nan("")), 1.0);
        QCOMPARE(WheelGeometry::innerRadiusFor(1.0), WheelGeometry::kInnerRadius);
        QCOMPARE(WheelGeometry::outerRadiusFor(6, 1.0), WheelGeometry::outerRadiusFor(6));
        QCOMPARE(WheelGeometry::outerRadiusFor(6, 1.4), WheelGeometry::outerRadiusFor(6) * 1.4);
    }

    void scaledDrawingMatchesHitTest_data()
    {
        QTest::addColumn<double>("scale");
        for (double s : {0.8, 1.0, 1.2, 1.4})
            QTest::addRow("%.1f", s) << s;
    }
    void scaledDrawingMatchesHitTest()
    {
        // The wheel draws and hit-tests with the same scaled radii: the
        // middle of each drawn ring maps to its slice, the drawn centre disc
        // cancels and points just beyond the scaled reach are outside.
        QFETCH(double, scale);
        for (int n : {3, 8}) {
            const double inner = WheelGeometry::innerRadiusFor(scale);
            const double outer = WheelGeometry::outerRadiusFor(n, scale);
            const double ring = (inner + outer) / 2;
            for (int i = 0; i < n; ++i) {
                const double mid = qDegreesToRadians(WheelGeometry::sliceStartAngle(i, n) + WheelGeometry::sliceSweep(n) / 2);
                QCOMPARE(WheelGeometry::sliceAt(std::cos(mid) * ring, std::sin(mid) * ring, n, inner, outer), i);
            }
            QCOMPARE(WheelGeometry::sliceAt(0, -(inner - 1), n, inner, outer), int(WheelGeometry::Cancel));
            QCOMPARE(WheelGeometry::sliceAt(0, -(inner + 1), n, inner, outer), 0);
            QCOMPARE(WheelGeometry::sliceAt(0, -(outer * WheelGeometry::kOuterReach + 1), n, inner, outer),
                     int(WheelGeometry::Outside));
        }
    }
};

QTEST_GUILESS_MAIN(TestWheelGeometry)
#include "tst_wheelgeometry.moc"
